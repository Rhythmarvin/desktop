/**
 * Plugin Host Bootstrap — the single entry point executed by Bun.
 *
 * Sequence:
 *   1. Init transport (capture stdout writer)
 *   2. Wait for $/initialize, respond with echo
 *   3. Init RPC infrastructure
 *   4. Wait for $/activate
 *   5. import() plugin entryPath
 *   6. Structural validation of default export
 *   7. await activate(context)
 *   8. Validate providers vs declaredAgents
 *   9. Respond with { providers }
 *  10. → Running (accept Agent business requests)
 */

import { createTransport } from "../rpc/transport.js";
import {
  createInitializeWaiter,
  createActivateWaiter,
} from "../lifecycle/handshake.js";
import type { InitializeParams } from "../lifecycle/handshake.js";

export type { InitializeParams };

const HEADER_LEN = 5;
const MAX_PAYLOAD = 8 * 1024 * 1024;

// ── Console guard (redirect to stderr) ──────────────────────

const _stderrWrite = process.stderr.write.bind(process.stderr);
console.log = (...args: unknown[]) =>
  _stderrWrite(`[plugin] ${args.map(String).join(" ")}\n`);
console.warn = (...args: unknown[]) =>
  _stderrWrite(`[plugin:warn] ${args.map(String).join(" ")}\n`);
console.error = (...args: unknown[]) =>
  _stderrWrite(`[plugin:error] ${args.map(String).join(" ")}\n`);

// ── Main bootstrap sequence ─────────────────────────────────

