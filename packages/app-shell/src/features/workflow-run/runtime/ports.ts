import type { DemoWorkflow } from "@ora/workflow-mock";
import type {
  GraphWorkflowRun,
  ProjectWorkflowMount,
  Unsubscribe,
  WorkflowArtifact,
  WorkflowRunEvent,
} from "./types";

/**
 * Project-level binding of a workflow definition (reference, not a copy).
 *
 * Invariant: at most one mount per (projectId, definitionId). Remount refreshes
 * the stored definition blob. Multiple executions are GraphWorkflowRun rows, not
 * duplicate mounts.
 */
export interface WorkflowHostRepository {
  listMounts: (projectId: string) => Promise<ProjectWorkflowMount[]>;
  /** Projects that already reference this definition (for deploy UX grouping). */
  listMountsByDefinition: (
    definitionId: string,
  ) => Promise<ProjectWorkflowMount[]>;
  /** Registers or refreshes the definition blob, then upserts the project mount. */
  mount: (
    projectId: string,
    definition: DemoWorkflow,
  ) => Promise<ProjectWorkflowMount>;
  unmount: (projectId: string, definitionId: string) => Promise<void>;
  getDefinition: (definitionId: string) => Promise<DemoWorkflow | null>;
}

/** Lifecycle and event surface for GraphWorkflowRun instances. */
export interface WorkflowRunRepository {
  list: (projectId: string) => Promise<GraphWorkflowRun[]>;
  get: (runId: string) => Promise<GraphWorkflowRun | null>;
  create: (input: {
    projectId: string;
    definitionId: string;
    kickoffInput?: string;
  }) => Promise<GraphWorkflowRun>;
  cancel: (runId: string) => Promise<void>;
  /**
   * Removes a run from the project list. Active runs are cancelled first so
   * concurrent siblings stay unaffected.
   */
  delete: (runId: string) => Promise<void>;
  /** Updates the display name shown in the sidebar and run workspace header. */
  rename: (runId: string, name: string) => Promise<GraphWorkflowRun>;
  submitHitl: (
    runId: string,
    requestId: string,
    payload: Record<string, unknown>,
  ) => Promise<void>;
  listArtifacts: (runId: string) => Promise<WorkflowArtifact[]>;
  /**
   * Subscribes to run events. Step 1 may be a no-op until the mock engine
   * advances nodes in Step 2; callers should still unregister on unmount.
   */
  subscribe: (
    runId: string,
    onEvent: (event: WorkflowRunEvent) => void,
  ) => Unsubscribe;
}

/** Combined runtime port so the shell can inject one memory (or future HTTP) impl. */
export interface WorkflowRuntime {
  host: WorkflowHostRepository;
  runs: WorkflowRunRepository;
}
