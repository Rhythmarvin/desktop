# workflow-run/runtime

Ports and in-memory mock for project workflow mounts and graph runs.

## Responsibilities

- Define `WorkflowHostRepository` / `WorkflowRunRepository` and shared run types.
- Provide a memory implementation that registers `DemoWorkflow` snapshots,
  mounts them to projects, creates runs with definition snapshots, and (later)
  streams run events.
- Expose the active runtime through React context for hooks and UI.

## Non-responsibilities

- No HTTP/NDJSON transport (Follow-up F2).
- No Theater UI (lives in the parent `workflow-run` feature).
- Not the settings session graph editor.

## Invariants

- Creating a run freezes `definitionSnapshot` so later library edits cannot
  mutate an in-flight or historical run.
- Mount is unique per `(projectId, definitionId)`; remount upserts. Multiple
  executions are separate `GraphWorkflowRun` rows.
- Event union shapes must stay mappable to future NDJSON frames.
- Types use `GraphWorkflowRun` naming to avoid colliding with OpenSpec
  `WorkflowRun` in `workflow-store`.
