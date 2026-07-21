// dispatcher.ts — Minimal request dispatcher with -32601 fallback.

import { FrameType } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import { encodeError, type RpcRequest } from "./envelope.js";

export type RequestHandler = (params: Record<string, unknown> | undefined) => Promise<unknown>;

export class RequestDispatcher {
  readonly #handlers = new Map<string, RequestHandler>();
  readonly #writer: ProtocolWriter;

  constructor(writer: ProtocolWriter) {
    this.#writer = writer;
  }

  register(method: string, handler: RequestHandler): void {
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
      const result = await handler(request.params);
      const payload = encodeError(request.id, 0, ""); // placeholder
      // Encode success
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
