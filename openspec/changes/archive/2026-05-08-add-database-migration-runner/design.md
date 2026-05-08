## Context

The codebase has a canonical starting schema in `docs/schema.sql`, but database initialization is still planned work and there is no mechanism to evolve schema versions over time. The database bootstrap path needs a deterministic way to reconcile a target SQLite database with the migrations bundled in the application for both persisted databases and in-memory test databases.

The user also wants migration state to move in both directions: if the database has fewer applied migrations than the bundled set, the runner should apply the missing `up` steps in order; if the database records more versions than the bundled set, the runner should execute `down` steps in reverse order until the database matches the available migration set.

## Goals / Non-Goals

**Goals:**
- Define a versioned migration model with explicit `up` and `down` SQL for every migration.
- Establish a dedicated `migrations` table that records applied migration versions and execution timestamps in the target database.
- Specify the reconciliation algorithm used during bootstrap to bring a database into alignment with the bundled migration set.
- Keep the first migration grounded in `docs/schema.sql` so the initial database shape has a single authoritative source.
- Make the migration runner straightforward to test against file-backed and in-memory SQLite databases.

**Non-Goals:**
- Defining the final Rust API surface of `ora-db` beyond what is needed to support migration execution.
- Designing runtime hot migration workflows outside normal database bootstrap.
- Supporting arbitrary branching migration histories or multiple concurrent heads.

## Decisions

### Use ordered, versioned migrations with paired `up` and `down` SQL
Each migration will have a unique monotonically increasing version and an `up`/`down` pair. This keeps rollback behavior explicit and avoids trying to derive a reverse operation from forward-only SQL.

Alternatives considered:
- Forward-only migrations: rejected because the requested behavior requires removing extra applied versions when the local migration set shrinks.
- Timestamp-only discovery without a stable version contract: rejected because reconciliation logic becomes less transparent and harder to test.

### Keep migration SQL in Rust code instead of external `.sql` files
The migration catalog will be defined directly in Rust code, with each migration carrying its `up` and `down` SQL as code-owned statements or string literals inside the `ora-db` crate. This keeps discovery static, avoids runtime file-loading concerns in Tauri packaging, and makes versioned migration definitions part of the reviewed Rust surface.

Alternatives considered:
- Standalone `.sql` files: rejected because the user wants migration statements defined in Rust rather than managed as separate assets.
- Mixed Rust-plus-file representation: rejected because it adds packaging and ownership complexity without a clear benefit for the current scope.

### Track applied versions in a dedicated `migrations` table
The target database will contain a `migrations` table that stores one row per applied migration version plus its execution timestamp. The runner will treat that table as the database's source of truth for which versions are currently active, while the timestamp provides lightweight operational visibility without adding checksum-management complexity.

Alternatives considered:
- Inferring state from table existence: rejected because schema shape alone cannot reliably distinguish versions or support safe rollback ordering.
- Storing only a single current version number: rejected because a per-version record makes ordered validation and partial reconciliation easier to reason about.
- Storing extra checksum metadata: rejected for now because the user only wants execution time in addition to version.

### Reconcile database state against the bundled migration set during bootstrap
At startup, the runner will compare the ordered available migrations with the ordered applied versions from the database.

If the database is missing trailing versions from the bundled set, the runner will execute the missing `up` migrations in ascending order and record each version as it succeeds.

If the database contains trailing versions that are not present in the bundled set, the runner will execute the corresponding `down` migrations in descending order and remove each version record as it succeeds.

This design assumes a single linear migration history. A mismatch in the shared prefix between applied and available versions is treated as an error instead of attempting to guess recovery behavior.

Alternatives considered:
- Auto-rebuilding the database on mismatch: rejected because it is unsafe for persisted user data.
- Allowing gaps or branch merges in version history: rejected because the current project does not need that complexity.

### Keep the initial migration aligned with `docs/schema.sql`
The first migration will create the schema represented in `docs/schema.sql`, plus the new `migrations` tracking table. That keeps the schema definition and migration history aligned while the persistence layer is still being introduced.

Alternatives considered:
- Duplicating the entire schema in a separate handwritten migration without reference to `docs/schema.sql`: rejected because it would create two sources of truth immediately.

### Execute each migration step transactionally where SQLite allows it
The runner should wrap each migration application or rollback step in a transaction so a failed `up` or `down` does not leave the version table inconsistent with the schema change it represents. If a step fails, execution stops and reports the failing version.

Alternatives considered:
- Best-effort continuation after failure: rejected because it makes the resulting schema state difficult to trust and debug.

## Risks / Trade-offs

- Prefix mismatch between applied and available versions could block startup for an existing database -> Mitigation: surface a clear error describing the unexpected versions and require an explicit migration fix rather than guessing.
- Maintaining `down` migrations increases authoring cost -> Mitigation: require every migration to ship with its paired rollback and cover both directions in tests.
- Keeping `docs/schema.sql` and migration assets aligned can drift over time -> Mitigation: treat the first migration as the canonical bootstrap mapping and add tests that validate a freshly migrated database matches expected schema objects.

## Migration Plan

1. Introduce the migration artifact layout and the `migrations` bookkeeping table.
2. Create migration version `0001` in Rust from the schema currently defined in `docs/schema.sql`, extended to create the `migrations` table with an execution timestamp column.
3. Implement bootstrap reconciliation so new databases apply all known `up` migrations and previously initialized databases are synchronized against the available version set.
4. Add tests for new database initialization, incremental upgrade, reverse rollback when versions are removed, and prefix mismatch failure.

Rollback strategy:
- If a newly introduced migration needs to be removed from the bundled set, the runner will execute its `down` step on databases that still record that version.
- If a released migration is found to be invalid and cannot be safely rolled back automatically, the runner will fail with a mismatch or execution error rather than silently mutating to an unknown state.

## Open Questions

- What timestamp representation the `migrations.executed_at` column should use in SQLite so it remains easy to read and stable across platforms.
