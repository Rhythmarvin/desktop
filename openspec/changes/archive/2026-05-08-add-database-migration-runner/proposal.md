## Why

The project has an initial SQLite schema in `docs/schema.sql`, but there is no migration system to bring a database instance to the expected version or roll it back when the local migration set changes. We need this now so the upcoming `ora-db` bootstrap work can initialize both file-backed and in-memory databases from a controlled, testable migration history instead of a single ad hoc schema dump.

## What Changes

- Add a database migration capability that defines versioned up/down migrations for the SQLite persistence layer.
- Introduce a `migrations` tracking table that records applied migration versions and execution timestamps inside the target database.
- Define migrations in Rust code rather than as standalone `.sql` assets.
- Define runner behavior that compares the database's recorded versions against the migrations available in the application and applies missing migrations in ascending order.
- Define rollback behavior that executes down migrations in descending order when the database contains versions that are no longer present in the available migration set.
- Establish validation and failure expectations for ordered execution, partial failure handling, and version consistency during database bootstrap.

## Capabilities

### New Capabilities
- `database-migrations`: Versioned SQLite schema migration management with tracked up/down execution and synchronization against the bundled migration set.

### Modified Capabilities

## Impact

Affected areas include the future `ora-db` crate bootstrap path, Rust-owned migration definitions, SQLite schema lifecycle, and automated tests that need deterministic database setup and teardown. This change also defines the contract that later application startup code will use when opening a database at a specific path or in memory.
