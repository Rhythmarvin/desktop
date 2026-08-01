# workflow-run

Product UI and mock runtime for **graph workflow runs** attached to projects
(sibling to tasks in the workspace tree).

## Responsibilities

- Host project mounts of workflow definitions and create/list `GraphWorkflowRun`
  instances (mock Host/Run repositories today; shape ready to extract).
- Render the Run Workspace when `workflowRunId` is selected (Theater / overview
  arrive in later steps; Step 1 is navigation + placeholder).
- Keep OpenSpec composer stepper (`features/workflow` + `workflow-store`) and
  settings React Flow editor (`settings/workflow-flow`) out of this module.

## Non-responsibilities

- Does not persist definitions in `@ora/workflow-mock` (that package stays
  session-demo + validation).
- Does not own OpenSpec Spec-mode state.
- Does not call Rust/contracts workflow APIs yet.

## Mount vs run (product invariant)

- **Mount**: at most one `(projectId, definitionId)`. Remount refreshes the
  stored definition snapshot. Many projects may mount the same definition.
- **Run**: every successful deploy creates a **new** `GraphWorkflowRun` under
  the project (sidebar lists runs, not mounts).
- First deploy = mount + first run; later deploy to the same project = refresh
  mount + another run (UI copy distinguishes the two).

## Interactions

- Deploy (settings): searchable project picker (Command), then mount upsert +
  create run, select that run, and close settings.
- Selection: `useWorkspaceSelectionStore.selectWorkflowRun`.
- Lists: react-query via `queryKeys.workflowMounts` /
  `workflowMountsByDefinition` / `workflowRuns`.
- Runtime: `WorkflowRuntimeProvider` in `AppShell` (memory impl for MVP).
