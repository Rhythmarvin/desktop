/**
 * RpcClient — sends outbound JSON-RPC messages and tracks pending responses.
 *
 * In v1, only the Host creates Requests; the Plugin only sends Responses and
 * Notifications (`$/stream`). RpcClient is used to send Responses for Host
 * Requests and `$/stream` notifications for streaming methods.
 */

let nextPluginId = 0;

/** Generate the next plugin-side request id (`p:<u64>`). */
function nextId(): string {
  nextPluginId += 1;
  return `p:${nextPluginId}`;
}

export interface PendingCall {
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
  method: string;
  timer?: ReturnType<typeof setTimeout>;
}

export function createRpcClient(
  send: (type: 1 | 2 | 3, msg: Record<string, unknown>) => void,
  defaultTimeoutMs = 30_000,
) {
  const pending = new Map<string, PendingCall>();

  /** Send a Request and wait for the Response. */
  function call(method: string, params?: Record<string, unknown>, timeoutMs?: number): Promise<unknown> {
    const id = nextId();
    const msg: Record<string, unknown> = {
      jsonrpc: "2.0",
      id,
      method,
    };
    if (params !== undefined) {
      msg.params = params;
    }

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Request timed out: ${method} (${id})`));
      }, timeoutMs ?? defaultTimeoutMs);

      pending.set(id, { resolve, reject, method, timer });

      try {
        send(1, msg);
      } catch (err) {
        clearTimeout(timer);
        pending.delete(id);
        reject(err);
      }
    });
  }

  /** Send a Notification (no response expected). */
  function notify(method: string, params?: Record<string, unknown>): void {
    const msg: Record<string, unknown> = {
      jsonrpc: "2.0",
      method,
    };
    if (params !== undefined) {
      msg.params = params;
    }
    send(3, msg);
  }

  /**
   * Handle an incoming message that might be a Response to one of our pending calls.
   * Returns `true` if the message was consumed as a pending response.
   */
  function handleResponse(msg: Record<string, unknown>): boolean {
    const id = msg.id as string | undefined;
    if (!id) return false;

    const call = pending.get(id);
    if (!call) return false;

    clearTimeout(call.timer);
    pending.delete(id);

    if ("result" in msg) {
      call.resolve(msg.result);
    } else if ("error" in msg) {
      const err = msg.error as Record<string, unknown>;
      const error = new Error(err.message as string) as Error & Record<string, unknown>;
      error.code = err.code;
      error.data = err.data;
      call.reject(error);
    } else {
      call.reject(new Error("Invalid response: missing result and error"));
    }
    return true;
  }

  /** Reject all pending calls (used on shutdown). */
  function destroy(): void {
    for (const [id, call] of pending) {
      clearTimeout(call.timer);
      call.reject(new Error(`Connection closed (pending: ${call.method}, id: ${id})`));
    }
    pending.clear();
  }

  return { call, notify, handleResponse, destroy, pending };
}
