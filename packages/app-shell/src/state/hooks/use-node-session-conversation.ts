import { useQuery } from "@tanstack/react-query";
import { loadSessionConversation, type ChatTurn } from "@ora/chat";
import type { WorkflowNodeConversationItem } from "@ora/workflow-runtime";
import { useContractsClient } from "../../contracts-client-context";

/**
 * Lazy-loads one node's real Ora session transcript and projects it into the
 * node-conversation shape.
 *
 * A workflow agent node runs against a normal session (`WorkflowNodeRun.sessionId`),
 * so its history is replayed through the same session loader as the chat pane and
 * rendered identically by `RunNodeConversation`. Loading is gated on `enabled` so
 * the theater only opens the stream when the node's conversation dock is open.
 */
export function useNodeSessionConversation(
  runId: string,
  nodeId: string,
  sessionId: string | null | undefined,
  enabled: boolean,
) {
  const client = useContractsClient();
  return useQuery({
    queryKey: ["workflowRun", "nodeSession", runId, nodeId, sessionId ?? ""],
    queryFn: async () => {
      if (sessionId == null || sessionId === "") {
        return [] as WorkflowNodeConversationItem[];
      }
      const conversation = await loadSessionConversation(client.session, sessionId);
      return turnsToNodeConversationItems(conversation.turns, runId, nodeId, sessionId);
    },
    enabled: enabled && sessionId != null && sessionId !== "",
    // A replayed transcript is immutable; reopening the dock reuses the cache.
    staleTime: Number.POSITIVE_INFINITY,
  });
}

/** Projects loaded chat turns onto the compact node-conversation item shape.
 *
 * Text messages keep the chat layout; thoughts and tool calls become folded
 * activities. Plans and non-text content blocks are omitted from the node card.
 */
export function turnsToNodeConversationItems(
  turns: ChatTurn[],
  runId: string,
  nodeId: string,
  sessionId: string,
): WorkflowNodeConversationItem[] {
  const items: WorkflowNodeConversationItem[] = [];
  for (const turn of turns) {
    items.push(toMessageItem(turn.userMessage, runId, nodeId, sessionId));
    for (const item of turn.items) {
      if (item.kind === "message") {
        items.push(toMessageItem(item, runId, nodeId, sessionId));
      } else if (item.kind === "thought") {
        items.push({
          kind: "activity",
          id: item.id,
          runId,
          nodeId,
          sessionId,
          activityKind: "thought",
          summary: item.content,
          status: "complete",
          createdAt: toIso(item.createdAt),
          updatedAt: toIso(item.createdAt),
        });
      } else if (item.kind === "toolCall") {
        items.push({
          kind: "activity",
          id: item.id,
          runId,
          nodeId,
          sessionId,
          activityKind: "tool",
          summary: item.title,
          status: "complete",
          createdAt: toIso(item.createdAt),
          updatedAt: toIso(item.updatedAt),
        });
      }
      // ChatPlan and ChatContent have no compact node representation; skip them.
    }
  }
  return items;
}

/** Adapts one chat text message to the node-conversation message shape. */
function toMessageItem(
  message: { id: string; role: "user" | "assistant"; content: string; createdAt: number },
  runId: string,
  nodeId: string,
  sessionId: string,
): WorkflowNodeConversationItem {
  const timestamp = toIso(message.createdAt);
  return {
    kind: "message",
    id: message.id,
    runId,
    nodeId,
    sessionId,
    role: message.role,
    markdown: message.content,
    status: "complete",
    createdAt: timestamp,
    updatedAt: timestamp,
  };
}

/** Converts the chat store's epoch-millis timestamp to the ISO string form. */
function toIso(millis: number): string {
  return new Date(millis).toISOString();
}
