import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Badge,
  Button,
  Spinner,
  cn,
} from "@ora/ui";
import {
  IconLayoutSidebarLeftExpand,
  IconMap,
  IconPlayerPlay,
  IconPlayerStop,
  IconTheater,
} from "@tabler/icons-react";
import { DragRegion } from "../../components/drag-region";
import { WindowControls } from "../../components/window-controls";
import { useUiStore } from "../../state/stores/ui-store";
import { useWorkspaceSelectionStore } from "../../state/stores/workspace-selection-store";
import {
  useCancelGraphWorkflowRun,
  useGraphWorkflowRun,
  useRerunGraphWorkflowRun,
  useStartGraphWorkflowRun,
} from "../../state/hooks/use-graph-workflow-runs";
import { RunOverviewCanvas } from "./run-overview-canvas";
import { RunTheater } from "./run-theater";
import { runStatusTone } from "./run-status-style";
import type { GraphWorkflowRunStatus } from "./runtime/types";
import type { WorkflowRunViewMode } from "./run-view-mode";

interface WorkflowRunWorkspaceProps {
  runId: string;
}

function isTerminalRunStatus(status: GraphWorkflowRunStatus): boolean {
  return (
    status === "succeeded"
    || status === "failed"
    || status === "partial_failed"
    || status === "cancelled"
  );
}

/**
 * Graph workflow run workspace: Overview after mount (pending) + Theater when live.
 * Header mirrors Settings Test Run: Start → Stop → Run again on a new sibling run.
 */
