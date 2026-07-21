import type { Disposable, SubscriptionStore } from "./disposable.js";

export type RequestHandler = (params: Record<string, unknown> | undefined) => Promise<unknown>;

export interface PluginLogger {
  debug(msg: string): void;
  info(msg: string): void;
  warn(msg: string): void;
  error(msg: string): void;
}

export interface ExtensionContext {
  readonly extensionId: string;
  readonly extensionPath: string;
  readonly storagePath: string;
  readonly sessionId: string;
  readonly subscriptions: SubscriptionStore;
  readonly logger: PluginLogger;
  readonly shutdownSignal: AbortSignal;
  /** Register a handler that Host can call via JSON-RPC. */
  registerHandler(method: string, handler: RequestHandler): void;
}

export function createExtensionContext(params: {
  extensionId: string;
  extensionPath: string;
  storagePath: string;
  sessionId: string;
  subscriptions: SubscriptionStore;
  shutdownSignal: AbortSignal;
  registerHandler: (method: string, handler: RequestHandler) => void;
}): ExtensionContext {
  const stderr = process.stderr.write.bind(process.stderr);
  const logger: PluginLogger = {
    debug: (msg) => stderr(`[plugin:debug] ${msg}\n`),
    info: (msg) => stderr(`[plugin:info] ${msg}\n`),
    warn: (msg) => stderr(`[plugin:warn] ${msg}\n`),
    error: (msg) => stderr(`[plugin:error] ${msg}\n`),
  };

  return {
    extensionId: params.extensionId,
    extensionPath: params.extensionPath,
    storagePath: params.storagePath,
    sessionId: params.sessionId,
    subscriptions: params.subscriptions,
    logger,
    shutdownSignal: params.shutdownSignal,
    registerHandler: params.registerHandler,
  };
}
