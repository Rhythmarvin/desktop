import type { DemoWorkflow } from "@ora/workflow-mock";
import {
  createDefaultMockPathPolicy,
  nodeKindUsesTokens,
  planMockExecution,
  topologicalOrder,
  type MockPathPolicy,
} from "./mock-execution-plan";
import type {
  GraphWorkflowNodeState,
  GraphWorkflowRun,
  GraphWorkflowTokenUsage,
  WorkflowArtifact,
  WorkflowRunEvent,
} from "./types";

export interface MockRunEngineOptions {
  /** Delay between node start and finish. Default 450ms. */
  nodeStepMs?: number;
  /** Condition path selection; defaults to kickoff-aware label heuristics. */
  pathPolicy?: MockPathPolicy;
}

export interface MockRunEngineHost {
  getRun: (runId: string) => GraphWorkflowRun | undefined;
  setRun: (run: GraphWorkflowRun) => void;
  appendArtifact: (artifact: WorkflowArtifact) => void;
  emit: (runId: string, event: WorkflowRunEvent) => void;
  notifyChanged: (run: GraphWorkflowRun) => void;
  nowIso: () => string;
  nextArtifactId: () => string;
}

/**
 * Deterministic sequential executor over a frozen DemoWorkflow snapshot.
 * Plans a reachable path (condition = exclusive), then walks that order.
 */
