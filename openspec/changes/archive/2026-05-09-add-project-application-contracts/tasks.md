## 1. Workspace and crate scaffolding

- [x] 1.1 Add `crates/application` and `crates/contracts` as workspace members and declare the shared dependencies they need for serialization, error handling, and testing.
- [x] 1.2 Scaffold `ora-contracts` with a private module layout and explicit public exports for the `project` CRUD request and response DTOs plus the shared `Project` view model.
- [x] 1.3 Scaffold `ora-application` with a private module layout and explicit public exports for `project` handlers, application errors, and handler-owned ports.

## 2. Define the project contract surface

- [x] 2.1 Implement the `ora-contracts` `project` DTOs for `CreateProject`, `GetProject`, `ListProjects`, `UpdateProject`, and `DeleteProject`.
- [x] 2.2 Implement the shared public `ora_contracts::Project` view model with only `id`, `name`, and `root_path`.
- [x] 2.3 Add contract-focused tests that verify serialization-friendly `project` payloads and preserve the single shared project view shape.

## 3. Implement application handlers and ports

- [x] 3.1 Define the `ora-application` project repository and supporting dependency traits needed by the five CRUD handlers.
- [x] 3.2 Implement `CreateProjectHandler`, `GetProjectHandler`, `ListProjectsHandler`, `UpdateProjectHandler`, and `DeleteProjectHandler` so each accepts one contract request and returns one contract response or an application error.
- [x] 3.3 Implement the domain-to-contract mapping inside `ora-application`, including the externally delete-shaped flow backed by an internal soft-delete-oriented repository call.
- [x] 3.4 Add unit tests with in-memory fakes that exercise each handler without HTTP, Tauri, or SQLite dependencies.

## 4. Adapter and documentation follow-through

- [x] 4.1 Update the web server wiring to consume `ora-contracts` request types and delegate `project` use cases through `ora-application` handlers without transport-owned orchestration.
- [x] 4.2 Document the new application/contracts boundary and the first `project` vertical slice in `docs/` wherever architecture or API references need to stay current.
- [x] 4.3 Run `task test` and fix any failures introduced by the new workspace crates and handler flow.
