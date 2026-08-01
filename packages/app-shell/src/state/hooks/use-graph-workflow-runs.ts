import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { DemoWorkflow } from "@ora/workflow-mock";
import { useWorkflowRuntime } from "../../features/workflow-run/runtime/workflow-runtime-context";
import type { GraphWorkflowRun } from "../../features/workflow-run/runtime/types";
import { useWorkspaceSelectionStore } from "../stores/workspace-selection-store";
import { queryKeys } from "./query-keys";

/**
 * Keeps react-query run caches in sync with mock-engine mutations
 * so sidebar status dots update without a Theater UI yet.
 */
export function useGraphWorkflowRunLiveSync() {
  const runtime = useWorkflowRuntime();
  const queryClient = useQueryClient();
  useEffect(() => {
    return runtime.runs.watch((run) => {
      const clone = structuredClone(run);
      queryClient.setQueryData(queryKeys.workflowRun(run.id), clone);
      // Patch the project list in place to avoid refetch flicker on every node tick.
      queryClient.setQueryData(
        queryKeys.workflowRuns(run.projectId),
        (previous: GraphWorkflowRun[] | undefined) => {
          if (previous === undefined) {
            return previous;
          }
          const index = previous.findIndex((item) => item.id === run.id);
          if (index < 0) {
            return [clone, ...previous];
          }
          const next = previous.slice();
          next[index] = clone;
          return next;
        },
      );
    });
  }, [runtime, queryClient]);
}

/** Lists graph workflow runs for a project (D1: react-query list). */
export function useGraphWorkflowRuns(projectId: string | null | undefined) {
  const runtime = useWorkflowRuntime();
  return useQuery({
    queryKey: queryKeys.workflowRuns(projectId ?? ""),
    queryFn: () => runtime.runs.list(projectId!),
    enabled: projectId != null && projectId !== "",
  });
}

/** Loads one graph workflow run by id. */
export function useGraphWorkflowRun(runId: string | null | undefined) {
  const runtime = useWorkflowRuntime();
  return useQuery({
    queryKey: queryKeys.workflowRun(runId ?? ""),
    queryFn: () => runtime.runs.get(runId!),
    enabled: runId != null && runId !== "",
  });
}

/** Deploys (registers + mounts) a definition onto a project. */
export function useMountWorkflow() {
  const runtime = useWorkflowRuntime();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      projectId,
      definition,
    }: {
      projectId: string;
      definition: DemoWorkflow;
    }) => runtime.host.mount(projectId, definition),
    onSuccess: (_mount, variables) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.workflowMounts(variables.projectId),
      });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.workflowMountsByDefinition(variables.definition.id),
      });
    },
  });
}

/** Starts a graph workflow run from an already-mounted definition. */
export function useCreateGraphWorkflowRun() {
  const runtime = useWorkflowRuntime();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: {
      projectId: string;
      definitionId: string;
      kickoffInput?: string;
    }) => runtime.runs.create(input),
    onSuccess: (run) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.workflowRuns(run.projectId),
      });
      queryClient.setQueryData(queryKeys.workflowRun(run.id), run);
    },
  });
}

/** Deletes a graph workflow run (cancels first when still active). */
export function useDeleteGraphWorkflowRun() {
  const runtime = useWorkflowRuntime();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      runId,
      projectId,
    }: {
      runId: string;
      projectId: string;
    }) => {
      await runtime.runs.delete(runId);
      return { runId, projectId };
    },
    onSuccess: ({ runId, projectId }) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.workflowRuns(projectId),
      });
      queryClient.removeQueries({ queryKey: queryKeys.workflowRun(runId) });
      const selection = useWorkspaceSelectionStore.getState().selection;
      if (selection.workflowRunId === runId) {
        useWorkspaceSelectionStore.getState().clearWorkflowRunSelection(projectId);
      }
    },
  });
}

/** Cancels an in-flight graph workflow run without deleting it. */
export function useCancelGraphWorkflowRun() {
  const runtime = useWorkflowRuntime();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      runId,
      projectId,
    }: {
      runId: string;
      projectId: string;
    }) => {
      await runtime.runs.cancel(runId);
      return { runId, projectId };
    },
    onSuccess: async ({ runId, projectId }) => {
      const run = await runtime.runs.get(runId);
      if (run !== null) {
        queryClient.setQueryData(queryKeys.workflowRun(runId), run);
        queryClient.setQueryData(
          queryKeys.workflowRuns(projectId),
          (previous: GraphWorkflowRun[] | undefined) => {
            if (previous === undefined) {
              return previous;
            }
            return previous.map((item) => (item.id === runId ? run : item));
          },
        );
      }
    },
  });
}

/** Renames a graph workflow run for sidebar / workspace labeling. */
export function useRenameGraphWorkflowRun() {
  const runtime = useWorkflowRuntime();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      runId,
      name,
    }: {
      runId: string;
      name: string;
    }) => runtime.runs.rename(runId, name),
    onSuccess: (run) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.workflowRuns(run.projectId),
      });
      queryClient.setQueryData(queryKeys.workflowRun(run.id), run);
    },
  });
}
