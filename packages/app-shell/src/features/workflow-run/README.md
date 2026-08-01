# workflow-run

Product UI and mock runtime for **graph workflow runs** attached to projects
(sibling to tasks in the workspace tree).

## Responsibilities

- Host project mounts of workflow definitions and create/list `GraphWorkflowRun`
  instances (mock Host/Run repositories today; shape ready to extract).
- Render the Run Workspace when `workflowRunId` is selected:
  - **Theater** (default): focused act stage + path rail + live totals.
    Parallel `running` / `awaiting_input` nodes share the stage (primary +
    secondary cards); path chips highlight every active act.
  - **Overview**: read-only React Flow of the frozen snapshot with status skin
    on shared `workflow-node-chrome`
- Keep OpenSpec composer stepper (`features/workflow` + `workflow-store`) and
  settings React Flow editor interaction out of this module (shared chrome only).

## Non-responsibilities

- Does not persist definitions in `@ora/workflow-mock` (that package stays
  session-demo + validation).
- Does not own OpenSpec Spec-mode state.
- Does not call Rust/contracts workflow APIs yet.
- Does not reuse settings `WorkflowCanvas` (no catalog / reconnect / delete).
- Artifacts rail and HITL forms are later steps (stubs/events already exist).

## Mount vs run (product invariant)

- **Mount**: at most one `(projectId, definitionId)`. Remount refreshes the
  stored definition snapshot. Many projects may mount the same definition.
- **Run**: every successful deploy creates a **new** `GraphWorkflowRun` under
  the project (sidebar lists runs, not mounts).
- First deploy = mount + first run; later deploy to the same project = refresh
  mount + another run (UI copy distinguishes the two).

## Interactions

- Deploy (settings): searchable project picker, then mount upsert + create
  run, select that run, and close settings. Kickoff input belongs in the main
  workspace UI later (`create` / path policy already accept `kickoffInput`).
- Selection: `useWorkspaceSelectionStore.selectWorkflowRun`.
- Lists: react-query via `queryKeys.workflowMounts` /
  `workflowMountsByDefinition` / `workflowRuns`.
- Runtime: `WorkflowRuntimeProvider` in `AppShell` (memory + mock engine).
  `useGraphWorkflowRunLiveSync` patches run caches via `runs.watch`.
  Sidebar supports cancel (keep row) and delete (cancel then remove).
- View toggle: Theater ↔ Overview. Overview node click returns to Theater
  focused on that node. `awaiting_input` forces Theater.
