import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { AgentActivityDots } from "../../components/agent-activity-dots";
import { useTranslation } from "react-i18next";
import { AnchorHighlight } from "./anchor-highlight";
import {
  ConversationNavigator,
  type ConversationNavigationPresentation,
} from "./conversation-navigator";
import { useConversationNavigation } from "./conversation-navigation";
import { MessageBubble } from "./message-bubble";
import { ResponseTurn } from "./response-turn";
import type { ChatModelChange, ChatTurn } from "@ora/chat";
import { useTaskWorkspace } from "../../state/hooks/use-task-workspace";
import { useWorkspaceCwd } from "../../state/hooks/use-workspace-cwd";
import {
  collectCumulativeArtifactIndices,
  type TurnArtifactCacheEntry,
} from "./chat-link/artifact-index";
import { ChatLinkContext } from "./chat-link/context";

interface MessageListProps {
  turns: ChatTurn[];
  /** Model switches to draw between the turns they happened after. */
  modelChanges?: ChatModelChange[];
  userName: string;
  isResponding: boolean;
  taskId?: string;
  projectId?: string;
  workspaceId?: string;
  /** Optional presentation override for chats embedded inside another surface. */
  conversationNavigation?: ConversationNavigationPresentation;
}

const EMPTY_MODEL_CHANGES: ChatModelChange[] = [];
const VIRTUALIZATION_MIN_TURNS = 40;
const ESTIMATED_TURN_HEIGHT_PX = 260;
const VIRTUAL_TURN_OVERSCAN = 3;
const HIDDEN_SURFACE_FALLBACK_TURNS = 8;

/** Keeps a hidden or not-yet-measured chat useful without mounting its complete transcript. */
function fallbackVirtualTurns(turnCount: number): Array<{
  index: number;
  start: number;
}> {
  const firstIndex = Math.max(0, turnCount - HIDDEN_SURFACE_FALLBACK_TURNS);
  return Array.from({ length: turnCount - firstIndex }, (_, offset) => {
    const index = firstIndex + offset;
    return { index, start: index * ESTIMATED_TURN_HEIGHT_PX };
  });
}

