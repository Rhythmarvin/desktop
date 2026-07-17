/**
 * Ordinary executor — bounded handler execution with AsyncGenerator stream support.
 *
 * - Max `capacity` concurrent invocations; overflow returns `-32010 ServerBusy`.
 * - Streaming methods (startConversation, sendMessage) are driven by calling
 *   `AsyncGenerator.next()` serially; each event is enqueued before the next
 *   `next()` call. Terminal `causal_after_seq` ensures all stream frames are
 *   written before the terminal Response.
 */

/** Callbacks for streaming method output. */
export interface StreamCallbacks {
  /** Send a stream event notification ($/stream). */
  sendStream: (requestId: string, seq: number, value: unknown) => Promise<void>;
  /** Send the terminal Response with the AgentTurnResult. */
  sendTerminal: (requestId: string, result: unknown) => void;
}

/** A handler function that may be streaming (AsyncGenerator) or non-streaming (Promise). */
export type AgentHandler = (
  requestId: string,
  signal: AbortSignal,
  params: Record<string, unknown>,
) => Promise<unknown> | AsyncGenerator<unknown, unknown, void>;

interface ActiveSlot {
  requestId: string;
  controller: AbortController;
}

export function createOrdinaryExecutor(
  capacity: number,
  streamCallbacks: StreamCallbacks,
  sendBusyResponse: (requestId: string) => void,
) {
  let active = 0;
  const activeMap = new Map<string, ActiveSlot>();

  /** Check if there is capacity for a new invocation. */
  function hasCapacity(): boolean {
    return active < capacity;
  }

  /** Get the number of currently active invocations. */
  function activeCount(): number {
    return active;
  }

  /** Start a new invocation. The handler result is wired to the callbacks. */
  function invoke(
    requestId: string,
    signal: AbortSignal,
    params: Record<string, unknown>,
    handler: AgentHandler,
  ): void {
    if (active >= capacity) {
      sendBusyResponse(requestId);
      return;
    }

    active += 1;
    const slot: ActiveSlot = { requestId, controller: new AbortController() };
    activeMap.set(requestId, slot);

    // Wire external signal to internal controller
    const onAbort = () => slot.controller.abort();
    signal.addEventListener("abort", onAbort, { once: true });

    const done = () => {
      active -= 1;
      activeMap.delete(requestId);
      signal.removeEventListener("abort", onAbort);
    };

    Promise.resolve()
      .then(() => handler(requestId, slot.controller.signal, params))
      .then(async (result) => {
        if (isAsyncGenerator(result)) {
          // ── Streaming path ──────────────────────────────────
          await driveGenerator(requestId, result as AsyncGenerator<unknown, unknown, void>);
        } else {
          // ── Non-streaming path ──────────────────────────────
          streamCallbacks.sendTerminal(requestId, result);
        }
        done();
      })
      .catch((err) => {
        // Handler threw — send error terminal
        const e = err as Error & { kind?: string };
        if (e.name === "AgentBusinessError") {
          streamCallbacks.sendTerminal(requestId, {
            __error: true,
            code: -32000,
            message: e.message,
            kind: e.kind,
          });
        } else {
          streamCallbacks.sendTerminal(requestId, {
            __error: true,
            code: -32000,
            message: e.message,
            kind: "providerFailure",
          });
        }
        done();
      });
  }

  /** Drive an AsyncGenerator serially — exactly one next() at a time. */
  async function driveGenerator(
    gen: AsyncGenerator<unknown, unknown, void>,
  ): Promise<void> {
    let seq = 0;

    try {
      let iterResult = await gen.next();
      while (!iterResult.done) {
        seq += 1;
        // Enqueue the stream event
        await streamCallbacks.sendStream("", seq, iterResult.value);

        iterResult = await gen.next();
      }

      // Terminal: generator return value (AgentTurnResult)
      streamCallbacks.sendTerminal("", iterResult.value);
    } catch (err) {
      const e = err as Error & { kind?: string };
      streamCallbacks.sendTerminal("", {
        __error: true,
        code: e.name === "AgentBusinessError" ? -32000 : -32000,
        message: e.message,
        kind: e.name === "AgentBusinessError" ? e.kind : "providerFailure",
      });
    }
  }

  /** Cancel all active invocations (used on shutdown). */
  function cancelAll(): void {
    for (const [, slot] of activeMap) {
      slot.controller.abort();
    }
    activeMap.clear();
    active = 0;
  }

  return { invoke, hasCapacity, activeCount, cancelAll };
}

/** Type guard: check if a value is an AsyncGenerator. */
function isAsyncGenerator(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const obj = value as Record<string | symbol, unknown>;
  return typeof obj[Symbol.asyncIterator] === "function" && typeof obj.next === "function";
}
