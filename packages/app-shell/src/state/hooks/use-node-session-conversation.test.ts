import { describe, expect, it } from "vitest";
import type { ChatTurn } from "@ora/chat";
import { turnsToNodeConversationItems } from "./use-node-session-conversation";

const RUN_ID = "run-1";
const NODE_ID = "agent-1";
const SESSION_ID = "session-1";

/** Builds a completed turn with one user message and mixed agent items. */
function turnFixture(): ChatTurn {
  return {
    id: "t1",
    userMessage: {
      kind: "message",
      id: "u1",
      role: "user",
      content: "帮我审查",
      createdAt: 10,
    },
    items: [
      {
        kind: "message",
        id: "a1",
        role: "assistant",
        content: "好的，开始审查",
        createdAt: 20,
      },
      {
        kind: "thought",
        id: "th1",
        content: "分析中",
        createdAt: 25,
      },
      {
        kind: "toolCall",
        id: "tool1",
        title: "读取文件",
        content: [],
        locations: [],
        status: "completed",
        createdAt: 26,
        updatedAt: 28,
      },
    ],
    status: "completed",
    stopReason: "end_turn",
    error: null,
    createdAt: 10,
  };
}

describe("turnsToNodeConversationItems", () => {
  it("projects messages and folds thoughts and tool calls into activities", () => {
    const items = turnsToNodeConversationItems([turnFixture()], RUN_ID, NODE_ID, SESSION_ID);

    expect(items).toEqual([
      {
        kind: "message",
        id: "u1",
        runId: RUN_ID,
        nodeId: NODE_ID,
        sessionId: SESSION_ID,
        role: "user",
        markdown: "帮我审查",
        status: "complete",
        createdAt: new Date(10).toISOString(),
        updatedAt: new Date(10).toISOString(),
      },
      {
        kind: "message",
        id: "a1",
        runId: RUN_ID,
        nodeId: NODE_ID,
        sessionId: SESSION_ID,
        role: "assistant",
        markdown: "好的，开始审查",
        status: "complete",
        createdAt: new Date(20).toISOString(),
        updatedAt: new Date(20).toISOString(),
      },
      {
        kind: "activity",
        id: "th1",
        runId: RUN_ID,
        nodeId: NODE_ID,
        sessionId: SESSION_ID,
        activityKind: "thought",
        summary: "分析中",
        status: "complete",
        createdAt: new Date(25).toISOString(),
        updatedAt: new Date(25).toISOString(),
      },
      {
        kind: "activity",
        id: "tool1",
        runId: RUN_ID,
        nodeId: NODE_ID,
        sessionId: SESSION_ID,
        activityKind: "tool",
        summary: "读取文件",
        status: "complete",
        createdAt: new Date(26).toISOString(),
        updatedAt: new Date(28).toISOString(),
      },
    ]);
  });
});
