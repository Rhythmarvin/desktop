/**
 * Transport — wraps FrameReader/FrameWriter for JSON-RPC over stdio.
 */
import { createFrameReader, type ParsedFrame, type FrameHandler, type ErrorHandler, type CloseHandler } from "../internal/reader.js";
import { createFrameWriter } from "../internal/writer.js";

export type MessageHandler = (msg: Record<string, unknown>) => boolean;

export function createTransport(onClose?: CloseHandler, onError?: ErrorHandler) {
  const writer = createFrameWriter();
  let messageHandlers: MessageHandler[] = [];
  let running = false;
  let stdinBuffer: Buffer[] = [];

  // Frame → message dispatch
  const frameHandler: FrameHandler = (frame: ParsedFrame) => {
    const msg = frame.payloadJson as Record<string, unknown>;
    // Ensure it's a valid JSON-RPC shape
    if (typeof msg.jsonrpc !== "string" || msg.jsonrpc !== "2.0") {
      onError?.(new Error(`Invalid jsonrpc version: ${msg.jsonrpc}`));
      return;
    }

    // Notify all handlers; first to return true consumes the message
    for (const handler of messageHandlers) {
      try {
        if (handler(msg)) return;
      } catch (e) {
        onError?.(e as Error);
        return;
      }
    }
    // Unhandled message is not fatal — just warn
    const method = msg.method as string | undefined;
    if (method) {
      // If it's a request, respond with -32601
      if (typeof msg.id === "string" && frame.type === 1) {
        send(2, {
          jsonrpc: "2.0",
          id: msg.id,
          error: { code: -32601, message: `Method not found: ${method}` },
        });
      }
    }
  };

  const reader = createFrameReader(
    frameHandler,
    (err) => {
      onError?.(err);
      running = false;
    },
    () => {
      running = false;
      onClose?.();
    },
  );

  function start(): void {
    running = true;
    // Use Bun's stdin stream if available
    if (typeof Bun !== "undefined" && Bun.stdin) {
      const stdinStream = Bun.stdin.stream();
      const reader2 = stdinStream.getReader();
      function pump(): void {
        reader2.read().then(({ done, value }) => {
          if (done) {
            reader.end();
            return;
          }
          reader.feed(Buffer.from(value));
          if (running) pump();
        }).catch((err: Error) => {
          onError?.(err);
        });
      }
      pump();
    } else {
      // Node.js fallback
      process.stdin.on("data", (chunk: Buffer) => {
        if (running) reader.feed(chunk);
      });
      process.stdin.on("end", () => {
        reader.end();
      });
    }
  }

  function send(type: 1 | 2 | 3, message: Record<string, unknown>): void {
    const json = JSON.stringify(message);
    const ok = writer.writeFrame(type, json);
    if (!ok) {
      onError?.(new Error("Failed to write frame to stdout"));
    }
  }

  function onMessage(handler: MessageHandler): () => void {
    messageHandlers.push(handler);
    return () => {
      messageHandlers = messageHandlers.filter((h) => h !== handler);
    };
  }

  return { start, send, onMessage, reader };
}

export { createFrameWriter } from "../internal/writer.js";
export { encodeFrame } from "../internal/writer.js";
