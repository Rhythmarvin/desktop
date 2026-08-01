import type { DemoWorkflow } from "@ora/workflow-mock";
import {
  createDefaultMockPathPolicy,
  nodeKindUsesTokens,
  planMockExecution,
  topologicalOrder,
  type MockExecutionPlan,
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
  /** Duration of each node step. Default 5000ms so Theater switching is tryable. */
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
 * Mock executor over a frozen DemoWorkflow snapshot.
 * Plans a reachable path (condition = exclusive), then runs ready nodes in
 * parallel waves: every node whose predecessors have succeeded starts together.
 * Per-node `data.mockStepMs` overrides the default step duration so staggered
 * starts/ends can be demonstrated.
 */
export function createMockRunEngine(
  host: MockRunEngineHost,
  options: MockRunEngineOptions = {},
) {
  const nodeStepMs = options.nodeStepMs ?? 5_000;
  const pathPolicy = options.pathPolicy ?? createDefaultMockPathPolicy();
  /** Per-run map of nodeId → in-flight step timer. */
  const timers = new Map<string, Map<string, ReturnType<typeof setTimeout>>>();
  const plans = new Map<string, MockExecutionPlan>();

  /** Resolves step length: node mockStepMs when positive, else engine default. */
  function stepMsFor(run: GraphWorkflowRun, nodeId: string): number {
    const node = run.definitionSnapshot.nodes.find((item) => item.id === nodeId);
    const custom = node?.data.mockStepMs;
    if (typeof custom === "number" && Number.isFinite(custom) && custom > 0) {
      return custom;
    }
    return nodeStepMs;
  }

  /** Clears every pending step timer for a run (cancel / delete). */
  function stop(runId: string): void {
    const byNode = timers.get(runId);
    if (byNode !== undefined) {
      for (const timer of byNode.values()) {
        clearTimeout(timer);
      }
      timers.delete(runId);
    }
    plans.delete(runId);
  }

  function timersFor(runId: string): Map<string, ReturnType<typeof setTimeout>> {
    let byNode = timers.get(runId);
    if (byNode === undefined) {
      byNode = new Map();
      timers.set(runId, byNode);
    }
    return byNode;
  }

  /**
   * Starts every currently ready idle node. When nothing is left to run and no
   * timers remain, finishes the run as succeeded.
   */
  function pump(runId: string): void {
    const run = host.getRun(runId);
    const plan = plans.get(runId);
    if (run === undefined || plan === undefined || isTerminal(run.status)) {
      return;
    }

    const ready = plan.order.filter((nodeId) => {
      const state = run.nodeStates[nodeId];
      if (state === undefined || state.status !== "idle") {
        return false;
      }
      if (timersFor(runId).has(nodeId)) {
        return false;
      }
      const preds = plan.predecessors[nodeId] ?? [];
      return preds.every((predId) => {
        const pred = run.nodeStates[predId];
        return pred?.status === "succeeded" || pred?.status === "skipped";
      });
    });

    for (const nodeId of ready) {
      beginNode(runId, nodeId);
    }

    const latest = host.getRun(runId);
    if (latest === undefined || isTerminal(latest.status)) {
      return;
    }

    const allDone = plan.order.every((nodeId) => {
      const status = latest.nodeStates[nodeId]?.status;
      return (
        status === "succeeded"
        || status === "skipped"
        || status === "failed"
        || status === "cancelled"
      );
    });
    if (allDone && timersFor(runId).size === 0) {
      finishRun(runId, /*status*/ "succeeded");
    }
  }

  function beginNode(runId: string, nodeId: string): void {
    const run = host.getRun(runId);
    if (run === undefined || isTerminal(run.status)) {
      return;
    }
    if (run.nodeStates[nodeId]?.status !== "idle") {
      return;
    }
    const startedAt = host.nowIso();
    const stepMs = stepMsFor(run, nodeId);
    patchNode(runId, nodeId, {
      status: "running",
      startedAt,
    });
    host.emit(runId, { type: "node_started", runId, nodeId });

    const timer = setTimeout(() => {
      timersFor(runId).delete(nodeId);
      const current = host.getRun(runId);
      if (current === undefined || current.status === "cancelled") {
        return;
      }
      completeNode(runId, nodeId, startedAt, stepMs);
      pump(runId);
    }, stepMs);
    timersFor(runId).set(nodeId, timer);
  }

  function completeNode(
    runId: string,
    nodeId: string,
    startedAt: string,
    stepMs: number,
  ): void {
    const run = host.getRun(runId);
    if (run === undefined) {
      return;
    }
    const finishedAt = host.nowIso();
    const durationMs = Math.max(stepMs, 1);
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
    plans.set(runId, plan);

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
    pump(runId);
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
