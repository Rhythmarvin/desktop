/**
 * RequestDispatcher — routes inbound Host Requests to registered handlers.
 * Unmatched methods receive a `-32601` response. The dispatcher is also
 * responsible for registering built-in handlers for `$/deactivate`, `$/exit`,
 * and `$/cancelRequest` during bootstrap.
 */

import type { MessageHandler } from "./transport.js";

/** Signature of a method handler: receives params and the request id, returns a result or throws. */
export type MethodHandler = (
  params: Record<string, unknown>,
  requestId: string,
) => Promise<unknown>;

/** Signature of a notification handler: receives params (no response expected). */
export type NotificationHandler = (params: Record<string, unknown>) => void | Promise<void>;

export function createDispatcher(
  send: (type: 2, msg: Record<string, unknown>) => void,
) {
  const methodHandlers = new Map<string, MethodHandler>();
  const notificationHandlers = new Map<string, NotificationHandler>();

  /** Register a method handler (for Host Request → Plugin Response). */
  function registerMethod(method: string, handler: MethodHandler): void {
    if (methodHandlers.has(method)) {
      throw new Error(`Duplicate method registration: ${method}`);
    }
    methodHandlers.set(method, handler);
  }

  /** Register a notification handler (for Host Notification → no response). */
  function registerNotification(method: string, handler: NotificationHandler): void {
    if (notificationHandlers.has(method)) {
      throw new Error(`Duplicate notification registration: ${method}`);
    }
    notificationHandlers.set(method, handler);
  }

  /**
   * Main dispatch function. Returns `true` if the message was consumed.
   * Wire this into `transport.onMessage()`.
   */
  const handle: MessageHandler = (msg) => {
    const method = msg.method as string | undefined;
    const id = msg.id as string | undefined;

    // ── Notifications (no id, no response) ────────────────────
    if (id === undefined) {
      if (!method) return false;
      const notifHandler = notificationHandlers.get(method);
      if (notifHandler) {
        const params = (msg.params ?? {}) as Record<string, unknown>;
        void Promise.resolve().then(() => notifHandler(params));
        return true;
      }
      // Unknown notifications that are syntactically valid are fatal in v1
      // (Plugin→Host Notification set is closed). Signal the caller.
      return false;
    }

    // ── Requests (has id, must respond) ───────────────────────
    if (!method) {
      send(2, {
        jsonrpc: "2.0",
        id,
        error: { code: -32600, message: "Invalid Request: missing method" },
      });
      return true;
    }

    const handler = methodHandlers.get(method);
    if (!handler) {
      send(2, {
        jsonrpc: "2.0",
        id,
        error: { code: -32601, message: `Method not found: ${method}` },
      });
      return true;
    }

    const params = (msg.params ?? {}) as Record<string, unknown>;
    Promise.resolve()
      .then(() => handler(params, id))
      .then((result) => {
        send(2, { jsonrpc: "2.0", id, result: result ?? null });
      })
      .catch((err: unknown) => {
        const e = err as Record<string, unknown> | undefined;
        const isBusinessError = e && (e as Error).name === "AgentBusinessError";
        if (isBusinessError) {
          send(2, {
            jsonrpc: "2.0",
            id,
            error: {
              code: -32000,
              message: (e as Error).message,
              data: { kind: e.kind ?? "agentUnavailable" },
            },
          });
        } else {
          send(2, {
            jsonrpc: "2.0",
            id,
            error: {
              code: -32603,
              message: (e as Error)?.message ?? "Internal error",
            },
          });
        }
      });

    return true;
  };

  return { registerMethod, registerNotification, handle };
}
