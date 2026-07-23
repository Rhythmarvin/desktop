// agent.ts — Agent Plugin Bootstrap
//
// runAgentBootstrap() is the entry point for agent-type plugins.
// It starts the agent process and ACP initialize handshake concurrently while
// immediately handling the Host's $/initialize / $/activate handshake.
//
// Flow:
//   1. Spawn agent process + start ACP initialize (background)
//   2. Immediately start Host read loop, respond to $/initialize, $/activate
//   3. On acp/connect → wait for ACP initialize → return capabilities
//   4. On acp/forward → unpack ACP message → forward to agent → return response
//   5. Agent session/update → forward as acp/event Notification to Host

import { FrameDecoder, FrameType } from "../transport/frame.js";
import { ProtocolWriter } from "../transport/writer.js";
import { RequestDispatcher, type RequestHandlerWithId } from "../rpc/dispatcher.js";
import { parseInbound } from "../rpc/envelope.js";

const stderr = process.stderr.write.bind(process.stderr);

// ── AgentPeer ──────────────────────────────────────────────────────

interface PendingEntry {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
}

/**
 * Manages NDJSON communication with one ACP agent child process.
 */
class AgentPeer {
  #process: { stdin: { write(data: Uint8Array): void }; stdout: AsyncIterable<Uint8Array>; kill(): void };
  #pending = new Map<number | string, PendingEntry>();
  #nextId = 1;
  #agentInfo: Record<string, unknown> | null = null;
  #agentCapabilities: Record<string, unknown> | null = null;
  #sessionUpdateCbs = new Map<string, (event: Record<string, unknown>) => void>();
  #running = false;

  constructor(proc: { stdin: { write(data: Uint8Array): void }; stdout: AsyncIterable<Uint8Array>; kill(): void }) {
    this.#process = proc;
  }

  get agentInfo() { return this.#agentInfo; }
  get agentCapabilities() { return this.#agentCapabilities; }

  /** Called after ACP initialize completes to cache capabilities. */
  setHandshakeResult(info: Record<string, unknown> | null, caps: Record<string, unknown> | null): void {
    this.#agentInfo = info;
    this.#agentCapabilities = caps;
  }

  startReader(): void {
    if (this.#running) return;
    this.#running = true;
    this.#readLoop().catch((err) => {
      stderr(`[agent-peer] reader loop error: ${String(err)}\n`);
    });
  }

  async request(method: string, params: Record<string, unknown>): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const id = this.#nextId++;
      this.#pending.set(id, { resolve, reject });
      const frame = JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n";
      try {
        this.#process.stdin.write(new TextEncoder().encode(frame));
      } catch (err) {
        this.#pending.delete(id);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  notify(method: string, params: Record<string, unknown>): void {
    const frame = JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n";
    try {
      this.#process.stdin.write(new TextEncoder().encode(frame));
    } catch {
      // ignore write errors
    }
  }

  onSessionUpdate(sessionId: string, cb: (event: Record<string, unknown>) => void): void {
    this.#sessionUpdateCbs.set(sessionId, cb);
  }

  removeSessionUpdate(sessionId: string): void {
    this.#sessionUpdateCbs.delete(sessionId);
  }

  kill(): void {
    try { this.#process.kill(); } catch { /* ignore */ }
  }

  // ── private ──────────────────────────────────────────────────

  async #readLoop(): Promise<void> {
    const decoder = new TextDecoder();
    let buffer = "";

    try {
      for await (const value of this.#process.stdout) {
        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed) continue;
          try {
            this.#routeFrame(JSON.parse(trimmed));
          } catch {
            stderr(`[agent-peer] failed to parse: ${trimmed.slice(0, 100)}\n`);
          }
        }
      }
      stderr("[agent-peer] agent stdout closed\n");
    } catch (err) {
      stderr(`[agent-peer] read error: ${String(err)}\n`);
    } finally {
      for (const [, entry] of this.#pending) {
        entry.reject(new Error("agent process closed"));
      }
      this.#pending.clear();
    }
  }

  #routeFrame(msg: Record<string, unknown>): void {
    // Response: has id, has result or error
    if (typeof msg.id !== "undefined" && (msg.result !== undefined || msg.error !== undefined)) {
      const id = msg.id as number | string;
      const entry = this.#pending.get(id);
      if (entry) {
        this.#pending.delete(id);
        if (msg.error) {
          const err = msg.error as Record<string, unknown>;
          entry.reject(new Error(`ACP error: ${String(err.message)} (code: ${err.code})`));
        } else {
          entry.resolve(msg.result);
        }
      }
      return;
    }

    // Notification: session/update
    if (typeof msg.method === "string" && msg.method === "session/update") {
      const params = msg.params as Record<string, unknown> | undefined;
      const sessionId = params?.sessionId as string | undefined;
      if (sessionId) {
        const cb = this.#sessionUpdateCbs.get(sessionId);
        if (cb) {
          cb((params?.update ?? params ?? {}) as Record<string, unknown>);
        }
      }
      return;
    }

    stderr(`[agent-peer] unhandled: method=${String(msg.method ?? "?")}\n`);
  }
}

