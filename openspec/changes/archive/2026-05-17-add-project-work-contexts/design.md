## Context

Ora currently models `projects` as pure persisted project identity and already reconciles one configured project during web-server startup. There is no persisted runtime-context table or typed ownership model for "which project is active right now," which makes crash recovery, multi-window coordination, and future Tauri support hard to add incrementally without overloading the `projects` table.

This change introduces a dedicated `project_work_contexts` persistence model owned by the backend. Each row binds one client surface and window identity to one project through a renewable lease. The design keeps project identity stable, allows the web client to use a fixed synthetic window identity, and gives Tauri a path to real per-window ownership without relying on graceful shutdown behavior.

## Goals / Non-Goals

**Goals:**

- Preserve `projects` as pure project identity without embedding transient "current project" state into that table.
- Add a typed backend-owned work-context model that supports web and future Tauri surfaces.
- Enforce exclusive project occupancy based on non-expired leases so two desktop windows cannot actively work in the same project at the same time.
- Make stale ownership self-healing through lease expiration rather than frontend-managed cleanup.
- Keep expired rows briefly for debugging while excluding them from all active conflict checks.

**Non-Goals:**

- Designing the full Tauri UI or transport surface for window management.
- Adding frontend-only persistence as a source of truth for active-project recovery.
- Introducing backward-compatibility layers that keep old project-selection behavior alive in parallel.

## Decisions

### Add a dedicated `project_work_contexts` table

We will model active working context as its own table with `id`, `surface`, `window_id`, `project_id`, `lease_expires_at`, `created_at`, and `updated_at`.

Why this approach:

- It keeps project identity separate from transient window ownership.
- It makes illegal states easier to prevent because each row represents exactly one active window-to-project binding.
- It avoids stuffing nullable "current" fields onto `projects`, which would not scale to multiple windows.

Alternatives considered:

- Add `current_window_id` or similar fields to `projects`: rejected because it couples identity and runtime state and does not model one window cleanly.
- Store current project only in frontend state: rejected because crashes and multi-window exclusivity need backend truth.

### Treat work contexts as leases instead of permanent claims

Each active context row will expire unless the owning client renews its `lease_expires_at`. Normal close may eagerly delete the row, but correctness will not depend on that path succeeding.

The first implementation will use a two-minute lease duration for both web and Tauri clients, with clients renewing every 30 seconds. The backend will compute `lease_expires_at` from its own current time and a fixed lease duration instead of trusting a client-provided absolute expiry timestamp. Context-establishing actions such as bootstrap, open, and switch will also renew the lease immediately as part of the same operation rather than waiting for the periodic renewal loop.

Why this approach:

- Crash recovery becomes automatic because stale rows stop blocking once their lease expires.
- The backend can reason about active ownership using a single time-based rule for both web and Tauri.

Alternatives considered:

- Permanent rows deleted only on close: rejected because abrupt process exits would leave projects blocked indefinitely.
- Immediate hard cleanup of expired rows: rejected because short retention helps debugging and operational inspection.

### Model client identity as `(surface, window_id)`

Web will use a fixed synthetic identity such as `surface = web` and `window_id = main`. Tauri will supply real window identifiers so different windows can hold different projects simultaneously.

Why this approach:

- It provides one portable ownership key that works for both current and future clients.
- It lets the same repository and application logic handle single-window web and multi-window desktop cases.

Alternatives considered:

- Separate tables or ports per client type: rejected because the domain rule is the same and should stay unified.

### Enforce exclusivity only for non-expired contexts

Project-open and project-switch flows will treat only non-expired contexts as active. A surface/window can have at most one active context, and a project can have at most one non-expired Tauri owner at a time.

Why this approach:

- It aligns conflict detection with the lease model.
- It avoids stale rows creating false-positive conflicts after crashes.

Alternatives considered:

- Global uniqueness regardless of expiry: rejected because it breaks crash recovery.
- No exclusivity at all: rejected because future multi-window desktop behavior needs deterministic conflict checks.

### Keep conflict details in logs while returning a simple occupied response

Conflict handling will log the owning `surface` and `window_id` for operators and debugging, but frontend-facing responses will expose only that the requested project is already occupied elsewhere.

Why this approach:

- It gives developers enough context to diagnose unexpected conflicts.
- It avoids coupling frontend behavior to internal window identifiers that may change by client type.

Alternatives considered:

- Return `surface` and `window_id` to the frontend: rejected because the product decision only needs occupancy state and internal identifiers are better kept as backend diagnostics.

## Risks / Trade-offs

- Lease duration tuning could be too short or too long -> Start with a conservative default and centralize renewal policy so it can be adjusted later.
- Time-based ownership depends on consistent clock handling -> Compute and compare lease timestamps in the backend and cover expiry boundaries with tests.
- Web bootstrap now owns both project reconciliation and active-context reconciliation -> Keep bootstrap orchestration thin and push persistence rules into repositories and handlers.
- Retaining expired rows increases table size modestly -> Limit retention to three days and add cleanup behavior for older expired rows.

## Migration Plan

- Add a new schema migration that creates `project_work_contexts` and supporting indexes or constraints.
- Introduce matching domain types, repository ports, and SQLite adapters.
- Update bootstrap and project-selection flows to create or renew the web work context and to enforce conflict detection through the new repository.
- Roll forward only; if rollback is required before release, revert the migration and dependent code together because there is no compatibility layer planned.

## Open Questions

None.
