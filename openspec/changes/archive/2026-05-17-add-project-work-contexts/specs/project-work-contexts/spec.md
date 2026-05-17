## ADDED Requirements

### Requirement: System SHALL persist window-scoped project work contexts
The system SHALL persist a typed `project_work_context` record for each active client window, including a unique identifier, `surface`, `window_id`, `project_id`, `lease_expires_at`, `created_at`, and `updated_at`. The `projects` table SHALL remain limited to project identity and SHALL NOT become the source of truth for active working context. The backend SHALL compute `lease_expires_at` from its own current time using a two-minute lease duration and SHALL NOT trust clients to provide an absolute expiry timestamp.

#### Scenario: Web runtime records its active project
- **WHEN** the web runtime finishes bootstrap for its configured project
- **THEN** the backend persists or updates one work-context row for the synthetic web window identity that points at the configured project and includes a non-expired lease

#### Scenario: Client sends a lease-related context request
- **WHEN** a client asks the backend to create, switch, or renew a project work context
- **THEN** the backend computes the resulting `lease_expires_at` from backend time plus two minutes instead of accepting a client-supplied absolute expiration

#### Scenario: Caller reads project identity
- **WHEN** application or persistence code loads a `project`
- **THEN** the returned project model contains only project identity fields and excludes transient active-context ownership data

### Requirement: System SHALL enforce active-context uniqueness using lease-aware rules
The system SHALL allow at most one non-expired active work context per `(surface, window_id)` pair. The system SHALL reject a project-open or project-switch operation when another non-expired conflicting context already owns the requested project according to the configured exclusivity rules. Conflict responses exposed to clients SHALL report only that the project is occupied, while backend logs SHALL include the owning `surface` and `window_id`.

#### Scenario: Window replaces its own active project
- **WHEN** the same `(surface, window_id)` requests a switch from one project to another while its current context lease is still valid
- **THEN** the backend updates that window's active work context instead of creating a second active row for the same window identity

#### Scenario: Different desktop window tries to open an occupied project
- **WHEN** one non-expired Tauri window context already points at `project-a` and a different Tauri window requests `project-a`
- **THEN** the backend rejects the request as a conflict instead of granting two active desktop contexts for the same project

#### Scenario: Client receives a conflict for an occupied project
- **WHEN** a project-open or project-switch request is rejected because another non-expired context owns that project
- **THEN** the client-facing response reports only that the project is occupied and the backend logs identify the owning `surface` and `window_id`

### Requirement: System SHALL ignore expired work contexts during conflict detection
The system SHALL treat `lease_expires_at` as the boundary for active ownership. Project-open and project-switch checks SHALL consider only non-expired contexts, and expired rows SHALL NOT block a new context from claiming the project. Clients SHALL renew active leases every 30 seconds, and bootstrap, open, and switch operations SHALL immediately refresh the lease as part of establishing the context.

#### Scenario: Expired context exists for requested project
- **WHEN** a caller requests a project whose only matching work-context rows have `lease_expires_at` earlier than the current backend time
- **THEN** the backend allows the new context to claim that project

#### Scenario: Client renews an active lease
- **WHEN** the owning client renews its work context before `lease_expires_at`
- **THEN** the backend extends the same row's lease rather than creating a duplicate active context

#### Scenario: Context-establishing action refreshes the lease immediately
- **WHEN** the backend handles bootstrap, open, or switch for a client window
- **THEN** it writes a fresh non-expired lease for that context during the same operation instead of waiting for the next 30-second renewal tick

### Requirement: System SHALL retain expired contexts temporarily without keeping them active
The system SHALL retain expired work-context rows for three days for debugging and operational inspection. Cleanup behavior SHALL remove rows older than that retention window, and retained expired rows SHALL remain invisible to active-ownership checks.

#### Scenario: Recently expired context is inspected
- **WHEN** an operator or internal workflow inspects persisted work contexts within three days after a lease expired
- **THEN** the expired row is still present for inspection even though it no longer blocks project access

#### Scenario: Expired context exceeds retention window
- **WHEN** cleanup runs for a work-context row whose lease expiry is older than the configured three-day retention period
- **THEN** the backend deletes that row from persistent storage