export function createMockRunEngine(
  host: MockRunEngineHost,
  options: MockRunEngineOptions = {},
) {
  const nodeStepMs = options.nodeStepMs ?? 450;
  const pathPolicy = options.pathPolicy ?? createDefaultMockPathPolicy();
  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  /** Clears any pending step timer for a run (cancel / delete). */
  function stop(runId: string): void {
    const timer = timers.get(runId);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.delete(runId);
    }
  }

  /** Schedules the next node, or finishes the run when the queue is empty. */
  function scheduleNext(runId: string, order: string[], index: number): void {
    stop(runId);
    const run = host.getRun(runId);
    if (run === undefined || isTerminal(run.status)) {
      return;
    }

    if (index >= order.length) {
      finishRun(runId, /*status*/ "succeeded");
      return;
    }

    const nodeId = order[index]!;
    const startedAt = host.nowIso();
    patchNode(runId, nodeId, {
      status: "running",
      startedAt,
    });
    host.emit(runId, { type: "node_started", runId, nodeId });

    const timer = setTimeout(() => {
      timers.delete(runId);
      const current = host.getRun(runId);
      if (current === undefined || current.status === "cancelled") {
        return;
      }
      completeNode(runId, nodeId, startedAt);
      scheduleNext(runId, order, index + 1);
    }, nodeStepMs);
    timers.set(runId, timer);
  }

  function completeNode(runId: string, nodeId: string, startedAt: string): void {
    const run = host.getRun(runId);
    if (run === undefined) {
      return;
    }
    const finishedAt = host.nowIso();
    const durationMs = Math.max(nodeStepMs, 1);
    const node = run.definitionSnapshot.nodes.find((item) => item.id === nodeId);
    const tokenUsage = node && nodeKindUsesTokens(node.data.kind)
      ? stubTokenUsage(nodeId)
      : undefined;
    patchNode(runId, nodeId, {
      status: "succeeded",
      startedAt,
      finishedAt,
      durationMs,
      tokenUsage,
    });
    host.emit(runId, {
      type: "node_finished",
      runId,
      nodeId,
      status: "succeeded",
      durationMs,
      tokenUsage,
    });

    // Surface a light artifact on agent/output nodes so Step 4 UI has something to show.
    if (node?.data.kind === "agent" || node?.data.kind === "output") {
      const artifact: WorkflowArtifact = {
        id: host.nextArtifactId(),
        runId,
        nodeId,
        kind: "markdown",
        title: node.data.title,
        body:
          node.data.kind === "output"
            ? `## ${node.data.title}\n\nMock run completed for **${run.name}**.`
            : `### ${node.data.title}\n\n${node.data.instruction}`,
        createdAt: finishedAt,
      };
      host.appendArtifact(artifact);
      host.emit(runId, { type: "artifact_added", runId, artifact });
    }
  }

  function patchNode(
    runId: string,
    nodeId: string,
    patch: GraphWorkflowNodeState,
  ): void {
    const run = host.getRun(runId);
    if (run === undefined) {
      return;
    }
    const updated: GraphWorkflowRun = {
      ...run,
      nodeStates: {
        ...run.nodeStates,
        [nodeId]: { ...run.nodeStates[nodeId], ...patch },
      },
      updatedAt: host.nowIso(),
    };
    host.setRun(updated);
    host.notifyChanged(updated);
  }

  function finishRun(
    runId: string,
    status: "succeeded" | "failed" | "cancelled",
  ): void {
    stop(runId);
    const run = host.getRun(runId);
    if (run === undefined) {
      return;
    }
    const finishedAt = host.nowIso();
    let totalTokens = 0;
    let durationMs = 0;
    for (const state of Object.values(run.nodeStates)) {
      totalTokens += state.tokenUsage?.totalTokens ?? 0;
      durationMs += state.durationMs ?? 0;
    }
    const totals = {
      durationMs,
      tokenUsage: totalTokens > 0 ? { totalTokens } : {},
    };
    const updated: GraphWorkflowRun = {
      ...run,
      status,
      totals,
      updatedAt: finishedAt,
      finishedAt,
    };
    host.setRun(updated);
    host.notifyChanged(updated);
    host.emit(runId, { type: "run_finished", runId, status, totals });
  }

  /**
   * Begins execution from `pending` only (re-entrant start is a no-op).
   * HITL resume will use a dedicated API later, not this method.
   */
  function start(runId: string): void {
    const run = host.getRun(runId);
    if (run === undefined || run.status !== "pending") {
      return;
    }
    stop(runId);
    const plan = planMockExecution(
      run.definitionSnapshot,
      { kickoffInput: run.kickoffInput },
      pathPolicy,
    );

    const nodeStates = { ...run.nodeStates };
    for (const nodeId of plan.skipped) {
      nodeStates[nodeId] = { status: "skipped" };
    }
    const started: GraphWorkflowRun = {
      ...run,
      status: "running",
      nodeStates,
      updatedAt: host.nowIso(),
    };
    host.setRun(started);
    host.notifyChanged(started);
    host.emit(runId, { type: "run_started", runId });
    for (const nodeId of plan.skipped) {
      host.emit(runId, {
        type: "node_finished",
        runId,
        nodeId,
        status: "skipped",
      });
    }
    scheduleNext(runId, plan.order, 0);
  }

  /** Stops timers, marks active nodes cancelled, and emits run_finished. */
  function cancel(runId: string): void {
    stop(runId);
    const run = host.getRun(runId);
    if (run === undefined || isTerminal(run.status)) {
      return;
    }
    const finishedAt = host.nowIso();
    const nodeStates = { ...run.nodeStates };
    for (const [nodeId, state] of Object.entries(nodeStates)) {
      if (state.status === "running" || state.status === "awaiting_input") {
        nodeStates[nodeId] = { ...state, status: "cancelled", finishedAt };
      }
    }
    host.setRun({
      ...run,
      nodeStates,
      updatedAt: finishedAt,
    });
    finishRun(runId, "cancelled");
  }

  return { start, stop, cancel };
}

function isTerminal(status: GraphWorkflowRun["status"]): boolean {
  return (
    status === "succeeded"
    || status === "failed"
    || status === "cancelled"
    || status === "partial_failed"
  );
}

function stubTokenUsage(nodeId: string): GraphWorkflowTokenUsage {
  return {
    inputTokens: 40 + nodeId.length * 3,
    outputTokens: 60 + nodeId.length * 2,
    totalTokens: 100 + nodeId.length * 5,
  };
}

/**
 * Full-graph topological order (does not apply condition exclusivity).
 * Prefer `planMockExecution` when simulating a run.
 */
export function executionOrder(workflow: DemoWorkflow): string[] {
  return topologicalOrder(
    workflow.nodes.map((node) => node.id),
    workflow.edges,
  );
}
