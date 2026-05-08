## 1. Define Migration Assets

- [x] 1.1 Create the Rust migration catalog and version representation that the database bootstrap layer will load in order.
- [x] 1.2 Add the initial `0001` migration with explicit Rust-owned `up` and `down` SQL based on `docs/schema.sql`, including creation of the `migrations` tracking table with an execution timestamp column.
- [x] 1.3 Document how new migrations must provide paired `up` and `down` steps and monotonically increasing versions.

## 2. Implement Reconciliation Runner

- [x] 2.1 Implement bootstrap logic that creates or opens the `migrations` table and reads applied versions plus execution timestamps from the target SQLite database.
- [x] 2.2 Implement ascending `up` execution for bundled versions missing from the database and record each version only after success.
- [x] 2.3 Implement descending `down` execution for applied versions that are no longer present in the bundled migration set and remove each version record after success.
- [x] 2.4 Detect shared-prefix mismatches between applied and bundled versions and return an explicit reconciliation error without attempting repair.

## 3. Harden Failure and Transaction Semantics

- [x] 3.1 Execute each `up` or `down` step transactionally so failed migrations do not leave schema state and recorded versions out of sync.
- [x] 3.2 Surface structured errors that identify the failing migration version and operation direction.

## 4. Verify Behavior

- [x] 4.1 Add tests for fresh database bootstrap applying all migrations to an empty SQLite database.
- [x] 4.2 Add tests for incremental upgrade when the database is behind the bundled migration set.
- [x] 4.3 Add tests for reverse rollback when the database contains versions that were removed from the bundled migration set.
- [x] 4.4 Add tests for shared-prefix mismatch and migration-step failure cases.
- [x] 4.5 Update any relevant documentation in `docs/` to explain the migration workflow and the relationship between `docs/schema.sql` and versioned migrations.
