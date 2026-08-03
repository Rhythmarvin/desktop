# Workflow

`ora-application` owns the workflow definition use cases, with persistence in `ora-db` and public contracts in `ora-contracts`. Workflows manage editable agent orchestration graphs with draft-as-workspace semantics and immutable published snapshots.

## Entities and tables

| Domain type | Backing table |
| --- | --- |
| `Workflow` | `workflows` |
| `WorkflowSnapshot` | `workflow_snapshots` |

`Workflow` holds the stable identity (name, published snapshot pointer, audit fields) while `WorkflowSnapshot` owns the versioned React Flow graph. Read models (`WorkflowDetail`, `WorkflowSummary`, `WorkflowVersion`) keep graph data out of list responses.

## Draft, publish, and version lifecycle

Every workflow has exactly one `draft` snapshot created atomically with the workflow itself. The draft is an editable workspace: `UpdateDraft` mutates its graph in-place without creating a new snapshot row.

Publishing copies the draft's graph into a new, immutable snapshot. `updated_at` on published snapshots is `NULL`, including after soft deletion — only draft editing changes that field. Publish always activates the new snapshot (sets `workflows.published_snapshot_id`), making it the version used by any future workflow execution.

Additional operations keep the version model flexible without data loss:

- **Rollback** copies any historical snapshot's graph into the draft. It does not change the published pointer, so the active version stays unchanged while the editor workspace resets to a known state.
- **Activate** switches the published pointer to a different snapshot and syncs its graph into the draft. This is the explicit "make this version live" operation for cases where publish-and-activate-together is not desired.
- **Snapshot deletion** removes individual published snapshots but refuses to delete the draft or the currently active version.

## Identifiers and versioning

`WorkflowId` and `WorkflowSnapshotId` are UUID-backed newtypes following the same `define_id!` macro convention as every other domain entity.

Snapshot versions are strings. The draft is identified by the reserved string `"draft"`. Published versions can be user-provided (e.g. `"v1.0.0"`) or automatically derived as `v{timestamp_millis}` from the same injected application clock that supplies `created_at`. If an automatic version collides in the same millisecond, a numeric suffix is added without changing its clock-derived prefix. User-provided versions must be non-blank, at most 128 bytes, safe for a single URL path segment, and cannot be `"."` or `".."`. A partial unique index prevents duplicate visible versions within one workflow, so a soft-deleted version name may be reused.

## Graph storage

The `graph` column stores the complete React Flow JSON document. The backend treats it as an opaque string — no structural validation is performed at this layer. Validation and compilation belong to the future Workflow Runtime.

## Handlers

The `workflow` module exposes the full set of CRUD and lifecycle handlers, all following the existing port-adapter pattern with `WorkflowRepository`, `WorkflowIdGenerator`, and `Clock`:

| Handler | Purpose |
| --- | --- |
| `CreateWorkflowHandler` | Create workflow with initial draft |
| `GetWorkflowHandler` | Fetch workflow + draft + published snapshot |
| `ListWorkflowsHandler` | List visible workflows without graph data |
| `UpdateWorkflowHandler` | Rename workflow |
| `DeleteWorkflowHandler` | Soft-delete workflow with cascade |
| `GetDraftHandler` | Fetch draft snapshot with graph |
| `UpdateDraftHandler` | Mutate draft graph in-place |
| `PublishWorkflowHandler` | Freeze draft as immutable snapshot and activate |
| `RollbackWorkflowHandler` | Copy historical graph into draft |
| `ActivateWorkflowHandler` | Switch published pointer and sync draft |
| `ListVersionsHandler` | List published version summaries |
| `GetVersionHandler` | Fetch a specific snapshot by version string |
| `DeleteSnapshotHandler` | Soft-delete a published snapshot (constrained) |

Unlike project and task, workflow deletion follows the standard CRUD handler pattern rather than a separate cascade repository, because the deletion constraints are simpler (no running-session check).

## Boundaries (non-goals)

- Workflow execution, node-level run records, runtime variables, and checkpoints belong to the future Workflow Runtime layer.
- Graph validation and React Flow node-type compilation are not part of this layer.
- Tauri command registration and web-server route wiring are transport concerns owned by the respective adapters.

See [Domain Models](domain-models.md), [Application and Contracts Boundary](application-contracts.md), [Database Repositories](database-repositories.md).
