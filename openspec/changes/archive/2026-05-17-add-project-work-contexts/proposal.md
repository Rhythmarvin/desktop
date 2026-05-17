## Why

Ora currently persists project identity, but it does not persist which project a client window is actively working in. We need a durable, typed working-context model now so the web client can restore its current project consistently and future Tauri windows can coordinate exclusive project opens with crash-safe recovery.

## What Changes

- Add a persisted project work context model that tracks the active project for a client surface and window identity.
- Store work contexts as lease-backed rows so stale ownership expires automatically after crashes or abrupt shutdowns.
- Enforce exclusive project occupancy for non-expired desktop contexts while keeping the web client on a fixed synthetic window identity.
- Add cleanup and retention rules so expired contexts remain inspectable briefly without blocking future opens or switches.

## Capabilities

### New Capabilities
- `project-work-contexts`: Persist and enforce window-scoped project working context with lease-based conflict detection and recovery.

### Modified Capabilities

None.

## Impact

- Affects Rust domain models, SQLite schema and migrations, repository adapters, and application handlers for project selection flows.
- Affects web runtime bootstrap so the configured project also becomes the active web work context.
- Prepares app contracts and future Tauri composition for multi-window project coordination without adding backward-compatibility layers.