// ── Agent initializer (runs concurrently with Host handshake) ─────

async function initializeAgent(agentPath: string): Promise<AgentPeer> {
  stderr(`[agent-bootstrap] spawning agent: ${agentPath} acp\n`);

  const proc = Bun.spawn([agentPath, "acp"], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "inherit",
  });

  const peer = new AgentPeer(proc);
  peer.startReader();

  stderr("[agent-bootstrap] ACP initialize handshake\n");
  const initResult = (await peer.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
    clientInfo: { name: "ora", version: "0.1.0" },
  })) as Record<string, unknown>;

  if (initResult.protocolVersion !== 1) {
    throw new Error(`unsupported ACP version: ${initResult.protocolVersion}`);
  }

  peer.setHandshakeResult(
    (initResult.agentInfo as Record<string, unknown>) ?? null,
    (initResult.agentCapabilities as Record<string, unknown>) ?? null,
  );

  stderr(`[agent-bootstrap] ACP connected: ${JSON.stringify(initResult.agentInfo)}\n`);
  return peer;
}

// ── Active stream tracking ────────────────────────────────────────

interface ActiveStream {
  hostRequestId: string;
  agentSessionId: string;
}

// ── Bootstrap ─────────────────────────────────────────────────────

export async function runAgentBootstrap(): Promise<void> {
  stderr("[agent-bootstrap] starting\n");

  const raw = (globalThis as Record<string, unknown>).__ora_plugin_config as Record<string, unknown> | undefined;
  const agentPath = (raw?.agentPath as string) ?? "opencode";

  // ── 1. Start ACP initialize in background (non-blocking) ───────
  const peerPromise = initializeAgent(agentPath);
  let peer: AgentPeer | null = null;

  // ── 2. Set up Host protocol (immediate) ───────────────────────
  const writer = new ProtocolWriter(process.stdout);
  const dispatcher = new RequestDispatcher(writer);
  const activeStreams = new Map<string, ActiveStream>();

  // --- $/initialize --- responds immediately (no ACP dependency)
  dispatcher.register("$/initialize", async (params: Record<string, unknown> | undefined) => {
    const p = params ?? {};
    const plugin = p.plugin as Record<string, unknown> | undefined;
    stderr(`[agent-bootstrap] $/initialize: sessionId=${String(p.sessionId ?? "?")}\n`);
    return {
      wireVersion: p.wireVersion ?? 1,
      runtimeVersion: "0.1.0",
      sessionId: p.sessionId,
      plugin: { id: plugin?.id ?? "unknown", version: plugin?.version ?? "0.1.0" },
    };
  });

  // --- $/activate --- responds immediately
  dispatcher.register("$/activate", async () => {
    stderr("[agent-bootstrap] $/activate\n");
    return { providers: [] };
  });

  // --- $/deactivate --- graceful shutdown
  dispatcher.register("$/deactivate", async () => {
    stderr("[agent-bootstrap] $/deactivate\n");
    if (peer) {
      // Clean up all active streams
      for (const [, stream] of activeStreams) {
        peer.removeSessionUpdate(stream.agentSessionId);
      }
      activeStreams.clear();
      peer.kill();
      peer = null;
    }
    return null;
  });

  // --- acp/connect --- wait for ACP initialize if needed
  dispatcher.register("acp/connect", async () => {
    if (!peer) {
      stderr("[agent-bootstrap] waiting for ACP initialize\n");
      peer = await peerPromise;
      stderr("[agent-bootstrap] ACP peer ready\n");
    }
    return {
      status: "connected",
      agentInfo: peer.agentInfo,
      agentCapabilities: peer.agentCapabilities,
    };
  });

  // --- acp/forward ---
  const forwardHandler: RequestHandlerWithId = async (params, hostRequestId) => {
    if (!peer) peer = await peerPromise;

    const p = params ?? {};
    const agentSessionId = String(p.agentSessionId ?? "");
    const message = p.message as { method: string; params: Record<string, unknown> } | undefined;

    if (!message?.method) {
      throw new Error("acp/forward requires message.method");
    }

    const isStreaming = message.method === "session/prompt" || message.method === "session/load";

    if (isStreaming && agentSessionId) {
      peer.onSessionUpdate(agentSessionId, (update: Record<string, unknown>) => {
        const eventPayload = new TextEncoder().encode(JSON.stringify({
          jsonrpc: "2.0",
          method: "acp/event",
          params: {
            agentSessionId,
            requestId: hostRequestId,
            event: { type: "session_update", update },
          },
        }));
        writer.write(FrameType.Notification, eventPayload);
      });

      activeStreams.set(agentSessionId, { hostRequestId, agentSessionId });
    }

    try {
      const result = await peer.request(message.method, message.params);

      if (isStreaming && agentSessionId) {
        peer.removeSessionUpdate(agentSessionId);
        activeStreams.delete(agentSessionId);

        const completedPayload = new TextEncoder().encode(JSON.stringify({
          jsonrpc: "2.0",
          method: "acp/event",
          params: {
            agentSessionId,
            requestId: hostRequestId,
            event: {
              type: "completed",
              stopReason: (result as Record<string, unknown>)?.stopReason ?? "end_turn",
            },
          },
        }));
        writer.write(FrameType.Notification, completedPayload);
      }

      return { agentSessionId, response: result };
    } catch (err) {
      if (isStreaming && agentSessionId) {
        peer.removeSessionUpdate(agentSessionId);
        activeStreams.delete(agentSessionId);

        const errMsg = err instanceof Error ? err.message : String(err);
        const errPayload = new TextEncoder().encode(JSON.stringify({
          jsonrpc: "2.0",
          method: "acp/event",
          params: {
            agentSessionId,
            requestId: hostRequestId,
            event: { type: "error", code: -32603, message: errMsg },
          },
        }));
        writer.write(FrameType.Notification, errPayload);
      }
      throw err;
    }
  };
  dispatcher.register("acp/forward", forwardHandler);

  // --- acp/cancel ---
  dispatcher.register("acp/cancel", async (params: Record<string, unknown> | undefined) => {
    if (!peer) return { cancelled: true };
    const p = params ?? {};
    const agentSessionId = String(p.agentSessionId ?? "");
    peer.notify("session/cancel", { sessionId: agentSessionId });
    peer.removeSessionUpdate(agentSessionId);
    activeStreams.delete(agentSessionId);
    return { cancelled: true };
  });

  // ── 3. Host stdin read loop ──────────────────────────────────
  const decoder = new FrameDecoder();

  try {
    for await (const chunk of process.stdin) {
      const bytes = typeof chunk === "string"
        ? new TextEncoder().encode(chunk)
        : new Uint8Array(chunk as ArrayBuffer);

      for (const frame of decoder.decodeChunk(bytes)) {
        const envelope = parseInbound({
          type: frame.type as FrameType,
          payload: frame.payload,
        });

        if (envelope.type === "request") {
          dispatcher.dispatch(envelope);
        } else if (envelope.type === "notification") {
          if (envelope.method === "$/exit") {
            stderr("[agent-bootstrap] received $/exit, shutting down\n");
            if (peer) { peer.kill(); peer = null; }
            process.exit(0);
          }
          stderr(`[agent-bootstrap] Host notification: ${envelope.method}\n`);
        }
      }
    }
  } catch (err) {
    stderr(`[agent-bootstrap] stdin error: ${String(err)}\n`);
  } finally {
    if (peer) { peer.kill(); }
    process.exit(0);
  }
}
