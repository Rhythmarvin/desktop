// main.ts — Bootstrap entry point. Executed by Bun as the plugin host process.
//
// Lifecycle:
//   1. Initialize transport (stdin frame reader, stdout writer)
//   2. Wait for $/initialize Request from Host, respond with session info
//   3. If a plugin entry is available (via __ora_plugin_config or ORA_PLUGIN_ENTRY env),
//      call activate(context) — plugins register their handlers here
//   4. Process frames until $/exit notification

import { FrameDecoder } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import { RequestDispatcher } from "../rpc/dispatcher.js";
import { BootstrapSession } from "./session.js";
import { createExtensionContext, type RequestHandler } from "../context.js";
import { createSubscriptionStore } from "../disposable.js";

const stderr = process.stderr.write.bind(process.stderr);

export async function runBootstrap(): Promise<void> {
  stderr("[bootstrap] started, waiting for frames on stdin\n");

  const writer = new ProtocolWriter(process.stdout);
  const dispatcher = new RequestDispatcher(writer);
  const session = new BootstrapSession(writer, dispatcher);
  const decoder = new FrameDecoder();

  // Built-in handlers (always available)
  let pingHandled = false;
  dispatcher.register("ping", async (_params) => {
    stderr("[bootstrap] received ping\n");
    if (!pingHandled) {
      pingHandled = true;
      setTimeout(() => {
        const notePayload = new TextEncoder().encode(
          JSON.stringify({
            jsonrpc: "2.0",
            method: "$/hello",
            params: {
              message: "Hello from plugin! Bidirectional communication works.",
              timestamp: Date.now(),
            },
          })
        );
        writer.write(3, notePayload);
        stderr("[bootstrap] sent $/hello notification\n");
      }, 50);
    }
    return { pong: true, timestamp: Date.now() };
  });

  // Read loop — handshake first, then load plugin after
  let pluginLoaded = false;

  try {
    for await (const chunk of process.stdin) {
      const bytes = typeof chunk === "string"
        ? new TextEncoder().encode(chunk)
        : new Uint8Array(chunk as ArrayBuffer);

      for (const frame of decoder.decodeChunk(bytes)) {
        const wireBytes = new Uint8Array(5 + frame.payload.byteLength);
        const view = new DataView(wireBytes.buffer);
        view.setInt32(0, frame.payload.byteLength, false);
        view.setInt8(4, frame.type);
        wireBytes.set(frame.payload, 5);

        const result = session.processFrame(wireBytes);

        if (result.type === "handshake") {
          stderr(`[bootstrap] handshake complete: plugin=${result.params.pluginId}\n`);

          // Load plugin after handshake (only once)
          if (!pluginLoaded) {
            pluginLoaded = true;
            await loadPluginEntry(result.params.extensionPath, {
              registerHandler: (method: string, handler: RequestHandler) => {
                stderr(`[bootstrap] plugin registered handler: ${method}\n`);
                dispatcher.register(method, handler);
              },
              extensionId: result.params.pluginId,
              extensionPath: result.params.extensionPath,
              storagePath: result.params.storagePath,
              sessionId: result.params.sessionId,
              shutdownSignal: session.shutdownSignal,
            });
          }
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

async function loadPluginEntry(
  extensionPath: string,
  contextParams: {
    registerHandler: (method: string, handler: RequestHandler) => void;
    extensionId: string;
    extensionPath: string;
    storagePath: string;
    sessionId: string;
    shutdownSignal: AbortSignal;
  },
): Promise<void> {
  // Check for plugin config set via definePlugin()
  const config = (globalThis as Record<string, unknown>).__ora_plugin_config as
    | { activate?(ctx: Record<string, unknown>): void | Promise<void>; deactivate?(): void | Promise<void> }
    | undefined;

  if (config?.activate) {
    const subs = createSubscriptionStore();
    const ctx = createExtensionContext({ ...contextParams, subscriptions: subs });
    try {
      await config.activate(ctx);
      stderr("[bootstrap] plugin activate() completed\n");
    } catch (err) {
      stderr(`[bootstrap] plugin activate() failed: ${err}\n`);
    }
  } else {
    stderr("[bootstrap] no plugin config found — running with built-in handlers only\n");
  }
}
