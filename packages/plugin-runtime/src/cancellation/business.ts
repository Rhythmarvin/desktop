/**
 * Business safety cancel — `agent.cancelConversation` handler.
 *
 * This is a safety-control method that uses an independent executor, separate
 * slot/byte reserve, and NEVER returns `-32010 ServerBusy`. If the safety
 * executor is saturated or any safety path fails, the connection is
 * terminated (fatal) and the Host completes as `UnknownOutcome(CancellationUnconfirmed)`.
 */

export type CancelConversationHandler = (
  params: Record<string, unknown>,
  requestId: string,
) => Promise<{ disposition: "accepted" | "alreadyStopped" }>;

/**
 * Safety executor for `cancelConversation`.
 *
 * Key invariants:
 * - Capacity ≥ maxActiveTurns (so a cancel slot is always available).
 * - Never returns busy — saturation is fatal (Job terminate).
 * - `Accepted` only after the target turn's `finishReason=cancelled` terminal
 *   has been Written.
 * - `AlreadyStopped` when the target was already completed/limit/no active turn.
 */
export function createSafetyExecutor(
  capacity: number,
  onFatal: (reason: string) => void,
) {
  let active = 0;

  function hasCapacity(): boolean {
    return active < capacity;
  }

  function activeCount(): number {
    return active;
  }

  async function execute(
    handler: CancelConversationHandler,
    params: Record<string, unknown>,
    requestId: string,
  ): Promise<{ disposition: "accepted" | "alreadyStopped" }> {
    if (active >= capacity) {
      // Safety saturation is fatal — we cannot return Busy for a safety method
      onFatal("safety executor saturated for cancelConversation");
      // Return a rejected promise; caller should terminate the Job
      throw new Error("Safety executor saturated — fatal");
    }

    active += 1;
    try {
      return await handler(params, requestId);
    } finally {
      active -= 1;
    }
  }

  function cancelAll(): void {
    // Safety handlers are not interruptible — they must complete or the Job is killed
  }

  return { execute, hasCapacity, activeCount, cancelAll };
}

/**
 * Create a `cancelConversation` business handler.
 *
 * The handler must:
 * 1. Check if the target conversation has an active turn.
 * 2. If no active turn → return `{ disposition: "alreadyStopped" }`.
 * 3. If active turn → request cancellation, wait for `finishReason=cancelled`
 *    terminal → return `{ disposition: "accepted" }`.
 * 4. Concurrent duplicate cancels join the same stop future.
 */
export function createCancelConversationHandler(
  getActiveTurn: (conversationId: string) => { abort: () => void } | null,
  waitForTerminal: (conversationId: string, timeoutMs: number) => Promise<"cancelled" | "completed" | "limit" | "timeout">,
  graceTimeoutMs = 5000,
): CancelConversationHandler {
  // Track in-progress stop futures to de-duplicate concurrent cancels
  const activeStops = new Map<string, Promise<{ disposition: "accepted" | "alreadyStopped" }>>();

  return async (params, _requestId) => {
    const conversationId = params.conversationId as string;
    if (!conversationId) {
      throw new Error("missing conversationId in cancelConversation params");
    }

    // Join existing stop future if a concurrent cancel is in progress
    const existing = activeStops.get(conversationId);
    if (existing) {
      return existing;
    }

    const stopPromise = (async (): Promise<{ disposition: "accepted" | "alreadyStopped" }> => {
      const activeTurn = getActiveTurn(conversationId);
      if (!activeTurn) {
        return { disposition: "alreadyStopped" };
      }

      // Request cancellation of the active turn
      activeTurn.abort();

      // Wait for the terminal
      const outcome = await waitForTerminal(conversationId, graceTimeoutMs);
      if (outcome === "cancelled") {
        return { disposition: "accepted" };
      }
      if (outcome === "timeout") {
        throw new Error("cancelConversation grace timeout — safety fatal");
      }
      // completed or limit — the turn finished before cancel took effect
      return { disposition: "alreadyStopped" };
    })();

    activeStops.set(conversationId, stopPromise);

    try {
      return await stopPromise;
    } finally {
      activeStops.delete(conversationId);
    }
  };
}