export async function bootstrap(): Promise<void> {
  const transport = createTransport(
    () => { /* onClose */ },
    (err) => { console.error(`[bootstrap] transport error: ${err.message}`); },
  );

  // Step 1: Start transport
  transport.start();

  // Step 2: Wait for $/initialize
  const { promise: initPromise, handler: initHandler } = createInitializeWaiter();
  transport.onMessage(initHandler);
  const init: InitializeParams = await initPromise;

  // Step 3: Respond to $/initialize with identity echo
  transport.send(2, {
    jsonrpc: "2.0",
    id: init.requestId,
    result: {
      wireVersion: init.wireVersion,
      runtimeVersion: init.runtimeVersion,
      sessionId: init.sessionId,
      plugin: {
        id: init.plugin.id,
        version: init.plugin.version,
      },
    },
  });

  // Step 4: Wait for $/activate
  const { promise: actPromise, handler: actHandler } = createActivateWaiter();
  transport.onMessage(actHandler);
  const activateReq = await actPromise;

  // Step 5: import() plugin entry
  let entryModule: Record<string, unknown>;
  try {
    entryModule = await import(init.paths.entryPath);
  } catch (e) {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32603,
        message: `Failed to import plugin entry: ${(e as Error).message}`,
      },
    });
    return;
  }

  // Step 6: Structural validation of default export
  const definition = entryModule.default as Record<string, unknown> | undefined;
  if (!definition || typeof definition !== "object" || Array.isArray(definition)) {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32603,
        message: "Plugin default export must be a plain object",
      },
    });
    return;
  }

  // Validate shape
  if (definition.kind !== "agent") {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32603,
        message: `Expected kind="agent", got ${String(definition.kind)}`,
      },
    });
    return;
  }

  if (definition.pluginApi !== 1) {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32603,
        message: `Expected pluginApi=1, got ${String(definition.pluginApi)}`,
      },
    });
    return;
  }

  if (typeof definition.activate !== "function") {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32603,
        message: "activate must be a function",
      },
    });
    return;
  }

  // Step 7: Build ExtensionContext and call activate
  const shutdownController = new AbortController();
  const subscriptions: Array<{ dispose(): void | Promise<void> }> = [];

  const context = {
    plugin: { id: init.plugin.id, version: init.plugin.version },
    sessionId: init.sessionId,
    extensionPath: process.cwd(),
    storagePath: init.paths.storagePath,
    logger: {
      debug: (m: string) => _stderrWrite(`[plugin:debug] ${m}\n`),
      info: (m: string) => _stderrWrite(`[plugin:info] ${m}\n`),
      warn: (m: string) => _stderrWrite(`[plugin:warn] ${m}\n`),
      error: (m: string) => _stderrWrite(`[plugin:error] ${m}\n`),
    },
    shutdownSignal: shutdownController.signal,
    subscriptions: {
      add<T extends { dispose(): void | Promise<void> }>(d: T): T {
        subscriptions.push(d);
        return d;
      },
    },
    errors: {
      business(input: Record<string, unknown>): Error {
        const err = new Error(input.message as string) as Error & {
          kind: string;
          retryable: boolean;
        };
        err.name = "AgentBusinessError";
        err.kind = (input.kind as string) ?? "agentUnavailable";
        err.retryable = (input.retryable as boolean) ?? false;
        return err;
      },
    },
  };

  let activation: Record<string, unknown>;
  try {
    const activateFn = definition.activate as (
      ctx: typeof context,
    ) => Promise<Record<string, unknown>> | Record<string, unknown>;
    activation = (await activateFn(context)) as Record<string, unknown>;
  } catch (e) {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32000,
        message: (e as Error).message,
        data: { kind: "providerFailure" },
      },
    });
    return;
  }

  // Step 8: Validate providers vs declaredAgents
  const providers = (activation.providers as Array<Record<string, unknown>>) ?? [];
  if (providers.length !== init.declaredAgents.length) {
    transport.send(2, {
      jsonrpc: "2.0",
      id: activateReq.requestId,
      error: {
        code: -32603,
        message: `Provider count mismatch: expected ${init.declaredAgents.length}, got ${providers.length}`,
      },
    });
    return;
  }

  for (const declared of init.declaredAgents) {
    const found = providers.find((p) => p.id === declared.id);
    if (!found) {
      transport.send(2, {
        jsonrpc: "2.0",
        id: activateReq.requestId,
        error: {
          code: -32603,
          message: `Missing provider: ${declared.id}`,
        },
      });
      return;
    }
    if (found.contractVersion !== declared.contractVersion) {
      transport.send(2, {
        jsonrpc: "2.0",
        id: activateReq.requestId,
        error: {
          code: -32603,
          message: `Provider ${declared.id} contractVersion mismatch`,
        },
      });
      return;
    }
  }

  // Step 9: Respond with providers
  transport.send(2, {
    jsonrpc: "2.0",
    id: activateReq.requestId,
    result: {
      providers: providers.map((p) => ({
        id: p.id,
        contractVersion: p.contractVersion,
      })),
    },
  });

  // Step 10: Register $/deactivate and $/exit handlers
  // Wait for Host's $/deactivate then $/exit
  let deactivated = false;
  transport.onMessage((msg: Record<string, unknown>) => {
    if (msg.method === "$/deactivate") {
      deactivated = true;
      shutdownController.abort();

      // LIFO dispose subscriptions
      for (let i = subscriptions.length - 1; i >= 0; i--) {
        try {
          subscriptions[i].dispose();
        } catch (e) {
          _stderrWrite(`[bootstrap] dispose error: ${(e as Error).message}\n`);
        }
      }

      if (typeof definition.deactivate === "function") {
        try {
          (definition.deactivate as () => void | Promise<void>)();
        } catch (e) {
          _stderrWrite(`[bootstrap] deactivate error: ${(e as Error).message}\n`);
        }
      }

      transport.send(2, {
        jsonrpc: "2.0",
        id: msg.id,
        result: null,
      });
      return true;
    }

    if (msg.method === "$/exit") {
      // Drain and exit
      setTimeout(() => {
        process.exit(0);
      }, 100);
      return true;
    }

    return false;
  });

  // ── Running ──
  // Plugin is now active and ready for Agent business requests.
  // In v1, all Agent methods are dispatched by the Host via the same transport.
}

// Auto-start when executed directly by Bun
if (import.meta.main || process.argv[1]?.includes("plugin-host-bootstrap")) {
  bootstrap().catch((err) => {
    console.error(`[bootstrap] fatal: ${err.message}`);
    process.exit(1);
  });
}
