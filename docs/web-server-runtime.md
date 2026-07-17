# Web Server Runtime

`apps/web/server` is the first HTTP backend runtime for Ora.

## Purpose

- It boots shared structured logging through `ora-logging`.
- It exposes health endpoints for process liveness and runtime readiness.
- It serves persisted HTTP CRUD routes for `project`, `task`, and `session` by delegating to `ora-application`.
- It provisions and cleans up task-owned linked worktrees as internal backend state during task lifecycle flows.
- It owns the shared `ora-pty` runtime manager used to start, attach, stream, resize, kill, and tear down task terminal sessions.
- It embeds the plugin management state writer, pinned Agent runtime supervisor, and authenticated loopback adapter in one `BackendRuntime`.

## Database Configuration

The web server reads its runtime data root from:

- `ORA_DATA_DIR`: root directory for runtime state. Default: `.`

Startup bootstraps the database through `ora-db`, applies the active migration catalog, and constructs the shared repository pool before the runtime is marked ready.

- SQLite database path: `<ORA_DATA_DIR>/ora.sqlite3`
- Worktree root: `<ORA_DATA_DIR>/worktrees`
- Log file: `<ORA_DATA_DIR>/logs/ora.log`

## Project Configuration

The web server also requires a bootstrap project identity:

- `ORA_PROJECT_NAME`: persisted workspace project name. Required.
- `ORA_PROJECT_PATH`: persisted workspace root path. Required.

Startup reconciles this configured project into the `projects` table before the runtime is marked ready.

- If no visible project exists with the configured name, startup creates one row.
- If a visible project exists with the configured name but a different stored path, startup updates that row in place.
- If both the configured name and path already match, startup leaves the row unchanged.
- If `ORA_WORK_DIR` is unset, startup uses a `worktrees/` directory next to the configured SQLite database file.
- Task creation provisions linked worktrees under `ORA_WORK_DIR/<full-task-id>`.
- Terminal session startup also resolves its working directory under `ORA_WORK_DIR/<full-task-id>` from backend-owned task/worktree state instead of accepting any caller-supplied path.
- After project reconciliation, startup also opens the synthetic web work context `surface = web`, `window_id = main` for that project and refreshes its lease immediately.

## Bind Configuration

`BackendRuntime` always binds the complete backend to `127.0.0.1:0`; the operating system chooses a
fresh port on every start. There is no `ORA_HOST`/`ORA_PORT` override. Packaged Tauri receives the
actual endpoint and a fresh 256-bit bearer through the in-process bootstrap command. The standalone
browser composition logs the actual endpoint but omits every plugin and Agent-invocation route
because it has no trusted bearer bootstrap channel.

Packaged plugin routes additionally require the exact runtime Host, an allowlisted non-null Origin,
and `Authorization: Bearer`. CORS preflight omits the bearer but still enforces exact Host, Origin,
route method, and the `Authorization`/`Content-Type` header allowlist. Success, error, and NDJSON
stream responses echo only the accepted Origin and include `Vary: Origin`.

## Health Endpoints

- `GET /health/live`: confirms that the process is running
- `GET /health/ready`: confirms that application-state bootstrap completed successfully

`/health/ready` remains unavailable until the runtime finishes constructing its application state.

## HTTP API

The persisted runtime exposes CRUD routes for the supported public models:

- `POST /api/projects`
- `GET /api/projects`
- `GET /api/projects/{project_id}`
- `PUT /api/projects/{project_id}`
- `DELETE /api/projects/{project_id}`
- `POST /api/project-work-contexts/open`
- `POST /api/project-work-contexts/renew`
- `POST /api/tasks`
- `GET /api/tasks`
- `GET /api/tasks/{task_id}`
- `PUT /api/tasks/{task_id}`
- `DELETE /api/tasks/{task_id}`
- `POST /api/sessions`
- `GET /api/sessions`
- `GET /api/sessions/{session_id}`
- `PUT /api/sessions/{session_id}`
- `DELETE /api/sessions/{session_id}`
- `GET /api/sessions/{session_id}/terminal` (WebSocket upgrade)

Request and response payloads use `ora-contracts` DTO shapes, so transport behavior stays aligned with the shared application contract.
Task payloads do not expose backend-owned worktree identifiers, and the runtime does not expose standalone public worktree CRUD endpoints.
Terminal session startup also keeps backend worktree ownership private by accepting only startup dimensions on `CreateSessionRequest`.

The project work context routes provide the current backend-managed project selection surface.

- `open` creates or switches one `(surface, window_id)` context into a project and refreshes its lease immediately.
- `renew` extends an existing context lease using backend time.
- Occupied-project conflicts return a stable HTTP `409` error without exposing the owning surface or window id in the response.

### Authenticated Plugin API

Plugin paths and strict request/response DTOs are owned by `ora-contracts`; Agent domain/wire DTOs
remain owned by `ora-plugin-protocol`. The adapter exposes catalog, configured-root scan, opaque
selection/candidate identify and install, enable/disable/uninstall, launch grants, runtime start/stop,
typed invocation, cancellation, and confirmed data removal. Client-provided local paths, content
digests, executable paths, and working directories are never accepted as authority.

Tauri native folder selection passes the operating-system path directly to the backend and returns
only a session-bound, expiring, single-use `SelectionHandle`. Identify consumes it and mints a
digest-bound `CandidateHandle`; install consumes that second handle. Project/worktree invocation
scope identifiers are resolved through current SQLite membership before the Host constructs the
canonical Agent scope.

Runtime resources are supplied from the packaged `plugin-runtime/` resource directory. Development
tests prepare the same strict layout explicitly with `task prepare-plugin-runtime`; normal build and
test commands never fall back to `PATH` Bun.

## Terminal Runtime

- Terminal creation still begins at `POST /api/sessions`, where terminal-backed requests use `agentId = "terminal"` plus an initial `terminal` size payload.
- The WebSocket route upgrades only after the runtime proves the addressed session is a running terminal session and no live client is already attached.
- The terminal protocol is text-oriented and shared with future Tauri clients:
  - Server messages: `ready`, `history`, `output`, `exit`, `error`
  - Client messages: `input`, `resize`, `kill`
- Reconnects have no idle timeout while the PTY is still running. Disconnecting a WebSocket client only detaches that client; it does not terminate the PTY.
- Reattaching to a running session replays bounded in-memory output history before forwarding new live output.
- Initial `cols` and `rows` only size the PTY at startup. Later viewport changes must arrive through terminal `resize` messages after attach.
- One server-owned cancellation token roots all terminal shutdown. Each session derives a child token so PTY readers, writers, and WebSocket loops shut down together when the PTY exits, when a kill request succeeds, or when the server shuts down.
- Server shutdown uses that root token to tear down PTYs instead of relying on client disconnect behavior.

## Storage Behavior

The current runtime uses a file-backed SQLite database bootstrapped through `ora-db`.

- Data persists across process restarts as long as the same `ORA_DATA_DIR` is reused.
- Readiness depends on successful database bootstrap, repository-pool construction, bootstrap-project reconciliation, and synthetic web work context reconciliation.
- Application-layer failures still map into the shared structured HTTP error envelope across the supported route families.