export function WorkflowRunWorkspace({ runId }: WorkflowRunWorkspaceProps) {
  const { t } = useTranslation();
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useUiStore((s) => s.setSidebarCollapsed);
  const selectWorkflowRun = useWorkspaceSelectionStore((s) => s.selectWorkflowRun);
  const runQuery = useGraphWorkflowRun(runId);
  const run = runQuery.data ?? null;
  const startRun = useStartGraphWorkflowRun();
  const cancelRun = useCancelGraphWorkflowRun();
  const rerun = useRerunGraphWorkflowRun();

  const [viewMode, setViewMode] = useState<WorkflowRunViewMode>("overview");
  const [focusNodeId, setFocusNodeId] = useState<string | null>(null);
  const [stopOpen, setStopOpen] = useState(false);

  // Reset local chrome when switching runs; mode is primed once below.
  useEffect(() => {
    setFocusNodeId(null);
    setStopOpen(false);
  }, [runId]);

  // Prime view once per selected run: pending/terminal → Overview, live → Theater.
  // Later status ticks must not steal Overview if the user chose it mid-run.
  const primedRunIdRef = useRef<string | null>(null);
  useEffect(() => {
    if (run === null || run.id !== runId) {
      return;
    }
    if (primedRunIdRef.current === runId) {
      return;
    }
    primedRunIdRef.current = runId;
    if (run.status === "running" || run.status === "awaiting_input") {
      setViewMode("theater");
    } else {
      setViewMode("overview");
    }
  }, [run, runId]);

  // HITL always forces Theater (product rule 3.5).
  useEffect(() => {
    if (run?.status === "awaiting_input") {
      setViewMode("theater");
    }
  }, [run?.status]);

  // Esc from Theater returns to Overview.
  useEffect(() => {
    if (viewMode !== "theater") {
      return;
    }
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") {
        setViewMode("overview");
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [viewMode]);

  const canStart = run?.status === "pending";
  const canStop = run !== null
    && (run.status === "running" || run.status === "awaiting_input");
  const canRunAgain = run !== null && isTerminalRunStatus(run.status);
  const runTone = run !== null ? runStatusTone(run.status) : null;
  const actionBusy = startRun.isPending || cancelRun.isPending || rerun.isPending;

  function focusNode(nodeId: string): void {
    setFocusNodeId(nodeId);
  }

  function focusNodeFromOverview(nodeId: string): void {
    setFocusNodeId(nodeId);
    setViewMode("theater");
  }

  async function handleStart(): Promise<void> {
    if (run === null || !canStart) {
      return;
    }
    await startRun.mutateAsync({
      runId: run.id,
      projectId: run.projectId,
    });
    setViewMode("theater");
  }

  async function handleRunAgain(): Promise<void> {
    if (run === null || !canRunAgain) {
      return;
    }
    const next = await rerun.mutateAsync(run);
    selectWorkflowRun(next.id, next.projectId);
  }

  async function handleConfirmStop(): Promise<void> {
    if (run === null || !canStop) {
      return;
    }
    await cancelRun.mutateAsync({
      runId: run.id,
      projectId: run.projectId,
    });
    setStopOpen(false);
  }

  return (
    <main
      id="main-content"
      className="flex min-h-0 min-w-0 flex-1 flex-col bg-background"
    >
      <header className="flex h-14 shrink-0 items-center gap-2 border-b border-border px-3">
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
        <DragRegion className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <div className="min-w-0">
              <p className="truncate text-sm font-medium tracking-[-0.01em]">
                {run?.name ?? t("workflowRun.loading")}
              </p>
              <p className="truncate text-[11px] text-muted-foreground">
                {runTone
                  ? t(runTone.labelKey)
                  : t("workflowRun.placeholderSubtitle")}
              </p>
            </div>
            {runTone && (
              <Badge
                variant="outline"
                className={cn("hidden border sm:inline-flex", runTone.badge)}
              >
                <span className={cn("size-1.5 rounded-full", runTone.dot)} aria-hidden />
                {t(runTone.labelKey)}
              </Badge>
            )}
          </div>
        </DragRegion>

        <div
          className="flex shrink-0 items-center gap-1.5"
          role="group"
          aria-label={t("workflowRun.viewMode.label")}
        >
          <div className="inline-flex rounded-lg border border-border p-0.5">
            <Button
              type="button"
              size="sm"
              variant={viewMode === "theater" ? "secondary" : "ghost"}
              className="h-7 gap-1.5 px-2.5 text-xs"
              aria-pressed={viewMode === "theater"}
              onClick={() => setViewMode("theater")}
            >
              <IconTheater className="size-3.5" />
              {t("workflowRun.viewMode.theater")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant={viewMode === "overview" ? "secondary" : "ghost"}
              className="h-7 gap-1.5 px-2.5 text-xs"
              aria-pressed={viewMode === "overview"}
              onClick={() => setViewMode("overview")}
            >
              <IconMap className="size-3.5" />
              {t("workflowRun.viewMode.overview")}
            </Button>
          </div>
          {canStart && run && (
            <Button
              type="button"
              size="sm"
              className="h-7 gap-1.5 px-2.5 text-xs"
              disabled={actionBusy}
              onClick={() => {
                void handleStart();
              }}
            >
              {startRun.isPending
                ? <Spinner className="size-3.5" />
                : <IconPlayerPlay className="size-3.5" />}
              {t("workflowRun.startAction")}
            </Button>
          )}
          {canStop && run && (
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-7 gap-1.5 px-2.5 text-xs"
              disabled={actionBusy}
              onClick={() => setStopOpen(true)}
            >
              <IconPlayerStop className="size-3.5" />
              {t("workflowRun.stopAction")}
            </Button>
          )}
          {canRunAgain && run && (
            <Button
              type="button"
              size="sm"
              className="h-7 gap-1.5 px-2.5 text-xs"
              disabled={actionBusy}
              onClick={() => {
                void handleRunAgain();
              }}
            >
              {rerun.isPending
                ? <Spinner className="size-3.5" />
                : <IconPlayerPlay className="size-3.5" />}
              {t("workflowRun.runAgainAction")}
            </Button>
          )}
        </div>
        <WindowControls />
      </header>

      {runQuery.isLoading && run === null
        ? (
          <div className="flex flex-1 items-center justify-center gap-2 text-sm text-muted-foreground">
            <Spinner className="size-4" />
            {t("workflowRun.loading")}
          </div>
        )
        : run === null
        ? (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            {t("workflowRun.missing")}
          </div>
        )
        : viewMode === "theater"
        ? (
          <RunTheater
            run={run}
            focusNodeId={focusNodeId}
            onFocusNode={focusNode}
          />
        )
        : (
          <RunOverviewCanvas
            run={run}
            focusedNodeId={focusNodeId}
            onFocusNode={focusNodeFromOverview}
          />
        )}

      <AlertDialog open={stopOpen} onOpenChange={setStopOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("workflowRun.stopTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("workflowRun.stopDescription", {
                name: run?.name ?? "",
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={cancelRun.isPending}>
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={cancelRun.isPending}
              onClick={(event) => {
                event.preventDefault();
                void handleConfirmStop();
              }}
            >
              {cancelRun.isPending
                ? t("workflowRun.stopping")
                : t("workflowRun.stopConfirmAction")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </main>
  );
}
