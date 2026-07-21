// session.ts — Bootstrap session: state machine, handshake, message routing.

import { FrameType } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import {
  encodeSuccess,
  parseInbound,
  type RpcRequest,
  type RpcNotification,
} from "../rpc/envelope.js";
import { RequestDispatcher } from "../rpc/dispatcher.js";

// ── Types ────────────────────────────────────────────────────────

export interface SessionIo {
  stdin: AsyncIterable<Uint8Array>;
}

interface InitParams {
  wireVersion: number;
  sessionId: string;
  pluginId: string;
  pluginVersion: string;
  extensionPath: string;
  entryPath: string;
  storagePath: string;
}

type Phase = "awaitingInitialize" | "running" | "exiting";

// ── Session ──────────────────────────────────────────────────────

export class BootstrapSession {
  readonly #writer: ProtocolWriter;
  readonly #dispatcher: RequestDispatcher;
  #phase: Phase = "awaitingInitialize";
  #shutdownAbort = new AbortController();

  constructor(writer: ProtocolWriter, dispatcher: RequestDispatcher) {
    this.#writer = writer;
    this.#dispatcher = dispatcher;
  }

  get phase(): Phase { return this.#phase; }
  get shutdownSignal(): AbortSignal { return this.#shutdownAbort.signal; }

  /** Processes one incoming frame. Returns the init params when handshake completes. */
  processFrame(data: Uint8Array): { type: "handshake"; params: InitParams } | { type: "message" } | { type: "exit" } {
    const envelope = parseInbound({
      type: data[4] as FrameType,
      payload: data.slice(5),
    });

    // Phase: awaitingInitialize
    if (this.#phase === "awaitingInitialize") {
      if (envelope.type !== "request" || envelope.method !== "$/initialize") {
        throw new Error("first Host frame must be $/initialize Request");
      }

      const params = (envelope.params ?? {}) as Record<string, unknown>;
      const initParams = this.#handleInitialize(envelope.id, params);

      this.#phase = "running";
      return { type: "handshake", params: initParams };
    }

    // Phase: running
    if (envelope.type === "request") {
      if (envelope.method === "$/initialize") {
        throw new Error("$/initialize cannot be repeated");
      }

      // Route to dispatcher
      this.#dispatcher.dispatch(envelope);
      return { type: "message" };
    }

    if (envelope.type === "notification") {
      if (envelope.method === "$/exit") {
        this.#phase = "exiting";
        this.#shutdownAbort.abort();
        return { type: "exit" };
      }
      // Other notifications: log and ignore for MVP
      const stderr = process.stderr.write.bind(process.stderr);
      stderr(`[bootstrap] notification: ${envelope.method}\n`);
      return { type: "message" };
    }

    // Responses go to pending callers (handled externally for MVP)
    return { type: "message" };
  }

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

    const stderr = process.stderr.write.bind(process.stderr);
    stderr(`[bootstrap] $/initialize: sessionId=${sessionId} plugin=${pluginId}\n`);

    // Send response
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
