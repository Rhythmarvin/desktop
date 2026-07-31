import { act, renderHook } from "@testing-library/react";
import type { Northbound } from "@ora/contracts";
import { describe, expect, it, vi } from "vitest";
import { createMockClient, createMockClientState } from "../test/mock-client";
import { createTestQueryClient } from "../test/hook-harness";
import { queryKeys } from "./hooks/query-keys";
import { useNorthbound } from "./subscribe-northbound";

describe("useNorthbound", () => {
  it("invalidates sessions for title events and lossy transport recovery", () => {
    const client = createMockClient(createMockClientState());
    const queryClient = createTestQueryClient();
    const invalidateQueries = vi.spyOn(queryClient, "invalidateQueries");
    let handleEvent: ((event: Northbound) => void) | undefined;
    let handleOutOfSync: (() => void) | undefined;
    const unsubscribe = vi.fn();
    client.northbound.subscribe = (eventHandler, outOfSyncHandler) => {
      handleEvent = eventHandler;
      handleOutOfSync = outOfSyncHandler;
      return unsubscribe;
    };
    const rendered = renderHook(() => useNorthbound(client, queryClient));

    act(() => {
      handleEvent?.({
        type: "session_title_updated",
        session_id: "session-1",
        title: "New title",
      });
      handleOutOfSync?.();
    });

    expect(invalidateQueries).toHaveBeenNthCalledWith(1, {
      queryKey: queryKeys.sessions,
    });
    expect(invalidateQueries).toHaveBeenNthCalledWith(2, {
      queryKey: queryKeys.sessions,
    });

    rendered.unmount();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });
});
