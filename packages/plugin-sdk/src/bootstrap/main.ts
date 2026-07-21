// main.ts — Bootstrap entry point. Executed by Bun as the plugin host process.
//
// Lifecycle:
//   1. Initialize transport (stdin frame reader, stdout writer)
//   2. Wait for $/initialize Request from Host, respond with session info
//   3. Register plugin handlers (ping, etc.)
//   4. Process frames until $/exit notification

import { FrameDecoder } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import { RequestDispatcher } from "../rpc/dispatcher.js";
import { BootstrapSession } from "./session.js";

const stderr = process.stderr.write.bind(process.stderr);

export async function runBootstrap(): Promise<void> {
  stderr("[bootstrap] started, waiting for frames on stdin\n");

  const writer = new ProtocolWriter(process.stdout);
  const dispatcher = new RequestDispatcher(writer);
  const session = new BootstrapSession(writer, dispatcher);
  const decoder = new FrameDecoder();

  // Register built-in handlers
  let pingHandled = false;
  dispatcher.register("ping", async (_params) => {
    stderr("[bootstrap] received ping\n");
    const result = { pong: true, timestamp: Date.now() };

    // After first ping, send a hello notification to prove bidirectionality
    if (!pingHandled) {
      pingHandled = true;
      setTimeout(() => {
        const notePayload = new TextEncoder().encode(
          JSON.stringify({
            jsonrpc: "2.0",
            method: "$/hello",
            params: { message: "Hello from plugin! Bidirectional communication works.", timestamp: Date.now() },
          })
        );
        writer.write(3, notePayload); // FrameType.Notification = 3
        stderr("[bootstrap] sent $/hello notification\n");
      }, 50);
    }

    return result;
  });

  // Read loop
  try {
    for await (const chunk of process.stdin) {
      const bytes = typeof chunk === "string"
        ? new TextEncoder().encode(chunk)
        : new Uint8Array(chunk as ArrayBuffer);

      for (const frame of decoder.decodeChunk(bytes)) {
        // Build full wire bytes for session processing
        const wireBytes = new Uint8Array(5 + frame.payload.byteLength);
        const view = new DataView(wireBytes.buffer);
        view.setInt32(0, frame.payload.byteLength, false);
        view.setInt8(4, frame.type);
        wireBytes.set(frame.payload, 5);

        const result = session.processFrame(wireBytes);

        if (result.type === "handshake") {
          stderr(`[bootstrap] handshake complete: plugin=${result.params.pluginId}\n`);
        }

        if (result.type === "exit" || session.phase === "exiting") {
          stderr("[bootstrap] received $/exit, shutting down\n");
          process.exit(0);
        }
      }
    }
  } catch (err) {
    stderr(`[bootstrap] error: ${err}\n`);
    process.exit(1);
  }

  stderr("[bootstrap] stdin closed\n");
  process.exit(0);
}

// Auto-run when executed as main
runBootstrap();
