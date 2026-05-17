## 1. Domain And Persistence Model

- [x] 1.1 Add the `project_work_contexts` SQLite schema, migration, indexes, and retention-oriented query support in `ora-db`.
- [x] 1.2 Add typed `ora-domain` models and identifiers for project work contexts, including lease expiry and client-surface identity fields.
- [x] 1.3 Add application-owned repository traits and error types for creating, updating, finding, renewing, deleting, and cleaning up project work contexts.

## 2. Repository And Lease Logic

- [x] 2.1 Implement SQLite-backed project work context repositories that map rows to domain models and filter active versus expired contexts correctly.
- [x] 2.2 Implement backend-owned lease computation with a two-minute duration, 30-second renewal cadence support, and immediate lease refresh on bootstrap, open, and switch flows.
- [x] 2.3 Implement lease-aware conflict checks that enforce one active context per `(surface, window_id)`, reject conflicting non-expired desktop ownership, and log owning `surface` and `window_id` while returning only occupied-state details to clients.
- [x] 2.4 Implement retention cleanup and lease-renewal operations with tests covering expiry boundaries, stale-row recovery, and backend-time-based expiry calculation.

## 3. Runtime Integration

- [x] 3.1 Update web bootstrap to reconcile the configured project into an active synthetic web work context during startup.
- [x] 3.2 Update project-open and project-switch application flows to use the new work context repository as the source of truth for active project ownership.
- [x] 3.3 Add or update transport and integration tests that cover bootstrap reconciliation, immediate lease refresh on context establishment, conflict rejection, successful switch, periodic lease renewal, and expired-context recovery.