/** The scrollable turn thread, kept pinned to live ACP activity unless the reader scrolls away. */
export function MessageList({
  turns,
  modelChanges = EMPTY_MODEL_CHANGES,
  userName,
  isResponding,
  taskId,
  projectId,
  workspaceId,
  conversationNavigation,
}: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const workspaceQuery = useTaskWorkspace(taskId);
  const workspaceCwdQuery = useWorkspaceCwd(
    taskId === undefined ? workspaceId : undefined,
  );
  const cwd = workspaceQuery.data?.rootPath ?? workspaceCwdQuery.data ?? null;
  const [artifactCache] = useState(
    () => new Map<string, TurnArtifactCacheEntry>(),
  );
  const linksUseArtifactIndex = taskId !== undefined || projectId !== undefined;
  const artifactIndices = useMemo(
    () =>
      linksUseArtifactIndex
        ? collectCumulativeArtifactIndices(turns, artifactCache)
        : [],
    [artifactCache, linksUseArtifactIndex, turns],
  );
  const artifactIndex = useMemo(
    () => artifactIndices.at(-1) ?? { edited: [], referenced: [] },
    [artifactIndices],
  );
  const chatLinkValue = useMemo(() => {
    if (taskId !== undefined) {
      return { index: artifactIndex, taskId, cwd };
    }
    if (projectId !== undefined) {
      return { index: artifactIndex, cwd };
    }
    return null;
  }, [artifactIndex, cwd, projectId, taskId]);
  const lastTurn = turns.at(-1);
  const lastAnchorId =
    lastTurn === undefined
      ? null
      : `${lastTurn.id}:${lastTurn.items.length === 0 && lastTurn.status === "streaming" ? "user" : "response"}`;
  const lastItem = lastTurn?.items.at(-1);
  const lastUserMessageId = lastTurn?.userMessage.id;
  // Hide the running indicator while the answer itself is streaming: the growing
  // text already shows the agent is live, so a second "working" line under it
  // just reads as noise. It returns for thoughts, tool calls, and the waits between.
  const streamingBody =
    lastItem?.kind === "message" && lastItem.role === "assistant";
  const showRunning = isResponding && !streamingBody;
  const navigation = useConversationNavigation({
    scrollRef,
    contentRef,
    followTailKey: `${turns.length}:${lastUserMessageId ?? ""}`,
    lastAnchorId,
  });
  const virtualized = turns.length >= VIRTUALIZATION_MIN_TURNS;
  // TanStack Virtual owns an imperative measurement cache by design; callers use its stable
  // instance directly instead of asking React Compiler to memoize this component.
  // eslint-disable-next-line react-hooks/incompatible-library
  const turnVirtualizer = useVirtualizer({
    count: virtualized ? turns.length : 0,
    getScrollElement: () => scrollRef.current,
    getItemKey: (index) => turns[index]?.id ?? index,
    estimateSize: () => ESTIMATED_TURN_HEIGHT_PX,
    // A temporarily hidden Theater card reports zero height; retaining the estimate prevents its
    // virtual range from collapsing until the card becomes measurable again.
    measureElement: (element) =>
      element.getBoundingClientRect().height || ESTIMATED_TURN_HEIGHT_PX,
    overscan: VIRTUAL_TURN_OVERSCAN,
    // A first measurement is unavailable until the scroll element mounts. Supplying a realistic
    // initial viewport lets a cold long-history render mount only a small window immediately.
    initialRect: { width: 760, height: 800 },
    enabled: virtualized,
  });
  const turnIndexById = useMemo(
    () => new Map(turns.map((turn, index) => [turn.id, index])),
    [turns],
  );
  const measuredVirtualTurns = turnVirtualizer.getVirtualItems();
  const renderedVirtualTurns =
    measuredVirtualTurns.length > 0
      ? measuredVirtualTurns
      : fallbackVirtualTurns(turns.length);

  /** Materializes an off-screen virtual turn before handing its exact anchor to DOM navigation. */
  const navigateToAnchor = (anchorId: string): void => {
    if (!virtualized) {
      navigation.navigateToAnchor(anchorId);
      return;
    }
    const separator = anchorId.lastIndexOf(":");
    const turnId = separator === -1 ? anchorId : anchorId.slice(0, separator);
    const turnIndex = turnIndexById.get(turnId);
    if (turnIndex === undefined) return;
    turnVirtualizer.scrollToIndex(turnIndex, { align: "start" });
    window.requestAnimationFrame(() => navigation.navigateToAnchor(anchorId));
  };

  /** Renders one complete turn row; virtual and short transcripts deliberately share the markup. */
  const renderTurn = (turn: ChatTurn, index: number) => {
    const turnIndex = artifactIndices[index] ?? {
      edited: [],
      referenced: [],
    };
    const turnChatLinkValue =
      taskId !== undefined
        ? { index: turnIndex, taskId, cwd }
        : projectId !== undefined
          ? { index: turnIndex, cwd }
          : null;
    return (
      <>
        {modelChangesAt(modelChanges, index).map((change) => (
          <ModelChangeDivider key={change.id} modelName={change.modelName} />
        ))}
        <div data-turn-anchor={turn.id}>
          <div data-turn-user data-conversation-anchor={`${turn.id}:user`}>
            <MessageBubble message={turn.userMessage} userName={userName} />
          </div>
          {(turn.items.length > 0 || turn.status !== "streaming") && (
            <ChatLinkContext.Provider value={turnChatLinkValue}>
              <div
                data-turn-response
                data-conversation-anchor={`${turn.id}:response`}
                className="relative overflow-visible rounded-xl"
              >
                <AnchorHighlight />
                <ResponseTurn turn={turn} userName={userName} />
              </div>
            </ChatLinkContext.Provider>
          )}
        </div>
      </>
    );
  };

  return (
    <ChatLinkContext.Provider value={chatLinkValue}>
      <div className="relative min-h-0 flex-1">
        <div
          ref={scrollRef}
          onScroll={navigation.handleScroll}
          onWheel={(event) => navigation.handleWheel(event.deltaY)}
          onPointerDown={navigation.beginPointerScroll}
          onPointerUp={navigation.endPointerScroll}
          onPointerCancel={navigation.endPointerScroll}
          onTouchStart={navigation.beginPointerScroll}
          onTouchEnd={navigation.endPointerScroll}
          onTouchCancel={navigation.endPointerScroll}
          data-testid="message-list"
          aria-live="polite"
          className="scrollbar-hide h-full min-h-0 animate-in overflow-y-auto fade-in duration-500"
        >
          <div
            ref={contentRef}
            className="mx-auto w-full max-w-[760px] px-3 pb-4 pt-5 sm:px-5 sm:pt-8"
          >
            {virtualized ? (
              <div
                data-testid="virtualized-message-turns"
                className="relative w-full"
                style={{ height: turnVirtualizer.getTotalSize() }}
              >
                {renderedVirtualTurns.map((virtualTurn) => {
                  const turn = turns[virtualTurn.index];
                  if (turn === undefined) return null;
                  return (
                    <div
                      key={turn.id}
                      ref={turnVirtualizer.measureElement}
                      data-index={virtualTurn.index}
                      className="absolute left-0 top-0 w-full"
                      style={{
                        transform: `translateY(${virtualTurn.start}px)`,
                      }}
                    >
                      {renderTurn(turn, virtualTurn.index)}
                    </div>
                  );
                })}
              </div>
            ) : (
              turns.map((turn, index) => (
                <div key={turn.id}>{renderTurn(turn, index)}</div>
              ))
            )}
            {modelChangesAt(modelChanges, turns.length).map((change) => (
              <ModelChangeDivider
                key={change.id}
                modelName={change.modelName}
              />
            ))}
            {showRunning && <RunningIndicator />}
            <div className="h-8" />
          </div>
        </div>
        <ConversationNavigator
          {...conversationNavigation}
          turns={turns}
          activeAnchorId={navigation.activeAnchorId}
          isAtTail={navigation.isAtTail}
          onNavigate={navigateToAnchor}
          onNavigateToTail={navigation.navigateToTail}
        />
      </div>
    </ChatLinkContext.Provider>
  );
}

