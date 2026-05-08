# Database Migrations

Ora keeps SQLite migration definitions in Rust code inside `ora-db` rather than as standalone `.sql` files.

## Rules

- Every migration has a monotonically increasing version such as `0001`.
- Every migration provides both `up` and `down` SQL.
- The `migrations` bookkeeping table stores `version` and `executed_at`.

## Reconciliation Model

The bootstrapper compares the applied rows in `migrations` with the active target prefix from the Rust migration catalog.

- If the database is missing trailing target versions, the bootstrapper applies their `up` SQL in ascending order.
- If the database has extra trailing versions beyond the current target prefix, the bootstrapper executes their `down` SQL in descending order.
- If the shared prefix diverges, bootstrap fails instead of guessing at repair.

Because rollback needs access to `down` SQL, retired tail migrations should remain defined in Rust until every managed database has been reconciled to the shorter target prefix.
