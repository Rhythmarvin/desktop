// dispatcher.ts — Minimal request dispatcher with -32601 fallback.

import { FrameType } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import { encodeError, type RpcRequest } from "./envelope.js";

export type RequestHandler = (params: Record<string, unknown> | undefined) => Promise<unknown>;

/**
 * Extended handler that also receives the Host request id.
 * Used by agent plugin handlers that need to emit `acp/event` Notifications
 * referencing the original Host request.
 */
export type RequestHandlerWithId = (
  params: Record<string, unknown> | undefined,
  requestId: string,
) => Promise<unknown>;

type AnyHandler = RequestHandler | RequestHandlerWithId;

function isWithId(h: AnyHandler): h is RequestHandlerWithId {
  return h.length >= 2;
}

export class RequestDispatcher {
  readonly #handlers = new Map<string, AnyHandler>();
  readonly #writer: ProtocolWriter;

  constructor(writer: ProtocolWriter) {
    this.#writer = writer;
  }

  register(method: string, handler: AnyHandler): void {
    this.#handlers.set(method, handler);
  }

  unregister(method: string): void {
    this.#handlers.delete(method);
  }

  /** Routes one inbound Request to the registered handler, or sends -32601. */
  async dispatch(request: RpcRequest): Promise<void> {
    const handler = this.#handlers.get(request.method);
    if (!handler) {
      const payload = encodeError(request.id, -32601, `Method not found: ${request.method}`);
      this.#writer.write(FrameType.Response, payload);
      return;
    }
    try {
      const result = isWithId(handler)
        ? await handler(request.params, request.id)
        : await handler(request.params);
      const successPayload = new TextEncoder().encode(
        JSON.stringify({ jsonrpc: "2.0", id: request.id, result: result ?? null })
      );
      this.#writer.write(FrameType.Response, successPayload);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const payload = encodeError(request.id, -32603, message);
      this.#writer.write(FrameType.Response, payload);
    }
  }
}