/** Selects the switches recorded after a given number of turns. */
function modelChangesAt(
  modelChanges: ChatModelChange[],
  turnCount: number,
): ChatModelChange[] {
  return modelChanges.filter((change) => change.afterTurnCount === turnCount);
}

/**
 * Marks where the answering model changed, so replies above and below a divider
 * are not mistaken for the work of one model.
 */
function ModelChangeDivider({ modelName }: { modelName: string }) {
  const { t } = useTranslation();
  return (
    <div
      role="separator"
      aria-label={t("chat.modelChange", { model: modelName })}
      className="flex items-center gap-3 py-4"
    >
      <span className="h-px flex-1 bg-border" />
      <span className="whitespace-nowrap text-xs text-muted-foreground">
        {t("chat.modelChange", { model: modelName })}
      </span>
      <span className="h-px flex-1 bg-border" />
    </div>
  );
}

/** Word rotation cadence — slow enough to read each phrase, quick enough to feel alive. */
const RUNNING_WORD_INTERVAL_MS = 5000;
/** Jitter applied to each rotation so the cadence doesn't feel metronomic (golden ratio, in ms). */
const RUNNING_WORD_JITTER_MS = 618;

/**
 * A playful "still working" line pinned to the foot of the live turn.
 *
 * Unlike the old typing dots, this stays for the whole response — through every
 * tool call and the quiet gaps between them — so the thread never looks frozen
 * while the agent is busy. The nine-dot grid carries the motion; the rotating
 * phrase reassures that time is passing rather than that anything has stalled.
 */
function RunningIndicator() {
  const { t } = useTranslation();
  const words = useMemo(
    () =>
      t("chat.runningWords")
        .split("|")
        .map((word) => word.trim())
        .filter(Boolean),
    [t],
  );
  const [index, setIndex] = useState(0);

  useEffect(() => {
    if (
      words.length <= 1 ||
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    )
      return;
    let timer: ReturnType<typeof setTimeout>;
    const scheduleNext = () => {
      const delay =
        RUNNING_WORD_INTERVAL_MS +
        (Math.random() * 2 - 1) * RUNNING_WORD_JITTER_MS;
      timer = setTimeout(() => {
        setIndex((current) => (current + 1) % words.length);
        scheduleNext();
      }, delay);
    };
    scheduleNext();
    return () => clearTimeout(timer);
  }, [words]);

  const word = words[index % words.length] ?? words[0] ?? "";
  return (
    <div
      className="flex items-center gap-3 py-4"
      role="status"
      aria-label={t("chat.typing")}
    >
      <span className="flex size-6 shrink-0 items-center justify-center text-muted-foreground">
        <AgentActivityDots
          label={t("common.running")}
          dotClassName="size-[3.5px]"
        />
      </span>
      {/* Keyed so each phrase crossfades in as the rotation advances. */}
      <span
        key={word}
        className="animate-in text-sm text-muted-foreground fade-in duration-500"
      >
        {word}
      </span>
    </div>
  );
}
