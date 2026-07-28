import { useEffect } from "react";
import type { ContractsClient } from "@ora/contracts";
import type { QueryClient } from "@tanstack/react-query";
import { queryKeys } from "./hooks/query-keys";

/**
 * Subscribes to backend-pushed northbound events and invalidates the
 * relevant TanStack Query caches so the UI re-fetches affected data.
 */
export function useNorthbound(
  client: ContractsClient,
  queryClient: QueryClient,
): void {
  useEffect(() => {
    const unsubscribe = client.northbound.subscribe((event) => {
      switch (event.type) {
        case "session_title_updated":
          queryClient.invalidateQueries({ queryKey: queryKeys.sessions });
          break;
      }
    });
    return unsubscribe;
  }, [client, queryClient]);
}
