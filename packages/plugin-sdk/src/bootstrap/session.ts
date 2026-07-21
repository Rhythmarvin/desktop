// session.ts — Full design-v3 lifecycle state machine.
//
// Phases:
//   awaitingInitialize → initialized → awaitingActivate → running → deactivating → exiting
//
// Flows:
//   $/initialize → initialized
//   $/activate   → running (plugin activate() called here)
//   $/deactivate → deactivated (plugin deactivate() + LIFO dispose)
//   $/exit       → exiting (process.exit)

import { FrameType } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import {
  encodeError,
  encodeSuccess,
  parseInbound,
} from "../rpc/envelope.js";
import { RequestDispatcher } from "../rpc/dispatcher.js";
import type { SubscriptionStore } from "../disposable.js";

const stderr = process.stderr.write.bind(process.stderr);

// ── Types ────────────────────────────────────────────────────────

export interface InitParams {
  wireVersion: number;
  sessionId: string;
  pluginId: string;
  pluginVersion: string;
  extensionPath: string;
  entryPath: string;
  storagePath: string;
}

export interface SessionCallbacks {
  onActivate(params: InitParams): Promise<void>;
  onDeactivate(): Promise<void>;
}

type Phase =
  | "awaitingInitialize"
  | "initialized"
  | "awaitingActivate"
  | "running"
  | "deactivating"
  | "exiting";

// ── Session ──────────────────────────────────────────────────────

export class BootstrapSession {
  readonly #writer: ProtocolWriter;
  readonly #dispatcher: RequestDispatcher;
  readonly #callbacks: SessionCallbacks;
  #phase: Phase = "awaitingInitialize";
  #initParams: InitParams | null = null;
  #shutdownAbort = new AbortController();

  constructor(
    writer: ProtocolWriter,
    dispatcher: RequestDispatcher,
    callbacks: SessionCallbacks,
  ) {
    this.#writer = writer;
    this.#dispatcher = dispatcher;
    this.#callbacks = callbacks;
  }

  get phase(): Phase { return this.#phase; }
  get shutdownSignal(): AbortSignal { return this.#shutdownAbort.signal; }

  get initParams(): InitParams | null { return this.#initParams; }

  /** Processes one incoming frame. Returns lifecycle signals for the caller to handle. */
  processFrame(data: Uint8Array): { type: "handshake" } | { type: "activate" } | { type: "deactivate" } | { type: "message" } | { type: "exit" } {
    const envelope = parseInbound({
      type: data[4] as FrameType,
      payload: data.slice(5),
    });

    // Phase: awaitingInitialize — only $/initialize is accepted
    if (this.#phase === "awaitingInitialize") {
      if (envelope.type !== "request" || envelope.method !== "$/initialize") {
        throw new Error("first Host frame must be $/initialize Request");
      }
      this.#initParams = this.#handleInitialize(envelope.id, (envelope as any).params ?? {});
      this.#phase = "awaitingActivate";
      return { type: "handshake" };
    }

    // Phase: awaitingActivate — only $/activate is accepted
    if (this.#phase === "awaitingActivate") {
      if (envelope.type === "request" && envelope.method === "$/activate") {
        // Respond to $/activate synchronously, then callback will be called
        const activateId = (envelope as any).id as string;
        const reason = (envelope as any).params?.reason ?? "manualStart";
        stderr(`[bootstrap] $/activate: reason=${reason}\n`);

        // Send activate response first, then call plugin activate
        this.#writer.write(
          FrameType.Response,
          encodeSuccess(activateId, { providers: [] }),
        );

        this.#phase = "running";
        return { type: "activate" };
      }
      // Reject non-activate requests during this phase
      if (envelope.type === "request") {
        const req = envelope as any;
        const payload = encodeError(req.id, -32603, "plugin not yet activated");
        this.#writer.write(FrameType.Response, payload);
        return { type: "message" };
      }
      return { type: "message" };
    }

    // Phase: running — accept business requests + lifecycle
    if (this.#phase === "running") {
      if (envelope.type === "request") {
        if (envelope.method === "$/initialize") {
          throw new Error("$/initialize cannot be repeated");
        }
        if (envelope.method === "$/activate") {
          throw new Error("$/activate cannot be repeated");
        }

        // $/deactivate → graceful shutdown
        if (envelope.method === "$/deactivate") {
          this.#phase = "deactivating";
          const deactId = (envelope as any).id as string;
          stderr("[bootstrap] $/deactivate\n");
          this.#writer.write(FrameType.Response, encodeSuccess(deactId, null));
          return { type: "deactivate" };
        }

        this.#dispatcher.dispatch(envelope);
        return { type: "message" };
      }

      if (envelope.type === "notification") {
        if (envelope.method === "$/exit") {
          this.#phase = "exiting";
          this.#shutdownAbort.abort();
          return { type: "exit" };
        }
        stderr(`[bootstrap] notification: ${envelope.method}\n`);
        return { type: "message" };
      }

      return { type: "message" };
    }

    // Phase: deactivating — only $/exit accepted
    if (this.#phase === "deactivating") {
      if (envelope.type === "notification" && envelope.method === "$/exit") {
        this.#phase = "exiting";
        this.#shutdownAbort.abort();
        return { type: "exit" };
      }
      // Reject requests during deactivation
      if (envelope.type === "request") {
        const req = envelope as any;
        const payload = encodeError(req.id, -32603, "plugin is shutting down");
        this.#writer.write(FrameType.Response, payload);
        return { type: "message" };
      }
      return { type: "message" };
    }

    return { type: "message" };
  }

  // ── private ─────────────────────────────────────────────────

  #handleInitialize(id: string, params: Record<string, unknown>): InitParams {
    const wireVersion = Number(params.wireVersion ?? 1);
    const sessionId = String(params.sessionId ?? "");
    const plugin = params.plugin as Record<string, unknown> | undefined;
    const pluginId = String(plugin?.id ?? "unknown");
    const pluginVersion = String(plugin?.version ?? "0.0.0");
    const paths = params.paths as Record<string, unknown> | undefined;
    const extensionPath = String(paths?.extensionPath ?? ".");
    const entryPath = String(paths?.entryPath ?? ".");
    const storagePath = String(paths?.storagePath ?? ".");

    stderr(`[bootstrap] $/initialize: sessionId=${sessionId} plugin=${pluginId}\n`);

    const result = {
      wireVersion,
      runtimeVersion: "0.1.0",
      sessionId,
      plugin: { id: pluginId, version: pluginVersion },
    };
    const responsePayload = encodeSuccess(id, result);
    this.#writer.write(FrameType.Response, responsePayload);

    return { wireVersion, sessionId, pluginId, pluginVersion, extensionPath, entryPath, storagePath };
  }
}
