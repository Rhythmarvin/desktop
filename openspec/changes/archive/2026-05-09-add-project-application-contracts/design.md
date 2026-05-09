## Context

The repository already separates domain entities from database migration bootstrap, but it does not yet have a transport-agnostic application layer or a frontend-facing contracts layer. The first implementation slice only needs to prove the architecture for `project`, so the design should create clear seams between `ora-domain`, `ora-application`, `ora-contracts`, transport adapters, and later persistence adapters without over-designing the rest of the product surface.

## Goals / Non-Goals

**Goals:**
- Establish `ora-application` as the owner of `project` CRUD orchestration, repository ports, mapping, and application-level errors.
- Establish `ora-contracts` as the owner of adapter-facing and frontend-generation-friendly request and response types for the first `project` slice.
- Keep handler APIs transport-agnostic so HTTP and Tauri can adapt them without leaking framework-specific concepts into core logic.
- Preserve testability by designing handlers against small traits and deterministic dependencies.

**Non-Goals:**
- Implement SQLite repository adapters in this change.
- Define task, session, worktree, or other non-`project` use cases.
- Finalize the long-term public contract shape beyond the first `project` slice.
- Add audit fields or transport-specific metadata to the initial `project` contracts.

## Decisions

### Keep `ora-contracts` separate from `ora-domain`

The project needs contract types that are serialization-friendly and stable enough for frontend TypeScript generation, while the domain layer should continue to model schema-backed entities and invariants. Keeping these crates separate prevents transport-driven shape decisions from leaking back into the domain model.

Alternative considered: exposing `ora-domain::Project` directly to adapters. This was rejected because it would couple frontend payloads to internal storage-oriented modeling and make future domain expansion harder to manage.

### Put repository ports and handler orchestration in `ora-application`

Handlers own use-case sequencing, validation that belongs at the application boundary, domain-to-contract mapping, and repository collaboration. Repository traits live beside handlers because they are ports defined by use-case needs rather than by persistence concerns.

Alternative considered: placing repository traits in `ora-domain` or `ora-db`. This was rejected because `ora-domain` should stay pure and `ora-db` should implement ports rather than define the use-case boundary.

### Model one handler per `project` CRUD use case

Using `CreateProjectHandler`, `GetProjectHandler`, `ListProjectsHandler`, `UpdateProjectHandler`, and `DeleteProjectHandler` keeps each entry point small and independently testable. Each handler takes one request DTO and returns one response DTO so every transport adapter can follow the same invoke-and-serialize flow.

Alternative considered: a single service struct with many ad hoc methods and shared optional dependencies. This was rejected because it weakens callsite clarity, makes testing broader than needed, and tends to blur use-case responsibilities.

### Keep delete externally hard-delete shaped while implementing soft delete internally

The first consumer-facing protocol should expose familiar CRUD semantics because that is the least surprising contract for frontend code. Internally, the application layer can call a repository port that marks records deleted without requiring the contract to expose storage behavior.

Alternative considered: exposing archive or soft-delete concepts in the first contract. This was rejected because the brainstorm scope explicitly keeps the first protocol surface intentionally small.

## Risks / Trade-offs

- [The first `project` slice may introduce abstractions that feel heavier than today's codebase] → Mitigation: keep the scope limited to one entity and only the ports needed by the five CRUD handlers.
- [Future frontend needs may require contract changes after TypeScript generation starts] → Mitigation: treat this slice as a validating architecture step and keep the initial contract surface intentionally minimal.
- [Soft delete semantics can surprise implementers if the contract implies hard deletion] → Mitigation: document the repository behavior and keep transport responses focused on observable CRUD results instead of persistence internals.
- [Leaving adapter and database integration for later could expose missing seams during implementation] → Mitigation: require specs to keep adapter boundaries and repository ports explicit so later crates have a clear contract to implement.
