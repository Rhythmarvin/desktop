import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@ora/ui";
import {
  IconLayoutSidebarLeftExpand,
  IconRoute,
} from "@tabler/icons-react";
import { DragRegion } from "../../components/drag-region";
import { WindowControls } from "../../components/window-controls";
import { useUiStore } from "../../state/stores/ui-store";
import { useGraphWorkflowRun } from "../../state/hooks/use-graph-workflow-runs";
import type { GraphWorkflowRunStatus } from "./runtime/types";

interface WorkflowRunWorkspaceProps {
  runId: string;
}

function statusLabelKey(status: GraphWorkflowRunStatus): string {
  return `workflowRun.status.${status}`;
}

/**
 * Pre-Theater workspace shell: live status + path progress from the mock engine.
 * Theater / overview / artifacts land in Steps 3–5.
 */
export function WorkflowRunWorkspace({ runId }: WorkflowRunWorkspaceProps) {
  const { t } = useTranslation();
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useUiStore((s) => s.setSidebarCollapsed);
  const runQuery = useGraphWorkflowRun(runId);
  const run = runQuery.data ?? null;

  const progress = useMemo(() => {
    if (run === null) {
      return null;
    }
    const states = Object.values(run.nodeStates);
    const total = states.length;
    const succeeded = states.filter((state) => state.status === "succeeded").length;
    const skipped = states.filter((state) => state.status === "skipped").length;
    let runningNodeTitle: string | undefined;
    for (const [nodeId, state] of Object.entries(run.nodeStates)) {
      if (state.status === "running") {
        runningNodeTitle = run.definitionSnapshot.nodes.find(
          (node) => node.id === nodeId,
        )?.data.title;
        break;
      }
    }
    return {
      total,
      succeeded,
      skipped,
      done: succeeded + skipped,
      runningNodeTitle,
      totalTokens: run.totals.tokenUsage?.totalTokens,
      durationMs: run.totals.durationMs,
    };
  }, [run]);

  return (
    <main
      id="main-content"
      className="flex min-h-0 min-w-0 flex-1 flex-col bg-background"
    >
      <header className="flex h-14 items-center gap-2 border-b border-border px-3">
        {sidebarCollapsed && (
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setSidebarCollapsed(false)}
            aria-label={t("sidebar.expand")}
          >
            <IconLayoutSidebarLeftExpand />
          </Button>
        )}
        <DragRegion>
          <div className="min-w-0">
            <p className="truncate text-sm font-medium tracking-[-0.01em]">
              {run?.name ?? t("workflowRun.loading")}
            </p>
            <p className="truncate text-[11px] text-muted-foreground">
              {run
                ? t(statusLabelKey(run.status))
                : t("workflowRun.placeholderSubtitle")}
            </p>
          </div>
        </DragRegion>
        <WindowControls />
      </header>
      <div className="flex flex-1 items-center justify-center p-6">
        <section className="w-full max-w-lg text-center">
          <div className="mx-auto mb-6 flex size-12 items-center justify-center rounded-lg border border-border bg-muted">
            <IconRoute className="size-5 text-muted-foreground" />
          </div>
          <h1 className="text-xl font-semibold">
            {t("workflowRun.placeholderTitle")}
          </h1>
          <p className="mt-2 text-sm leading-6 text-muted-foreground">
            {t("workflowRun.placeholderBody")}
          </p>
          {run && progress && (
            <dl className="mt-8 grid gap-px overflow-hidden rounded-md border border-border bg-border text-left sm:grid-cols-2">
              <div className="bg-background p-4">
                <dt className="text-xs text-muted-foreground">
                  {t("workflowRun.field.status")}
                </dt>
                <dd className="mt-1 text-sm font-medium">
                  {t(statusLabelKey(run.status))}
                </dd>
              </div>
              <div className="bg-background p-4">
                <dt className="text-xs text-muted-foreground">
                  {t("workflowRun.field.progress")}
                </dt>
                <dd className="mt-1 text-sm font-medium tabular-nums">
                  {t("workflowRun.progressValue", {
                    done: progress.done,
                    total: progress.total,
                  })}
                  {progress.skipped > 0
                    ? ` · ${t("workflowRun.skippedCount", { count: progress.skipped })}`
                    : ""}
                </dd>
              </div>
              <div className="bg-background p-4 sm:col-span-2">
                <dt className="text-xs text-muted-foreground">
                  {t("workflowRun.field.currentNode")}
                </dt>
                <dd className="mt-1 text-sm font-medium">
                  {progress.runningNodeTitle
                    ?? (run.status === "succeeded"
                      ? t("workflowRun.currentNodeDone")
                      : run.status === "cancelled"
                        ? t("workflowRun.currentNodeCancelled")
                        : t("workflowRun.currentNodeIdle"))}
                </dd>
              </div>
              {(progress.totalTokens !== undefined || progress.durationMs !== undefined) && (
                <div className="bg-background p-4 sm:col-span-2">
                  <dt className="text-xs text-muted-foreground">
                    {t("workflowRun.field.totals")}
                  </dt>
                  <dd className="mt-1 text-sm font-medium tabular-nums text-muted-foreground">
                    {progress.totalTokens !== undefined
                      && t("workflowRun.totalsTokens", { count: progress.totalTokens })}
                    {progress.totalTokens !== undefined && progress.durationMs !== undefined
                      ? " · "
                      : ""}
                    {progress.durationMs !== undefined
                      && t("workflowRun.totalsDuration", {
                        ms: progress.durationMs,
                      })}
                  </dd>
                </div>
              )}
            </dl>
          )}
        </section>
      </div>
    </main>
  );
}
