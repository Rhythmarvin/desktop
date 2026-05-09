## Why

The repository currently has domain models and database bootstrap logic, but it does not yet have an application layer or transport-agnostic contract layer for feature use cases. We need that split now so the first `project` flow can prove that HTTP and Tauri adapters stay thin while the core use-case logic remains testable and reusable.

## What Changes

- Add `ora-application` as the home for `project` CRUD use-case handlers, application errors, repository ports, and mapping from domain entities into app-facing responses.
- Add `ora-contracts` as the home for serialization-friendly request and response DTOs that define the frontend-facing `project` protocol surface.
- Define the first `project` vertical slice around `CreateProject`, `GetProject`, `ListProjects`, `UpdateProject`, and `DeleteProject` handlers with one contract request and one contract response per use case.
- Keep delete externally modeled as CRUD deletion while allowing application and persistence layers to implement it as a soft delete internally.
- Document the architectural boundary that HTTP and Tauri adapters only translate transport input and output, without owning use-case orchestration.

## Capabilities

### New Capabilities
- `application-handlers`: Defines transport-agnostic application handlers, ports, and error behavior for `project` CRUD use cases.
- `app-contracts`: Defines serialization-friendly `project` request and response contracts shared by adapters and frontend code generation.

### Modified Capabilities

## Impact

- Affected code: `crates/application`, `crates/contracts`, future updates in `crates/db`, `apps/web/server`, and the future Tauri adapter.
- Affected APIs: new public Rust APIs for `ora-application` handlers and `ora-contracts` DTOs.
- Affected architecture: introduces the repository's first explicit application and contract boundaries for vertical slice delivery.
