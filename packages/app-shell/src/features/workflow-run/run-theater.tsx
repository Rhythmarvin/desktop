import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { IconChevronLeft, IconChevronRight } from "@tabler/icons-react";
import { Badge, Button, cn } from "@ora/ui";
import { RunTheaterActCard } from "./run-theater-act-card";
import { resolveTheaterFocus } from "./run-focus";
import { runStatusTone } from "./run-status-style";
import type { GraphWorkflowRun } from "./runtime/types";

interface RunTheaterProps {
  run: GraphWorkflowRun;
  focusNodeId: string | null;
  onFocusNode: (nodeId: string) => void;
}

/** Cycles among parallel active acts (wraps). Falls back to first if focus is elsewhere. */
function cycleParallelActive(
  activeIds: string[],
  currentId: string | null,
  delta: 1 | -1,
): string | null {
  if (activeIds.length === 0) {
    return null;
  }
  const index = currentId === null ? -1 : activeIds.indexOf(currentId);
  if (index < 0) {
    return activeIds[0] ?? null;
  }
  const next = (index + delta + activeIds.length) % activeIds.length;
  return activeIds[next] ?? null;
}

/**
 * Focused act stage + compact path rail.
 * Parallel waves keep a single card; siblings switch via chips / prev-next.
 * Esc (handled by workspace) returns to Overview.
 */
