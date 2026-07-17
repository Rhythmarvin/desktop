/**
 * AgentProvider interface — defines the 8 method Agent Contract v1.
 * Plugin authors implement this to provide an Agent capability.
 */

import type {
  DiscoverInstallationsRequest,
  DiscoverInstallationsResponse,
  GetConfigurationSummaryRequest,
  GetConfigurationSummaryResponse,
  ListSkillsRequest,
  ListSkillsResponse,
  ListMcpServersRequest,
  ListMcpServersResponse,
  ListConversationsRequest,
  ListConversationsResponse,
  StartConversationRequest,
  SendMessageRequest,
  CancelConversationRequest,
  CancelConversationResponse,
  AgentEvent,
  AgentTurnResult,
} from "../types/index.js";

/** Per-invocation context: request ID and abort signal. */
export interface AgentCallContext {
  readonly requestId: string;
  readonly signal: AbortSignal;
}

/**
 * AgentProvider — the full Agent Contract v1 interface.
 * 6 Promise-returning methods + 2 AsyncGenerator methods.
 */
export interface AgentProvider {
  readonly id: string;
  readonly contractVersion: 1;

  // ── Idempotent ────────────────────────────────────────────

  discoverInstallations(
    call: AgentCallContext,
    request: DiscoverInstallationsRequest,
  ): Promise<DiscoverInstallationsResponse>;

  getConfigurationSummary(
    call: AgentCallContext,
    request: GetConfigurationSummaryRequest,
  ): Promise<GetConfigurationSummaryResponse>;

  listSkills(
    call: AgentCallContext,
    request: ListSkillsRequest,
  ): Promise<ListSkillsResponse>;

  listMcpServers(
    call: AgentCallContext,
    request: ListMcpServersRequest,
  ): Promise<ListMcpServersResponse>;

  listConversations(
    call: AgentCallContext,
    request: ListConversationsRequest,
  ): Promise<ListConversationsResponse>;

  cancelConversation(
    call: AgentCallContext,
    request: CancelConversationRequest,
  ): Promise<CancelConversationResponse>;

  // ── Non-idempotent, streaming ──────────────────────────────

  startConversation(
    call: AgentCallContext,
    request: StartConversationRequest,
  ): AsyncGenerator<AgentEvent, AgentTurnResult, void>;

  sendMessage(
    call: AgentCallContext,
    request: SendMessageRequest,
  ): AsyncGenerator<AgentEvent, AgentTurnResult, void>;
}
