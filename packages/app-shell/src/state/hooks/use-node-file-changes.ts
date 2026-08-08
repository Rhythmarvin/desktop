import { useQuery } from "@tanstack/react-query";
import { loadSessionConversation } from "@ora/chat";
import { collectTurnDiffFiles, type TurnDiffFile } from "../../features/chat/turn-diff-files";
import { useContractsClient } from "../../contracts-client-context";

/**
 * Lazy-loads the file changes a workflow node's session made.
 *
 * A node runs against a normal Ora session, so its tool-call history replays
 * through the chat session loader and the same `collectTurnDiffFiles` extraction
 * the chat pane uses to show per-turn file edits. Gated on `enabled` so the
 * inspector only opens the stream for the focused node.
 */
export function useNodeFileChanges(
  sessionId: string | null | undefined,
  enabled: boolean,
) {
  const client = useContractsClient();
  return useQuery({
    queryKey: ["workflowRun", "nodeFileChanges", sessionId ?? ""],
    queryFn: async () => {
      if (sessionId == null || sessionId === "") {
        return [] as TurnDiffFile[];
      }
      const conversation = await loadSessionConversation(client.session, sessionId);
      return conversation.turns.flatMap((turn) => collectTurnDiffFiles(turn));
    },
    enabled: enabled && sessionId != null && sessionId !== "",
    // A replayed transcript is immutable; revisiting the node reuses the cache.
    staleTime: Number.POSITIVE_INFINITY,
  });
}
