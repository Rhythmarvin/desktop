import { createContext, memo, useContext, type ReactNode } from "react";
import {
  Handle,
  Position,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { useTranslation } from "react-i18next";
import { cn } from "@ora/ui";
import {
  createMockWorkflowNodeType,
  WORKFLOW_NODE_ANCHOR_Y,
  WORKFLOW_NODE_WIDTH,
  type WorkflowNodeData,
} from "@ora/workflow-mock";
import { formatRunClock } from "../../lib/format";
import { WorkflowNodeCardShell } from "../workflow-node-chrome";
import { runStatusTone } from "./run-status-style";
import type { GraphWorkflowNodeState } from "./runtime/types";

export type RunOverviewNodeData = WorkflowNodeData & {
  runStatus: GraphWorkflowNodeState["status"];
};

interface RunOverviewStatusMap {
  states: Record<string, GraphWorkflowNodeState>;
  focusedNodeId: string | null;
  activeNodeIds: string[];
}

const RunOverviewStatusContext = createContext<RunOverviewStatusMap>({
  states: {},
  focusedNodeId: null,
  activeNodeIds: [],
});

/** Provides live nodeStates to overview node renderers. */
export function RunOverviewStatusProvider({
  states,
  focusedNodeId,
  activeNodeIds,
  children,
}: RunOverviewStatusMap & { children: ReactNode }) {
  return (
    <RunOverviewStatusContext.Provider
      value={{ states, focusedNodeId, activeNodeIds }}
    >
      {children}
    </RunOverviewStatusContext.Provider>
  );
}

/**
 * Read-only run graph card on shared chrome + execution status overlay.
 */
export const RunOverviewNode = memo(function RunOverviewNode({
  id,
  data,
  selected,
}: NodeProps<Node<RunOverviewNodeData, "workflow">>) {
  const { i18n, t } = useTranslation();
  const { states, focusedNodeId, activeNodeIds } = useContext(
    RunOverviewStatusContext,
  );
  const locale = i18n.resolvedLanguage === "en-US" ? "en-US" as const : "zh-CN" as const;
  const state = states[id] ?? { status: "idle" as const };
  const tone = runStatusTone(state.status);
  const kindLabel = createMockWorkflowNodeType(data.kind, locale).label;
  const focused = focusedNodeId === id || selected;
  const active = activeNodeIds.includes(id);
  const startedLabel = state.startedAt !== undefined
    ? formatRunClock(state.startedAt, locale)
    : null;
  const finishedLabel = state.finishedAt !== undefined
    ? formatRunClock(state.finishedAt, locale)
    : null;
  const hasTiming = startedLabel !== null || finishedLabel !== null;
  const hasMetrics = hasTiming
    || state.durationMs !== undefined
    || state.tokenUsage?.totalTokens !== undefined;

  return (
    <WorkflowNodeCardShell
      data-workflow-run-node=""
      kind={data.kind}
      title={data.title}
      description={data.description}
      kindLabel={kindLabel}
      density="run"
      selected={focused}
      width={WORKFLOW_NODE_WIDTH * 0.92}
      ariaLabel={`${data.title}: ${t(tone.labelKey)}`}
      frameClassName={cn(
        tone.ring,
        "ring-2",
        state.status === "skipped" && "opacity-55",
        active && state.status === "running" && "motion-safe:shadow-md",
      )}
      iconAccessory={(
        <span
          className={cn(
            "absolute -right-0.5 -top-0.5 size-2 rounded-full ring-2 ring-card",
            tone.dot,
            state.status === "running" && "motion-safe:animate-pulse",
          )}
          aria-hidden
        />
      )}
      headerAccessory={(
        <span
          className={cn(
            "shrink-0 rounded px-1.5 py-0.5 text-[9px] font-medium",
            tone.badge,
          )}
        >
          {t(tone.labelKey)}
        </span>
      )}
      footer={hasMetrics
        ? (
          <p className="font-mono text-[9px] tabular-nums text-muted-foreground">
            {state.durationMs !== undefined ? `${state.durationMs}ms` : ""}
            {state.durationMs !== undefined
              && state.tokenUsage?.totalTokens !== undefined
              ? " · "
              : ""}
            {state.tokenUsage?.totalTokens !== undefined
              ? `${state.tokenUsage.totalTokens} tok`
              : ""}
            {hasTiming && (
              <span className="text-muted-foreground/55">
                {(state.durationMs !== undefined
                  || state.tokenUsage?.totalTokens !== undefined)
                  ? " · "
                  : ""}
                {startedLabel ?? "—"}
                –
                {finishedLabel ?? "—"}
              </span>
            )}
          </p>
        )
        : undefined}
      targetHandle={(
        <Handle
          type="target"
          position={Position.Left}
          className="!size-2 !border-0 !bg-transparent"
          style={{ top: WORKFLOW_NODE_ANCHOR_Y * 0.92 }}
          isConnectable={false}
        />
      )}
      sourceHandle={(
        <Handle
          type="source"
          position={Position.Right}
          className="!size-2 !border-0 !bg-transparent"
          style={{ top: WORKFLOW_NODE_ANCHOR_Y * 0.92 }}
          isConnectable={false}
        />
      )}
    />
  );
});
