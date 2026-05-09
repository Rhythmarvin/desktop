## Purpose

Define the transport-neutral, serialization-friendly contract surface used by adapters and frontend generation for the first `project` vertical slice.

## Requirements

### Requirement: Project contracts SHALL define the first frontend-facing CRUD protocol
The system SHALL define request and response DTOs for `CreateProject`, `GetProject`, `ListProjects`, `UpdateProject`, and `DeleteProject` in `ora-contracts`. These DTOs SHALL be the transport-neutral contract surface used by adapters and by frontend type generation.

#### Scenario: Adapter needs a request payload type
- **WHEN** an HTTP or Tauri adapter accepts input for a `project` CRUD action
- **THEN** it uses the corresponding `ora-contracts` request DTO instead of transport-local ad hoc structs

#### Scenario: Frontend types are generated
- **WHEN** the repository generates frontend-consumable types from Rust contracts
- **THEN** the generated types come from `ora-contracts` rather than directly from domain entities or adapter-specific payload structs

### Requirement: Project view contracts SHALL expose a single public project shape
The system SHALL expose a single public `ora_contracts::Project` view model for the first `project` slice, and that model SHALL include `id`, `name`, and `root_path` fields only.

#### Scenario: Handler returns a project to an adapter
- **WHEN** a create, get, list, or update use case returns project data
- **THEN** the response uses the shared `ora_contracts::Project` shape instead of separate summary and detail variants

#### Scenario: Caller inspects project payload fields
- **WHEN** an adapter or generated frontend type consumes `ora_contracts::Project`
- **THEN** it receives `id`, `name`, and `root_path` and does not receive `created_at`, `updated_at`, or other audit fields in the first version

### Requirement: Contract types SHALL remain serialization-friendly and domain-decoupled
The system SHALL keep `ora-contracts` types suitable for serialization and frontend generation, and SHALL require `ora-application` to map domain entities into those contracts rather than exposing raw domain models directly.

#### Scenario: Domain model evolves internally
- **WHEN** the domain layer adds internal fields or invariants that are not part of the app-facing protocol
- **THEN** adapters and generated frontend types remain bound to `ora-contracts` shapes instead of inheriting those internal domain details automatically
