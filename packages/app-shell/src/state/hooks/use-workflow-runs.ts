import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  parseWorkflowGraph,
  projectNodeStatus,
  projectRunStatus,
  type GraphWorkflowNodeState,
  type GraphWorkflowRun,
  type WorkflowDefinition,
} from "@ora/workflow-runtime";
import { useContractsClient } from "../../contracts-client-context";
import { isTerminalRunStatus } from "../../features/workflow-run/run-status-style";

const runsByWorkflowKey = (workflowId: string) => ["workflowRun", "byWorkflow", workflowId] as const;
const runsByProjectKey = (projectId: string) => ["workflowRun", "byProject", projectId] as const;
const runDetailKey = (runId: string) => ["workflowRun", "detail", runId] as const;

/**
 * Lists the runs of one workflow so the deploy dialog can derive the projects the
 * workflow already runs in (a run-task's project is the deploy target).
 */
export function useWorkflowRunsByWorkflow(workflowId: string | null | undefined) {
  const client = useContractsClient();
  return useQuery({
    queryKey: runsByWorkflowKey(workflowId ?? ""),
    queryFn: async () => (await client.workflowRun.listByWorkflow({ workflowId: workflowId! })).runs,
    enabled: workflowId != null && workflowId !== "",
  });
}

/** Lists the persisted workflow runs of one project. */
export function useWorkflowRunsByProject(projectId: string | null | undefined) {
  const client = useContractsClient();
  return useQuery({
    queryKey: runsByProjectKey(projectId ?? ""),
    queryFn: async () => (await client.workflowRun.list({ projectId: projectId! })).runs,
    enabled: projectId != null && projectId !== "",
  });
}

/** Creates one pending workflow run against a published snapshot with a required name. */
export function useCreateWorkflowRun() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: {
      projectId: string;
      workflowId: string;
      name: string;
      baseBranch?: string;
    }) => client.workflowRun.create(input),
    onSuccess: (_result, variables) => {
      void queryClient.invalidateQueries({ queryKey: runsByProjectKey(variables.projectId) });
      void queryClient.invalidateQueries({ queryKey: runsByWorkflowKey(variables.workflowId) });
    },
  });
}

/** Soft-deletes one non-active workflow run and refreshes its project's run list. */
export function useDeleteWorkflowRun() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { runId: string; projectId?: string }) =>
      client.workflowRun.delete({ runId: input.runId }),
    onSuccess: (_result, variables) => {
      if (variables.projectId != null) {
        void queryClient.invalidateQueries({ queryKey: runsByProjectKey(variables.projectId) });
      }
    },
  });
}

/** Starts one pending workflow run through the execution engine. */
export function useStartWorkflowRun() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { runId: string }) => client.workflowRun.start(input),
    onSuccess: (_result, variables) => {
      void queryClient.invalidateQueries({ queryKey: runDetailKey(variables.runId) });
    },
  });
}

/** Cancels one running workflow run through the execution engine. */
export function useCancelWorkflowRun() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { runId: string }) => client.workflowRun.cancel(input),
    onSuccess: (_result, variables) => {
      void queryClient.invalidateQueries({ queryKey: runDetailKey(variables.runId) });
    },
  });
}

/** Restarts one finished workflow run through the execution engine. */
export function useRestartWorkflowRun() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { runId: string }) => client.workflowRun.restart(input),
    onSuccess: (_result, variables) => {
      void queryClient.invalidateQueries({ queryKey: runDetailKey(variables.runId) });
    },
  });
}

/** Sets the kickoff input of a pending run, used as the start node's input on start. */
export function useUpdateWorkflowRunInput() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { runId: string; input: string }) =>
      client.workflowRun.updateInput({ runId: input.runId, input: input.input }),
    onSuccess: (_result, variables) => {
      void queryClient.invalidateQueries({ queryKey: runDetailKey(variables.runId) });
    },
  });
}

/**
 * Renames one persisted workflow run through its run-task title.
 *
 * The run's display name is the run-task title, so the adapter resolves the run-task id
 * from the run detail and updates the task while preserving its status.
 */
export function useRenameWorkflowRun() {
  const client = useContractsClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: { runId: string; name: string }) => {
      const detail = await client.workflowRun.get({ runId: input.runId });
      const task = await client.task.get({ taskId: detail.taskId });
      await client.task.update({
        taskId: detail.taskId,
        title: input.name,
        status: task.task.status,
      });
      void queryClient.invalidateQueries({ queryKey: runDetailKey(input.runId) });
      return input.name;
    },
  });
}

/**
 * Loads one persisted workflow run and projects it into the frontend display model.
 *
 * The backend run is lean (no name, graph, or node state), so the adapter composes the run
 * detail with its frozen snapshot graph and node-runs to satisfy the Theater/Overview canvas.
 * `taskId` is the run-task that owns the Git worktree used by Task Diff.
 */
export function useRealWorkflowRun(runId: string | null | undefined) {
  const client = useContractsClient();
  return useQuery({
    queryKey: runDetailKey(runId ?? ""),
    queryFn: async (): Promise<RealWorkflowRunDetail> => {
      const detail = await client.workflowRun.get({ runId: runId! });
      const { snapshot } = await client.workflow.getSnapshot({
        snapshotId: detail.run.snapshotId,
      });
      return {
        run: buildDisplayRun(detail, snapshot.graph),
        taskId: detail.taskId,
      };
    },
    enabled: runId != null && runId !== "",
    // Poll while the run is still executing so status, node states, and reasons stay live.
    refetchInterval: (query) =>
      isTerminalRunStatus(query.state.data?.run?.status ?? "pending") ? false : 1500,
  });
}

