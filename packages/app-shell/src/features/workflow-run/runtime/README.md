# workflow-run/runtime

Ports and in-memory mock for project workflow mounts and graph runs.

## Responsibilities

- Define `WorkflowHostRepository` / `WorkflowRunRepository` and shared run types.
- Provide a memory implementation that registers `DemoWorkflow` snapshots,
  mounts them to projects, and creates runs with frozen definition snapshots.
- Drive runs with `mock-run-engine` + `mock-execution-plan` (timed progression and
  `WorkflowRunEvent` stream). Cancel / delete clear timers so mid-run stop is reliable.
- Expose the active runtime through React context for hooks and UI.
- Notify `runs.watch` listeners on mutations so react-query can refresh sidebar
  status without Theater UI.

## Non-responsibilities

- No HTTP/NDJSON transport (Follow-up F2).
- No Theater / overview / HITL UI (parent `workflow-run` feature, Steps 3–5).
- Not the settings session graph editor.
- Settings **Test run** still uses `@ora/workflow-mock` `runDemoWorkflow` — a
  separate demo path until an optional later convergence.

## Invariants

- Creating a run freezes `definitionSnapshot` so later library edits cannot
  mutate an in-flight or historical run.
- Mount is unique per `(projectId, definitionId)`; remount upserts. Multiple
  executions are separate `GraphWorkflowRun` rows.
- Concurrent runs are independent; cancelling one does not stop siblings.
- Event union shapes must stay mappable to future NDJSON frames.
- Types use `GraphWorkflowRun` naming to avoid colliding with OpenSpec
  `WorkflowRun` in `workflow-store`.

## Mock engine semantics (extensible)

- **Path plan**: `planMockExecution` walks from `start` seeds. At `condition`
  nodes it picks **one** outgoing edge via `MockPathPolicy` (default is
  kickoff-aware label heuristics; otherwise first edge). Unreachable nodes are
  marked `skipped` and emit `node_finished` with that status.
- **Start**: only from `pending`. Re-entrant `start` is a no-op (HITL resume will
  be a separate API).
- **Tokens**: stubbed only for `prompt` / `agent` / `tool` kinds.
- **Artifacts**: markdown stubs on `agent` / `output` for Step 4.
- **Options**: `nodeStepMs` (default 450), `autoStart` (default true; kickoff UI
  can create then `start`), injectable `pathPolicy`.
- Kickoff text is stored on the run and fed into path planning when provided.
  Deploy does not collect it; the main workspace start flow will.
