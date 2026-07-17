/**
 * Handshake waiters for $/initialize and $/activate.
 * Each returns a { promise, handler } pair.
 */

export interface InitializeParams {
  requestId: string;
  wireVersion: number;
  hostVersion: string;
  runtimeVersion: string;
  sessionId: string;
  plugin: {
    id: string;
    version: string;
    kind: string;
    pluginApi: number;
    contentOwner: string;
  };
  paths: {
    extensionPath: string;
    entryPath: string;
    storagePath: string;
  };
  declaredAgents: Array<{ id: string; contractVersion: number }>;
  limits: {
    maxFrameBytes: number;
    maxPendingRequests: number;
    maxAgentEventBytes: number;
    maxAgentResultBytes: number;
    maxAgentPromptBytes: number;
    maxActiveTurns: number;
    maxPageItems: number;
  };
}

export interface ActivateParams {
  requestId: string;
  reason: "lazyInvocation" | "manualStart";
}

/**
 * Create a waiter for the $/initialize handshake.
 * Resolves when the first $/initialize Request arrives.
 */
export function createInitializeWaiter(): {
  promise: Promise<InitializeParams>;
  handler: (msg: Record<string, unknown>) => boolean;
} {
  let resolve!: (params: InitializeParams) => void;
  const promise = new Promise<InitializeParams>((r) => { resolve = r; });

  const handler = (msg: Record<string, unknown>): boolean => {
    if (msg.method !== "$/initialize") return false;
    if (typeof msg.id !== "string") return false;

    const params = msg.params as Record<string, unknown>;
    const plugin = params.plugin as Record<string, unknown>;
    const paths = params.paths as Record<string, unknown>;
    const limits = params.limits as Record<string, unknown>;
    const agents = params.declaredAgents as Array<Record<string, unknown>>;

    resolve({
      requestId: msg.id,
      wireVersion: params.wireVersion as number,
      hostVersion: params.hostVersion as string,
      runtimeVersion: params.runtimeVersion as string,
      sessionId: params.sessionId as string,
      plugin: {
        id: plugin.id as string,
        version: plugin.version as string,
        kind: plugin.kind as string,
        pluginApi: plugin.pluginApi as number,
        contentOwner: plugin.contentOwner as string,
      },
      paths: {
        extensionPath: paths.extensionPath as string,
        entryPath: paths.entryPath as string,
        storagePath: paths.storagePath as string,
      },
      declaredAgents: agents.map((a) => ({
        id: a.id as string,
        contractVersion: a.contractVersion as number,
      })),
      limits: {
        maxFrameBytes: limits.maxFrameBytes as number,
        maxPendingRequests: limits.maxPendingRequests as number,
        maxAgentEventBytes: limits.maxAgentEventBytes as number,
        maxAgentResultBytes: limits.maxAgentResultBytes as number,
        maxAgentPromptBytes: limits.maxAgentPromptBytes as number,
        maxActiveTurns: limits.maxActiveTurns as number,
        maxPageItems: limits.maxPageItems as number,
      },
    });
    return true;
  };

  return { promise, handler };
}

/**
 * Create a waiter for the $/activate handshake.
 * Resolves when the $/activate Request arrives (after initialize).
 */
export function createActivateWaiter(): {
  promise: Promise<ActivateParams>;
  handler: (msg: Record<string, unknown>) => boolean;
} {
  let resolve!: (params: ActivateParams) => void;
  const promise = new Promise<ActivateParams>((r) => { resolve = r; });

  const handler = (msg: Record<string, unknown>): boolean => {
    if (msg.method !== "$/activate") return false;
    if (typeof msg.id !== "string") return false;

    const params = (msg.params ?? {}) as Record<string, unknown>;
    resolve({
      requestId: msg.id,
      reason: (params.reason as "lazyInvocation" | "manualStart") ?? "lazyInvocation",
    });
    return true;
  };

  return { promise, handler };
}