/** Persisted run detail plus the run-task id used for worktree Diff / Files. */
export type RealWorkflowRunDetail = {
  run: GraphWorkflowRun;
  taskId: string;
};

/** Projects a persisted run detail onto the Theater/Overview display model. */
export function buildDisplayRun(
  detail: {
    run: { id: string; workflowId: string; status: string; state: string | null; input: string | null; startedAt: bigint | null; finishedAt: bigint | null; createdAt: bigint; updatedAt: bigint };
    name: string;
    nodes: Array<{
      nodeId: string;
      status: string;
      startedAt: bigint | null;
      finishedAt: bigint | null;
      error: string | null;
      output: string | null;
      payload: string | null;
      sessionId?: string | null;
    }>;
  },
  graph: string,
): GraphWorkflowRun {
  const envelope = parseWorkflowGraph(graph);
  const currentNodes = parseCurrentNodes(detail.run.state);
  // The start node's instruction is the run's kickoff input. Editing it on a pending run stores
  // the value on the run, not on the frozen snapshot, so overlay the committed run input on the
  // start node (falling back to the snapshot instruction until an input has been saved).
  const kickoffInput = detail.run.input;
  const nodes = kickoffInput != null
    ? envelope.nodes.map((node) => (
      node.data.kind === "start"
        ? { ...node, data: { ...node.data, instruction: kickoffInput } }
        : node
    ))
    : envelope.nodes;
  const definitionSnapshot: WorkflowDefinition = {
    id: detail.run.workflowId,
    name: detail.name,
    description: envelope.description ?? "",
    updatedAt: toIso(detail.run.updatedAt),
    viewport: envelope.viewport,
    nodes,
    edges: envelope.edges,
  };
  const nodeRunByNodeId = new Map(detail.nodes.map((node) => [node.nodeId, node]));
  const nodeStates: Record<string, GraphWorkflowNodeState> = {};
  for (const node of definitionSnapshot.nodes) {
    const nodeRun = nodeRunByNodeId.get(node.id) ?? null;
    const payload = nodeRun?.payload != null ? parseNodePayload(nodeRun.payload) : null;
    const durationMs = nodeRun?.startedAt != null && nodeRun?.finishedAt != null
      ? Number(nodeRun.finishedAt - nodeRun.startedAt)
      : undefined;
    nodeStates[node.id] = {
      status: projectNodeStatus(
        nodeRun as { status: "pending" | "running" | "succeeded" | "failed" | "cancelled" } | null,
      ),
      ...(nodeRun?.sessionId != null && nodeRun.sessionId !== ""
        ? { sessionId: nodeRun.sessionId }
        : {}),
      ...(nodeRun?.startedAt != null ? { startedAt: toIso(nodeRun.startedAt) } : {}),
      ...(nodeRun?.finishedAt != null ? { finishedAt: toIso(nodeRun.finishedAt) } : {}),
      ...(durationMs != undefined ? { durationMs } : {}),
      ...(nodeRun?.error != null ? { errorMessage: nodeRun.error } : {}),
      ...(payload?.stop_reason != null ? { stopReason: payload.stop_reason } : {}),
      ...(payload?.token_usage?.used != null
        ? { tokenUsage: { totalTokens: payload.token_usage.used } }
        : {}),
      ...(nodeRun?.output != null ? { output: { summary: nodeRun.output } } : {}),
    };
  }
  const runDurationMs = detail.run.startedAt != null && detail.run.finishedAt != null
    ? Number(detail.run.finishedAt - detail.run.startedAt)
    : undefined;
  return {
    id: detail.run.id,
    projectId: "",
    definitionId: detail.run.workflowId,
    definitionSnapshot,
    name: detail.name,
    status: projectRunStatus(detail.run.status as "pending" | "running" | "succeeded" | "failed" | "cancelled", currentNodes),
    kickoffInput: kickoffInput ?? undefined,
    nodeStates,
    openHitls: [],
    totals: runDurationMs != undefined ? { durationMs: runDurationMs } : {},
    createdAt: toIso(detail.run.createdAt),
    updatedAt: toIso(detail.run.updatedAt),
    ...(detail.run.finishedAt != null ? { finishedAt: toIso(detail.run.finishedAt) } : {}),
  };
}

/** Reads the ACP stop reason and token usage from a node run's `payload` JSON, tolerating
 * malformed payloads. */
function parseNodePayload(
  payload: string,
): { stop_reason?: string; token_usage?: { used?: number } } | null {
  try {
    const value = JSON.parse(payload) as {
      stop_reason?: unknown;
      token_usage?: { used?: unknown };
    };
    return {
      ...(typeof value.stop_reason === "string" ? { stop_reason: value.stop_reason } : {}),
      ...(value.token_usage != null && typeof value.token_usage.used === "number"
        ? { token_usage: { used: value.token_usage.used } }
        : {}),
    };
  } catch {
    return null;
  }
}

/** Parses the run's `{"current_nodes":[...]}` state blob into a node-id list. */
function parseCurrentNodes(state: string | null): string[] {
  if (state == null) {
    return [];
  }
  try {
    const parsed: unknown = JSON.parse(state);
    const nodes = (parsed as { current_nodes?: unknown })?.current_nodes;
    return Array.isArray(nodes) ? nodes.filter((node): node is string => typeof node === "string") : [];
  } catch {
    return [];
  }
}

/** Converts a backend epoch-millis timestamp into the editor's ISO string form. */
function toIso(millis: bigint): string {
  return new Date(Number(millis)).toISOString();
}
