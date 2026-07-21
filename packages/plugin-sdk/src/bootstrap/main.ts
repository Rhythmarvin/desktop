// main.ts — Full design-v3 lifecycle bootstrap entry point.
//
// Lifecycle:
//   1. $/initialize → echo session info
//   2. $/activate   → load plugin, call activate(ctx)
//   3. === Running ===  (business requests accepted)
//   4. $/deactivate → call plugin deactivate(), LIFO dispose subscriptions
//   5. $/exit       → process.exit(0)

import { FrameDecoder } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import { RequestDispatcher } from "../rpc/dispatcher.js";
import { BootstrapSession } from "./session.js";
import { createExtensionContext, type RequestHandler } from "../context.js";
import { createSubscriptionStore } from "../disposable.js";

const stderr = process.stderr.write.bind(process.stderr);

export async function runBootstrap(): Promise<void> {
  stderr("[bootstrap] started, awaiting $/initialize\n");

  const writer = new ProtocolWriter(process.stdout);
  const dispatcher = new RequestDispatcher(writer);
  const session = new BootstrapSession(writer, dispatcher);

  let pluginConfig: {
    activate?(ctx: Record<string, unknown>): void | Promise<void>;
    deactivate?(): void | Promise<void>;
  } | null = null;
  let pluginSubs = createSubscriptionStore();

  // Load plugin config (set by definePlugin() before runBootstrap())
  const raw = (globalThis as Record<string, unknown>).__ora_plugin_config as typeof pluginConfig;
  pluginConfig = raw ?? null;

  // Built-in handlers
  let pingHandled = false;
  dispatcher.register("ping", async () => {
    stderr("[bootstrap] received ping\n");
    if (!pingHandled) {
      pingHandled = true;
      setTimeout(() => {
        const note = new TextEncoder().encode(JSON.stringify({
          jsonrpc: "2.0", method: "$/hello",
          params: { message: "Hello from plugin! Bidirectional communication works.", timestamp: Date.now() },
        }));
        writer.write(3, note);
        stderr("[bootstrap] sent $/hello notification\n");
      }, 50);
    }
    return { pong: true, timestamp: Date.now() };
  });

  // Read loop
  const decoder = new FrameDecoder();
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

        // ── Handshake: $/initialize complete, awaiting $/activate ──
        if (result.type === "handshake") {
          stderr("[bootstrap] $/initialize complete, awaiting $/activate\n");
        }

        // ── Activate: call plugin activate() ──
        if (result.type === "activate") {
          stderr("[bootstrap] $/activate received\n");
          const params = session.initParams;
          if (params && pluginConfig?.activate) {
            const ctx = createExtensionContext({
              extensionId: params.pluginId,
              extensionPath: params.extensionPath,
              storagePath: params.storagePath,
              sessionId: params.sessionId,
              subscriptions: pluginSubs,
              shutdownSignal: session.shutdownSignal,
              registerHandler: (method: string, handler: RequestHandler) => {
                stderr(`[bootstrap] plugin registered handler: ${method}\n`);
                dispatcher.register(method, handler);
              },
            });

            try {
              await pluginConfig.activate(ctx as any);
              stderr("[bootstrap] plugin activate() completed\n");
            } catch (err) {
              stderr(`[bootstrap] plugin activate() failed: ${err}\n`);
            }
          } else if (!pluginConfig) {
            stderr("[bootstrap] no plugin config — running with built-in handlers only\n");
          }
        }

        // ── Deactivate: call plugin deactivate() + LIFO dispose ──
        if (result.type === "deactivate") {
          stderr("[bootstrap] $/deactivate received\n");
          if (pluginConfig?.deactivate) {
            try { await pluginConfig.deactivate(); } catch (err) {
              stderr(`[bootstrap] plugin deactivate() failed: ${err}\n`);
            }
          }
          await pluginSubs.disposeAll();
          stderr("[bootstrap] subscriptions disposed, awaiting $/exit\n");
        }

        // ── Exit ──
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
