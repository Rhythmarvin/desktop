import { useTranslation } from "react-i18next";
import { Badge, cn } from "@ora/ui";
import {
  createMockWorkflowNodeType,
  type WorkflowNodeData,
} from "@ora/workflow-mock";
import { formatRunClock } from "../../lib/format";
import { WorkflowNodeCardShell } from "../workflow-node-chrome";
import { runStatusTone } from "./run-status-style";
import type { GraphWorkflowNodeState } from "./runtime/types";

interface RunTheaterActCardProps {
  nodeId: string;
  data: WorkflowNodeData;
  state: GraphWorkflowNodeState;
  /** Soft emphasis when this act is live (running / awaiting). */
  live: boolean;
  /** Large primary stage vs secondary parallel card. */
  variant?: "stage" | "compact";
  /** Promote this parallel act to primary focus. */
  onSelect?: () => void;
}

/**
 * Theater act card built on shared workflow-node chrome.
 * Stage = primary spotlight; compact kept for denser secondary surfaces.
 */
export function RunTheaterActCard({
  nodeId,
  data,
  state,
  live,
  variant = "stage",
  onSelect,
}: RunTheaterActCardProps) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage === "en-US" ? "en-US" as const : "zh-CN" as const;
  const kindLabel = createMockWorkflowNodeType(data.kind, locale).label;
  const tone = runStatusTone(state.status);
  const detail = data.model ?? data.tool ?? data.condition;
  const compact = variant === "compact";
  const timingRange = state.startedAt !== undefined || state.finishedAt !== undefined
    ? [
      state.startedAt !== undefined
        ? formatRunClock(state.startedAt, locale)
        : "—",
      state.finishedAt !== undefined
        ? formatRunClock(state.finishedAt, locale)
        : "—",
    ].join(" → ")
    : null;

  const metrics = (
    <div className="space-y-2.5">
      {timingRange !== null && (
        <p className="text-[10px] tabular-nums text-muted-foreground/65">
          {timingRange}
        </p>
      )}
      <dl className={cn("grid gap-3", compact ? "grid-cols-2" : "sm:grid-cols-3")}>
        {!compact && (
          <div className="rounded-lg border border-border/70 bg-background/80 px-3 py-2.5">
            <dt className="text-[10px] text-muted-foreground">
              {t("workflowRun.theater.nodeId")}
            </dt>
            <dd className="mt-0.5 truncate font-mono text-xs">{nodeId}</dd>
          </div>
        )}
        <div className="rounded-lg border border-border/70 bg-background/80 px-3 py-2.5">
          <dt className="text-[10px] text-muted-foreground">
            {t("workflowRun.field.duration")}
          </dt>
          <dd className="mt-0.5 text-xs tabular-nums">
            {state.durationMs !== undefined
              ? t("workflowRun.totalsDuration", { ms: state.durationMs })
              : "—"}
          </dd>
        </div>
        <div className="rounded-lg border border-border/70 bg-background/80 px-3 py-2.5">
          <dt className="text-[10px] text-muted-foreground">
            {t("workflowRun.field.tokens")}
          </dt>
          <dd className="mt-0.5 text-xs tabular-nums">
            {state.tokenUsage?.totalTokens !== undefined
              ? t("workflowRun.totalsTokens", {
                count: state.tokenUsage.totalTokens,
              })
              : "—"}
          </dd>
        </div>
      </dl>
    </div>
  );

  return (
    <WorkflowNodeCardShell
      kind={data.kind}
      title={data.title}
      description={data.description}
      kindLabel={kindLabel}
      density={compact ? "compact" : "stage"}
      className={cn(
        compact ? "w-full" : "mx-auto w-full max-w-xl",
        onSelect && "cursor-pointer hover:border-foreground/30",
      )}
      ariaLabel={`${data.title}: ${t(tone.labelKey)}`}
      aria-live={compact ? undefined : "polite"}
      frameClassName={cn(
        tone.ring,
        "ring-2",
        live && state.status === "running" && "motion-safe:shadow-md",
      )}
      iconAccessory={(
        <span
          className={cn(
            "absolute -right-0.5 -top-0.5 rounded-full ring-2 ring-card",
            compact ? "size-2" : "size-2.5",
            tone.dot,
            live && state.status === "running" && "motion-safe:animate-pulse",
          )}
          aria-hidden
        />
      )}
      headerAccessory={(
        <Badge variant="outline" className={cn("border", tone.badge)}>
          {t(tone.labelKey)}
        </Badge>
      )}
      body={compact
        ? (
          <p className="mt-1 line-clamp-2 text-[11px] leading-4 text-muted-foreground">
            {data.description}
          </p>
        )
        : (
          <>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              {data.description}
            </p>
            <div className="mt-5 rounded-xl border border-border/80 bg-muted/30 px-4 py-3">
              <p className="text-[11px] font-medium uppercase tracking-[0.04em] text-muted-foreground">
                {t("workflowRun.theater.instruction")}
              </p>
              <p className="mt-1.5 text-sm leading-6 text-foreground/90">
                {data.instruction}
              </p>
              {detail !== undefined && detail !== "" && (
                <p className="mt-2 font-mono text-[11px] text-muted-foreground">
                  {detail}
                </p>
              )}
            </div>
          </>
        )}
      footer={metrics}
      onClick={onSelect}
      onKeyDown={onSelect
        ? (event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelect();
          }
        }
        : undefined}
      role={onSelect ? "button" : undefined}
      tabIndex={onSelect ? 0 : undefined}
    />
  );
}