export function RunTheater({
  run,
  focusNodeId,
  onFocusNode,
}: RunTheaterProps) {
  const { t } = useTranslation();
  const focus = useMemo(
    () => resolveTheaterFocus(run, focusNodeId),
    [run, focusNodeId],
  );
  const primaryId = focus.primaryId;
  const parallel = focus.activeIds.length > 1;
  const parallelIndex = primaryId !== null
    ? focus.activeIds.indexOf(primaryId)
    : -1;

  const primaryNode = run.definitionSnapshot.nodes.find(
    (node) => node.id === primaryId,
  );
  const primaryState = primaryId !== null
    ? run.nodeStates[primaryId]
    : undefined;

  const progress = useMemo(() => {
    const states = Object.values(run.nodeStates);
    const total = Math.max(states.length, 1);
    const done = states.filter(
      (state) =>
        state.status === "succeeded"
        || state.status === "skipped"
        || state.status === "failed"
        || state.status === "cancelled",
    ).length;
    return { done, total, percent: Math.round((done / total) * 100) };
  }, [run.nodeStates]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="shrink-0 border-b border-border/80 bg-muted/20 px-4 py-3">
        <div className="mx-auto flex max-w-3xl flex-col gap-2.5">
          <div className="flex items-center justify-between gap-3">
            <p className="text-[11px] font-medium uppercase tracking-[0.05em] text-muted-foreground">
              {t("workflowRun.theater.path")}
            </p>
            <p className="text-[11px] tabular-nums text-muted-foreground">
              {t("workflowRun.progressValue", {
                done: progress.done,
                total: progress.total,
              })}
            </p>
          </div>
          <div
            className="h-1.5 overflow-hidden rounded-full bg-muted"
            role="progressbar"
            aria-valuenow={progress.percent}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={t("workflowRun.field.progress")}
          >
            <div
              className="h-full rounded-full bg-foreground/70 transition-[width] duration-300 ease-out motion-reduce:transition-none"
              style={{ width: `${progress.percent}%` }}
            />
          </div>
          <div className="overflow-x-auto">
            <ol className="flex w-max gap-2 pb-0.5">
              {run.definitionSnapshot.nodes.map((node) => {
                const state = run.nodeStates[node.id] ?? { status: "idle" as const };
                const tone = runStatusTone(state.status);
                const selected = node.id === primaryId;
                const active = focus.activeIds.includes(node.id);
                return (
                  <li key={node.id}>
                    <button
                      type="button"
                      onClick={() => onFocusNode(node.id)}
                      className={cn(
                        "inline-flex max-w-[10rem] cursor-pointer items-center gap-2 rounded-full border px-2.5 py-1.5 text-left transition-colors duration-150",
                        selected
                          ? "border-foreground/35 bg-background shadow-sm"
                          : active
                          ? "border-sky-500/35 bg-sky-500/5"
                          : "border-transparent bg-background/60 hover:border-border hover:bg-background",
                      )}
                      aria-current={selected ? "step" : undefined}
                      aria-label={`${node.data.title}: ${t(tone.labelKey)}`}
                    >
                      <span
                        className={cn(
                          "size-1.5 shrink-0 rounded-full",
                          tone.dot,
                          active
                            && state.status === "running"
                            && "motion-safe:animate-pulse",
                        )}
                        aria-hidden
                      />
                      <span className="truncate text-[11px] font-medium">
                        {node.data.title}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ol>
          </div>
        </div>
      </div>

      <div className="relative flex min-h-0 flex-1 items-center justify-center overflow-auto p-6">
        <div
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_50%_30%,color-mix(in_oklch,var(--muted)_55%,transparent),transparent_65%)]"
          aria-hidden
        />
        <div className="relative w-full max-w-xl">
          {primaryNode && primaryState
            ? (
              <RunTheaterActCard
                nodeId={primaryNode.id}
                data={primaryNode.data}
                state={primaryState}
                live={
                  primaryState.status === "running"
                  || primaryState.status === "awaiting_input"
                }
                variant="stage"
              />
            )
            : (
              <p className="text-center text-sm text-muted-foreground">
                {t("workflowRun.theater.empty")}
              </p>
            )}

          {parallel && (
            <div
              className="mt-5 flex flex-col items-center gap-3"
              aria-label={t("workflowRun.theater.parallelSwitch")}
            >
              <p className="text-center text-[11px] text-muted-foreground">
                {t("workflowRun.theater.parallelHint", {
                  count: focus.activeIds.length,
                  index: parallelIndex >= 0 ? parallelIndex + 1 : 1,
                })}
              </p>
              <div className="flex w-full items-center gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="icon-sm"
                  className="shrink-0"
                  aria-label={t("workflowRun.theater.parallelPrev")}
                  onClick={() => {
                    const next = cycleParallelActive(
                      focus.activeIds,
                      primaryId,
                      -1,
                    );
                    if (next !== null) {
                      onFocusNode(next);
                    }
                  }}
                >
                  <IconChevronLeft className="size-4" />
                </Button>
                <div className="flex min-w-0 flex-1 flex-wrap justify-center gap-1.5">
                  {focus.activeIds.map((nodeId) => {
                    const node = run.definitionSnapshot.nodes.find(
                      (item) => item.id === nodeId,
                    );
                    if (node === undefined) {
                      return null;
                    }
                    const selected = nodeId === primaryId;
                    return (
                      <button
                        key={nodeId}
                        type="button"
                        onClick={() => onFocusNode(nodeId)}
                        className={cn(
                          "max-w-[9rem] truncate rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors",
                          selected
                            ? "border-foreground/35 bg-background shadow-sm"
                            : "border-border/70 bg-muted/40 text-muted-foreground hover:border-border hover:bg-background hover:text-foreground",
                        )}
                        aria-pressed={selected}
                        aria-label={t("workflowRun.theater.focusAct", {
                          name: node.data.title,
                        })}
                      >
                        {node.data.title}
                      </button>
                    );
                  })}
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="icon-sm"
                  className="shrink-0"
                  aria-label={t("workflowRun.theater.parallelNext")}
                  onClick={() => {
                    const next = cycleParallelActive(
                      focus.activeIds,
                      primaryId,
                      1,
                    );
                    if (next !== null) {
                      onFocusNode(next);
                    }
                  }}
                >
                  <IconChevronRight className="size-4" />
                </Button>
              </div>
            </div>
          )}

          <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
            <Badge variant="outline" className="tabular-nums">
              {t(runStatusTone(run.status).labelKey)}
            </Badge>
            {parallel && (
              <Badge variant="secondary" className="tabular-nums">
                {t("workflowRun.theater.parallelCount", {
                  count: focus.activeIds.length,
                })}
              </Badge>
            )}
            {run.totals.tokenUsage?.totalTokens !== undefined && (
              <Badge variant="secondary" className="tabular-nums">
                {t("workflowRun.totalsTokens", {
                  count: run.totals.tokenUsage.totalTokens,
                })}
              </Badge>
            )}
            {run.totals.durationMs !== undefined && (
              <Badge variant="secondary" className="tabular-nums">
                {t("workflowRun.totalsDuration", { ms: run.totals.durationMs })}
              </Badge>
            )}
          </div>
          <p className="mt-3 text-center text-[10px] text-muted-foreground/70">
            {t("workflowRun.theater.returnOverviewHint")}
          </p>
        </div>
      </div>
    </div>
  );
}
