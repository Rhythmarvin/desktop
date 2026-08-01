import type { DemoWorkflow } from "@ora/workflow-mock";
import type {
  WorkflowHostRepository,
  WorkflowRunRepository,
  WorkflowRuntime,
} from "./ports";
import type {
  GraphWorkflowNodeState,
  GraphWorkflowRun,
  GraphWorkflowRunStatus,
  ProjectWorkflowMount,
  WorkflowArtifact,
  WorkflowRunEvent,
} from "./types";

type Listener = (event: WorkflowRunEvent) => void;

/** Local-time ISO timestamp for run metadata (Ora prefers local clocks). */
function nowIso(): string {
  const date = new Date();
  const pad = (value: number, width = 2) => String(value).padStart(width, "0");
  const offsetMin = -date.getTimezoneOffset();
  const sign = offsetMin >= 0 ? "+" : "-";
  const abs = Math.abs(offsetMin);
  const offset = `${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}${offset}`;
}

function idleNodeStates(workflow: DemoWorkflow): Record<string, GraphWorkflowNodeState> {
  return Object.fromEntries(
    workflow.nodes.map((node) => [node.id, { status: "idle" as const }]),
  );
}

/**
 * In-memory Host + Run repositories for MVP.
 * Definition blobs live here after deploy; `@ora/workflow-mock` stays free of persistence.
 */
export function createMemoryWorkflowRuntime(): WorkflowRuntime {
  const definitions = new Map<string, DemoWorkflow>();
  const mounts: ProjectWorkflowMount[] = [];
  const runs = new Map<string, GraphWorkflowRun>();
  const artifacts = new Map<string, WorkflowArtifact[]>();
  const listeners = new Map<string, Set<Listener>>();
  let runSeq = 0;

  const emit = (runId: string, event: WorkflowRunEvent) => {
    const set = listeners.get(runId);
    if (set === undefined) {
      return;
    }
    for (const listener of set) {
      listener(event);
    }
  };

  const host: WorkflowHostRepository = {
    async listMounts(projectId) {
      return mounts
        .filter((mount) => mount.projectId === projectId)
        .map((mount) => structuredClone(mount));
    },

    async listMountsByDefinition(definitionId) {
      return mounts
        .filter((mount) => mount.definitionId === definitionId)
        .map((mount) => structuredClone(mount));
    },

    async mount(projectId, definition) {
      definitions.set(definition.id, structuredClone(definition));
      const existing = mounts.findIndex(
        (mount) =>
          mount.projectId === projectId && mount.definitionId === definition.id,
      );
      const next: ProjectWorkflowMount = {
        projectId,
        definitionId: definition.id,
        definitionName: definition.name,
        mountedAt: nowIso(),
      };
      if (existing >= 0) {
        mounts[existing] = next;
      } else {
        mounts.push(next);
      }
      return structuredClone(next);
    },

    async unmount(projectId, definitionId) {
      const index = mounts.findIndex(
        (mount) =>
          mount.projectId === projectId && mount.definitionId === definitionId,
      );
      if (index >= 0) {
        mounts.splice(index, 1);
      }
    },

    async getDefinition(definitionId) {
      const definition = definitions.get(definitionId);
      return definition === undefined ? null : structuredClone(definition);
    },
  };

  const runRepo: WorkflowRunRepository = {
    async list(projectId) {
      return [...runs.values()]
        .filter((run) => run.projectId === projectId)
        .map((run) => structuredClone(run))
        .sort((left, right) => right.createdAt.localeCompare(left.createdAt));
    },

    async get(runId) {
      const run = runs.get(runId);
      return run === undefined ? null : structuredClone(run);
    },

    async create({ projectId, definitionId, kickoffInput }) {
      const mounted = mounts.some(
        (mount) =>
          mount.projectId === projectId && mount.definitionId === definitionId,
      );
      if (!mounted) {
        throw new Error(`Workflow ${definitionId} is not mounted on project ${projectId}`);
      }
      const definition = definitions.get(definitionId);
      if (definition === undefined) {
        throw new Error(`Unknown workflow definition ${definitionId}`);
      }
      // Freeze the graph so later library edits cannot rewrite this run.
      const snapshot = structuredClone(definition);
      runSeq += 1;
      const createdAt = nowIso();
      const run: GraphWorkflowRun = {
        id: `gwr-${runSeq}`,
        projectId,
        definitionId,
        definitionSnapshot: snapshot,
        name: snapshot.name,
        // Step 1 leaves runs pending; Step 2 mock engine advances them.
        status: "pending",
        kickoffInput,
        nodeStates: idleNodeStates(snapshot),
        totals: {},
        createdAt,
        updatedAt: createdAt,
      };
      runs.set(run.id, run);
      artifacts.set(run.id, []);
      return structuredClone(run);
    },

    async cancel(runId) {
      const run = runs.get(runId);
      if (run === undefined) {
        throw new Error(`Unknown workflow run ${runId}`);
      }
      if (
        run.status === "succeeded"
        || run.status === "failed"
        || run.status === "cancelled"
        || run.status === "partial_failed"
      ) {
        return;
      }
      const finishedAt = nowIso();
      const nextStatus: GraphWorkflowRunStatus = "cancelled";
      const nodeStates = { ...run.nodeStates };
      for (const [nodeId, state] of Object.entries(nodeStates)) {
        if (state.status === "running" || state.status === "awaiting_input") {
          nodeStates[nodeId] = { ...state, status: "cancelled", finishedAt };
        }
      }
      const updated: GraphWorkflowRun = {
        ...run,
        status: nextStatus,
        nodeStates,
        updatedAt: finishedAt,
        finishedAt,
      };
      runs.set(runId, updated);
      emit(runId, {
        type: "run_finished",
        runId,
        status: nextStatus,
        totals: updated.totals,
      });
    },

    async delete(runId) {
      const run = runs.get(runId);
      if (run === undefined) {
        return;
      }
      // Cancel in-flight work first; sibling runs keep their own state machines.
      if (
        run.status === "pending"
        || run.status === "running"
        || run.status === "awaiting_input"
      ) {
        await runRepo.cancel(runId);
      }
      runs.delete(runId);
      artifacts.delete(runId);
      listeners.delete(runId);
    },

    async rename(runId, name) {
      const run = runs.get(runId);
      if (run === undefined) {
        throw new Error(`Unknown workflow run ${runId}`);
      }
      const trimmed = name.trim();
      if (trimmed === "") {
        throw new Error("Workflow run name cannot be empty");
      }
      const updated: GraphWorkflowRun = {
        ...run,
        name: trimmed,
        updatedAt: nowIso(),
      };
      runs.set(runId, updated);
      return structuredClone(updated);
    },

    async submitHitl(_runId, _requestId, _payload) {
      // Step 5 wires HITL; keep the port stable for callers.
      throw new Error("HITL is not implemented yet");
    },

    async listArtifacts(runId) {
      return structuredClone(artifacts.get(runId) ?? []);
    },

    subscribe(runId, onEvent) {
      let set = listeners.get(runId);
      if (set === undefined) {
        set = new Set();
        listeners.set(runId, set);
      }
      set.add(onEvent);
      return () => {
        set.delete(onEvent);
        if (set.size === 0) {
          listeners.delete(runId);
        }
      };
    },
  };

  return { host, runs: runRepo };
}
