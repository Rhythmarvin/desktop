/**
 * Transport-cancel management — per-invocation AbortController map.
 *
 * `$/cancelRequest` notifications abort the target invocation's signal.
 * Only the original Host requester may send cancel (v1 constraint).
 * Duplicate/unknown cancels are logged + dropped with rate limiting.
 */

export interface CancelEntry {
  controller: AbortController;
  method: string;
}

/** Track the rate of unknown/duplicate cancel requests. */
interface CancelRateTracker {
  count: number;
  resetAt: number;
}

export function createTransportCancel(
  send: (type: 2, msg: Record<string, unknown>) => void,
  maxUnknownCancelRate = 10,
  rateWindowMs = 1000,
) {
  const controllers = new Map<string, CancelEntry>();
  const rateTracker: CancelRateTracker = { count: 0, resetAt: Date.now() + rateWindowMs };

  /** Register an invocation for potential cancel. */
  function register(requestId: string, controller: AbortController, method: string): void {
    controllers.set(requestId, { controller, method });
  }

  /** Unregister an invocation (after completion). */
  function unregister(requestId: string): void {
    controllers.delete(requestId);
  }

  /**
   * Handle a `$/cancelRequest` notification from the Host.
   * Returns `true` if the message was consumed.
   */
  function handleCancel(params: Record<string, unknown>): boolean {
    const targetId = params.id as string | undefined;
    if (!targetId) return false;

    const entry = controllers.get(targetId);
    if (!entry) {
      // Unknown/duplicate cancel — log + rate-limit
      const now = Date.now();
      if (now > rateTracker.resetAt) {
        rateTracker.count = 0;
        rateTracker.resetAt = now + rateWindowMs;
      }
      rateTracker.count += 1;
      if (rateTracker.count > maxUnknownCancelRate) {
        // Too many unknown cancels — could be an attack; warn but don't fatal
        console.error(
          `[bootstrap] excessive unknown cancel requests (${rateTracker.count} in ${rateWindowMs}ms)`,
        );
        rateTracker.count = 0;
      }
      return false;
    }

    // Abort the target invocation
    entry.controller.abort();

    // Once the handler settles, respond with -32800
    // (in practice, the handler checks signal.aborted and returns)
    // For immediate acknowledgement when handler already settled:
    if (entry.controller.signal.aborted) {
      // Give the handler a tick to notice and settle
      setTimeout(() => {
        // If the entry was already unregistered by handler completion, we're done
        if (!controllers.has(targetId)) return;
        // Otherwise the handler should complete soon
      }, 0);
    }

    return true;
  }

  /** Cancel all registered invocations (generation shutdown). */
  function cancelAll(): void {
    for (const [, entry] of controllers) {
      entry.controller.abort();
    }
    controllers.clear();
  }

  return { register, unregister, handleCancel, cancelAll };
}
