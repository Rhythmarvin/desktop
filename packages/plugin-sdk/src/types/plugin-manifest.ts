// Agent Contract v1 DTOs — hand-written until ts-rs CI generation is wired.
// Matches ora-plugin-protocol Rust types.

export type PluginId = string;
export type AgentProviderId = string;

export interface AgentProviderKey {
  pluginId: PluginId;
  providerId: AgentProviderId;
}

export type AgentScope =
  | { type: "global" }
  | { type: "project"; projectHandle: string; workingDirectory: string }
  | { type: "worktree"; projectHandle: string; worktreeHandle: string; workingDirectory: string };

export type AgentInstallationId = string;
export type AgentConversationId = string;
export type AgentTurnId = string;
export type AgentCursor = string;
export type AgentResourceId = string;
export type AgentToolCallId = string;
export type AgentConfigurationKey = string;
export type ClientRequestId = string;
export type AgentPrompt = string;

export interface DiscoverInstallationsRequest {
  providerId: AgentProviderId;
  scope: AgentScope;
}
export interface GetConfigurationSummaryRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  scope: AgentScope;
}
export interface ListSkillsRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  scope: AgentScope;
  cursor?: AgentCursor;
  limit: number;
}
export interface ListMcpServersRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  scope: AgentScope;
  cursor?: AgentCursor;
  limit: number;
}
export interface ListConversationsRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  scope: AgentScope;
  cursor?: AgentCursor;
  limit: number;
}
export interface StartConversationRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  scope: AgentScope;
  clientRequestId: ClientRequestId;
  prompt: AgentPrompt;
}
export interface SendMessageRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  conversationId: AgentConversationId;
  scope: AgentScope;
  clientRequestId: ClientRequestId;
  prompt: AgentPrompt;
}
export interface CancelConversationRequest {
  providerId: AgentProviderId;
  installationId: AgentInstallationId;
  conversationId: AgentConversationId;
  scope: AgentScope;
}

export interface DiscoverInstallationsResponse {
  installations: AgentInstallation[];
  diagnostics: AgentDiscoveryDiagnostic[];
}
export interface AgentInstallation {
  installationId: AgentInstallationId;
  displayName: string;
  version?: string;
  locationDisplay?: string;
  availability: AgentAvailability;
}
export type AgentAvailability =
  | { type: "available" }
  | { type: "unavailable"; reason: string };
export interface AgentDiscoveryDiagnostic {
  kind: "notFound" | "permissionDenied" | "probeFailed";
  message: string;
}
export interface GetConfigurationSummaryResponse {
  items: AgentConfigurationItem[];
}
export interface AgentConfigurationItem {
  key: AgentConfigurationKey;
  displayName: string;
  source: AgentResourceSource;
  value: AgentConfigurationValue;
}
export type AgentResourceSource =
  | { type: "user" }
  | { type: "project" }
  | { type: "worktree" }
  | { type: "builtIn" }
  | { type: "unknown"; display?: string };
export type AgentConfigurationValue =
  | { type: "unset" }
  | { type: "redacted" }
  | { type: "boolean"; value: boolean }
  | { type: "number"; value: number }
  | { type: "string"; value: string }
  | { type: "stringList"; value: string[] };
export interface ListSkillsResponse {
  items: AgentSkillSummary[];
  nextCursor?: AgentCursor;
}
export interface AgentSkillSummary {
  id: AgentResourceId;
  displayName: string;
  description?: string;
  source: AgentResourceSource;
}
export interface ListMcpServersResponse {
  items: AgentMcpServerSummary[];
  nextCursor?: AgentCursor;
}
export interface AgentMcpServerSummary {
  id: AgentResourceId;
  displayName: string;
  transport: "stdio" | "http" | "sse" | "unknown";
  enabled: boolean;
  source: AgentResourceSource;
}
export interface ListConversationsResponse {
  items: AgentConversationSummary[];
  nextCursor?: AgentCursor;
}
export interface AgentConversationSummary {
  conversationId: AgentConversationId;
  title?: string;
  updatedAt?: string;
}
export interface CancelConversationResponse {
  disposition: "accepted" | "alreadyStopped";
}

export type AgentEvent =
  | { kind: "conversationStarted"; conversationId: AgentConversationId }
  | { kind: "textDelta"; channel: "assistant" | "reasoning" | "tool"; text: string }
  | { kind: "status"; phase: string; message?: string }
  | { kind: "toolCall"; callId: AgentToolCallId; name: string; summary?: string }
  | { kind: "toolResult"; callId: AgentToolCallId; isError: boolean; summary?: string }
  | { kind: "usage"; usage: AgentUsage };

export interface AgentUsage {
  inputTokens?: number;
  outputTokens?: number;
  costMicros?: number;
}

export interface AgentTurnResult {
  conversationId: AgentConversationId;
  turnId?: AgentTurnId;
  finishReason: "completed" | "cancelled" | "limit";
  usage?: AgentUsage;
}

export interface PluginManifest {
  manifestVersion: number;
  id: PluginId;
  displayName: string;
  kind: "agent" | "workbench";
  main?: string;
  engines: PluginEngines;
  contributes?: ManifestContributes;
}
export interface PluginEngines {
  ora: string;
  pluginApi?: number;
  bun?: string;
}
export interface ManifestContributes {
  agents?: AgentContribution[];
  workbench?: { schemaVersion: number };
}
export interface AgentContribution {
  id: AgentProviderId;
  displayName: string;
  contractVersion: number;
}
