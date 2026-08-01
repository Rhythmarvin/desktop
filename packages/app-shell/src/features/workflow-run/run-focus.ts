import type { GraphWorkflowNodeStatus, GraphWorkflowRun } from "./runtime/types";

const ACTIVE_STATUSES: ReadonlySet<GraphWorkflowNodeStatus> = new Set([
  "running",
  "awaiting_input",
]);

export interface TheaterFocus {
  /** Primary act shown large on stage. */
  primaryId: string | null;
  /**
   * All currently active acts (running + awaiting_input), snapshot order.
   * Length > 1 means genuine parallelism from the UI's point of view.
   */
  activeIds: string[];
}

function isActiveStatus(status: GraphWorkflowNodeStatus): boolean {
  return ACTIVE_STATUSES.has(status);
}

/** Snapshot order keeps parallel chips stable across engine patches. */
function orderedActiveIds(run: GraphWorkflowRun): string[] {
  return run.definitionSnapshot.nodes
    .map((node) => node.id)
    .filter((nodeId) => {
      const state = run.nodeStates[nodeId];
      return state !== undefined && isActiveStatus(state.status);
    });
}

/**
 * Among active acts, prefer awaiting_input, then latest startedAt, then snapshot order.
 */
function pickPrimaryAmongActive(
  run: GraphWorkflowRun,
  activeIds: string[],
): string | null {
  if (activeIds.length === 0) {
    return null;
  }

  const awaiting = activeIds.filter(
    (nodeId) => run.nodeStates[nodeId]?.status === "awaiting_input",
  );
  const pool = awaiting.length > 0 ? awaiting : activeIds;

  let bestId = pool[0]!;
  let bestStarted = run.nodeStates[bestId]?.startedAt ?? "";
  for (const nodeId of pool.slice(1)) {
    const started = run.nodeStates[nodeId]?.startedAt ?? "";
    if (started.localeCompare(bestStarted) > 0) {
      bestId = nodeId;
      bestStarted = started;
    }
  }
  return bestId;
}

function pickFallbackPrimary(run: GraphWorkflowRun): string | null {
  let latestSucceeded: { nodeId: string; finishedAt: string } | null = null;
  for (const node of run.definitionSnapshot.nodes) {
    const state = run.nodeStates[node.id];
    if (state?.status === "succeeded" && state.finishedAt !== undefined) {
      if (
        latestSucceeded === null
        || state.finishedAt.localeCompare(latestSucceeded.finishedAt) > 0
      ) {
        latestSucceeded = { nodeId: node.id, finishedAt: state.finishedAt };
      }
    }
  }
  if (latestSucceeded !== null) {
    return latestSucceeded.nodeId;
  }
  return run.definitionSnapshot.nodes[0]?.id ?? null;
}

/**
 * Resolves Theater spotlight under sequential or parallel execution.
 *
 * - `activeIds`: every running / awaiting_input node (may be many).
 * - `primaryId`: user preference if still valid; else policy among actives;
 *   else last succeeded / first node.
 */
export function resolveTheaterFocus(
  run: GraphWorkflowRun,
  preferredNodeId: string | null,
): TheaterFocus {
  const activeIds = orderedActiveIds(run);

  if (
    preferredNodeId !== null
    && run.nodeStates[preferredNodeId] !== undefined
  ) {
    // Keep user focus even after the act leaves "active", until they pick another.
    return { primaryId: preferredNodeId, activeIds };
  }

  const primaryId = pickPrimaryAmongActive(run, activeIds)
    ?? pickFallbackPrimary(run);
  return { primaryId, activeIds };
}

/** @deprecated Prefer resolveTheaterFocus — kept for older call sites. */
export function resolveFocusNodeId(
  run: GraphWorkflowRun,
  preferredNodeId: string | null,
): string | null {
  return resolveTheaterFocus(run, preferredNodeId).primaryId;
}
