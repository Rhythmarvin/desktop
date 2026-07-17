//! Agent Contract v1 DTOs — request, result, event, and error types.
//!
//! All newtypes encode transparently as JSON primitives (no `{ "value": ... }` wrapper).
//! All objects recursively reject unknown fields (except `AgentBusinessError.details`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::identity::AgentProviderId;

// ── Leaf newtypes (transparent JSON encoding) ────────────────────

/// Opaque installation ID (1..=256 UTF-8 bytes, no NUL/C0/C1 control, `/`, `\`, `:`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentInstallationId(pub String);

/// Opaque conversation ID (same rules as installation ID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentConversationId(pub String);

/// Opaque turn ID (same rules as installation ID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentTurnId(pub String);

/// Opaque cursor token (same rules as installation ID).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentCursor(pub String);

/// Opaque resource ID (same rules as installation ID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentResourceId(pub String);

/// Opaque tool call ID (same rules as installation ID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentToolCallId(pub String);

/// Configuration key (1..=512 ASCII bytes).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentConfigurationKey(pub String);

/// Session-bound project handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ProjectHandle(pub String);

/// Session-bound worktree handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct WorktreeHandle(pub String);

/// Client request ID (canonical lowercase UUID 8-4-4-4-12 hex format).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ClientRequestId(pub String);

/// Host-resolved absolute Windows path (1..=32 KiB UTF-8, no NUL).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct HostResolvedAbsolutePath(pub String);

/// Agent prompt (1..=1 MiB UTF-8, no NUL, preserves whitespace/newlines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentPrompt(pub String);

// ── AgentScope ────────────────────────────────────────────────────

/// Scoping context for Agent method invocations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentScope {
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "project", rename_all = "camelCase")]
    Project {
        project_handle: ProjectHandle,
        working_directory: HostResolvedAbsolutePath,
    },
    #[serde(rename = "worktree", rename_all = "camelCase")]
    Worktree {
        project_handle: ProjectHandle,
        worktree_handle: WorktreeHandle,
        working_directory: HostResolvedAbsolutePath,
    },
}

// ── Request DTOs ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct DiscoverInstallationsRequest {
    pub provider_id: AgentProviderId,
    pub scope: AgentScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct GetConfigurationSummaryRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub scope: AgentScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ListSkillsRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub scope: AgentScope,
    pub cursor: Option<AgentCursor>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ListMcpServersRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub scope: AgentScope,
    pub cursor: Option<AgentCursor>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ListConversationsRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub scope: AgentScope,
    pub cursor: Option<AgentCursor>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct StartConversationRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub scope: AgentScope,
    pub client_request_id: ClientRequestId,
    pub prompt: AgentPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct SendMessageRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub conversation_id: AgentConversationId,
    pub scope: AgentScope,
    pub client_request_id: ClientRequestId,
    pub prompt: AgentPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct CancelConversationRequest {
    pub provider_id: AgentProviderId,
    pub installation_id: AgentInstallationId,
    pub conversation_id: AgentConversationId,
    pub scope: AgentScope,
}

// ── Response DTOs ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct DiscoverInstallationsResponse {
    pub installations: Vec<AgentInstallation>,
    pub diagnostics: Vec<AgentDiscoveryDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentInstallation {
    pub installation_id: AgentInstallationId,
    pub display_name: String,
    pub version: Option<String>,
    pub location_display: Option<String>,
    pub availability: AgentAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentAvailability {
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "unavailable", rename_all = "camelCase")]
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentDiscoveryDiagnostic {
    pub kind: AgentDiscoveryDiagnosticKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentDiscoveryDiagnosticKind {
    #[serde(rename = "notFound")]
    NotFound,
    #[serde(rename = "permissionDenied")]
    PermissionDenied,
    #[serde(rename = "probeFailed")]
    ProbeFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct GetConfigurationSummaryResponse {
    pub items: Vec<AgentConfigurationItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentConfigurationItem {
    pub key: AgentConfigurationKey,
    pub display_name: String,
    pub source: AgentResourceSource,
    pub value: AgentConfigurationValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentConfigurationValue {
    #[serde(rename = "unset")]
    Unset,
    #[serde(rename = "redacted")]
    Redacted,
    #[serde(rename = "boolean")]
    Boolean { value: bool },
    #[serde(rename = "number")]
    Number { value: f64 },
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "stringList")]
    StringList { value: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentResourceSource {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "worktree")]
    Worktree,
    #[serde(rename = "builtIn")]
    BuiltIn,
    #[serde(rename = "unknown")]
    Unknown { display: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ListSkillsResponse {
    pub items: Vec<AgentSkillSummary>,
    pub next_cursor: Option<AgentCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentSkillSummary {
    pub id: AgentResourceId,
    pub display_name: String,
    pub description: Option<String>,
    pub source: AgentResourceSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ListMcpServersResponse {
    pub items: Vec<AgentMcpServerSummary>,
    pub next_cursor: Option<AgentCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentMcpServerSummary {
    pub id: AgentResourceId,
    pub display_name: String,
    pub transport: AgentMcpTransport,
    pub enabled: bool,
    pub source: AgentResourceSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentMcpTransport {
    #[serde(rename = "stdio")]
    Stdio,
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "sse")]
    Sse,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ListConversationsResponse {
    pub items: Vec<AgentConversationSummary>,
    pub next_cursor: Option<AgentCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentConversationSummary {
    pub conversation_id: AgentConversationId,
    pub title: Option<String>,
    pub updated_at: Option<String>,
}

// ── Stream Events ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentEvent {
    #[serde(rename = "conversationStarted", rename_all = "camelCase")]
    ConversationStarted {
        conversation_id: AgentConversationId,
    },
    #[serde(rename = "textDelta", rename_all = "camelCase")]
    TextDelta {
        channel: AgentOutputChannel,
        text: String,
    },
    #[serde(rename = "status", rename_all = "camelCase")]
    Status {
        phase: String,
        message: Option<String>,
    },
    #[serde(rename = "toolCall", rename_all = "camelCase")]
    ToolCall {
        call_id: AgentToolCallId,
        name: String,
        summary: Option<String>,
    },
    #[serde(rename = "toolResult", rename_all = "camelCase")]
    ToolResult {
        call_id: AgentToolCallId,
        is_error: bool,
        summary: Option<String>,
    },
    #[serde(rename = "usage", rename_all = "camelCase")]
    Usage { usage: AgentUsage },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentOutputChannel {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "reasoning")]
    Reasoning,
    #[serde(rename = "tool")]
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_micros: Option<u64>,
}

// ── Terminal Result ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentTurnResult {
    pub conversation_id: AgentConversationId,
    pub turn_id: Option<AgentTurnId>,
    pub finish_reason: AgentFinishReason,
    pub usage: Option<AgentUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentFinishReason {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "limit")]
    Limit,
}

// ── Cancel Result ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct CancelConversationResponse {
    pub disposition: CancelDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum CancelDisposition {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "alreadyStopped")]
    AlreadyStopped,
}

// ── Business Error Kinds ──────────────────────────────────────────

/// Agent business failure kind — closed enum for v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum AgentBusinessFailureKind {
    #[serde(rename = "agentUnavailable")]
    AgentUnavailable,
    #[serde(rename = "authenticationRequired")]
    AuthenticationRequired,
    #[serde(rename = "invalidAgentConfiguration")]
    InvalidAgentConfiguration,
    #[serde(rename = "installationNotFound")]
    InstallationNotFound,
    #[serde(rename = "conversationNotFound")]
    ConversationNotFound,
    #[serde(rename = "unsupportedAgentCapability")]
    UnsupportedAgentCapability,
    #[serde(rename = "invalidState")]
    InvalidState,
    #[serde(rename = "permissionDenied")]
    PermissionDenied,
    #[serde(rename = "cursorExpired")]
    CursorExpired,
    #[serde(rename = "agentProcessFailed")]
    AgentProcessFailed,
    /// Reserved — only bootstrap can create this kind.
    #[serde(rename = "providerFailure")]
    ProviderFailure,
}

// ── Method Registry ───────────────────────────────────────────────

/// Metadata about each Agent Contract method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMethodInfo {
    pub method: &'static str,
    pub idempotent: bool,
    pub streaming: bool,
    pub safety: bool,
}

/// Registry of all v1 Agent Contract methods.
pub static AGENT_METHODS: &[AgentMethodInfo] = &[
    AgentMethodInfo {
        method: "agent.discoverInstallations",
        idempotent: true,
        streaming: false,
        safety: false,
    },
    AgentMethodInfo {
        method: "agent.getConfigurationSummary",
        idempotent: true,
        streaming: false,
        safety: false,
    },
    AgentMethodInfo {
        method: "agent.listSkills",
        idempotent: true,
        streaming: false,
        safety: false,
    },
    AgentMethodInfo {
        method: "agent.listMcpServers",
        idempotent: true,
        streaming: false,
        safety: false,
    },
    AgentMethodInfo {
        method: "agent.listConversations",
        idempotent: true,
        streaming: false,
        safety: false,
    },
    AgentMethodInfo {
        method: "agent.cancelConversation",
        idempotent: true,
        streaming: false,
        safety: true,
    },
    AgentMethodInfo {
        method: "agent.startConversation",
        idempotent: false,
        streaming: true,
        safety: false,
    },
    AgentMethodInfo {
        method: "agent.sendMessage",
        idempotent: false,
        streaming: true,
        safety: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ── AgentScope round-trip ─────────────────────────────────────

    #[test]
    fn scope_global() {
        let json = serde_json::json!({"type": "global"});
        let scope: AgentScope = serde_json::from_value(json).unwrap();
        assert!(matches!(scope, AgentScope::Global));
    }

    #[test]
    fn scope_project() {
        let json = serde_json::json!({
            "type": "project",
            "projectHandle": "proj-1",
            "workingDirectory": "D:\\work"
        });
        let scope: AgentScope = serde_json::from_value(json.clone()).unwrap();
        assert!(matches!(scope, AgentScope::Project { .. }));
        let back = serde_json::to_value(&scope).unwrap();
        assert_eq!(back["type"], "project");
        assert_eq!(back["projectHandle"], "proj-1");
    }

    // ── AgentEvent round-trip ─────────────────────────────────────

    #[test]
    fn event_conversation_started() {
        let json = serde_json::json!({
            "kind": "conversationStarted",
            "conversationId": "conv-123"
        });
        let event: AgentEvent = serde_json::from_value(json).unwrap();
        match event {
            AgentEvent::ConversationStarted { conversation_id } => {
                assert_eq!(conversation_id.0, "conv-123");
            }
            _ => panic!("expected ConversationStarted"),
        }
    }

    #[test]
    fn event_text_delta() {
        let json = serde_json::json!({
            "kind": "textDelta",
            "channel": "assistant",
            "text": "Hello!"
        });
        let event: AgentEvent = serde_json::from_value(json).unwrap();
        match event {
            AgentEvent::TextDelta { channel, text } => {
                assert_eq!(channel, AgentOutputChannel::Assistant);
                assert_eq!(text, "Hello!");
            }
            _ => panic!("expected TextDelta"),
        }
    }

    #[test]
    fn event_status() {
        let json = serde_json::json!({
            "kind": "status",
            "phase": "thinking",
            "message": "Analyzing..."
        });
        let event: AgentEvent = serde_json::from_value(json).unwrap();
        match event {
            AgentEvent::Status { phase, message } => {
                assert_eq!(phase, "thinking");
                assert_eq!(message.unwrap(), "Analyzing...");
            }
            _ => panic!("expected Status"),
        }
    }

    #[test]
    fn event_usage() {
        let json = serde_json::json!({
            "kind": "usage",
            "usage": {
                "inputTokens": 1000,
                "outputTokens": 500
            }
        });
        let event: AgentEvent = serde_json::from_value(json).unwrap();
        match event {
            AgentEvent::Usage { usage } => {
                assert_eq!(usage.input_tokens, Some(1000));
                assert_eq!(usage.output_tokens, Some(500));
                assert_eq!(usage.cost_micros, None);
            }
            _ => panic!("expected Usage"),
        }
    }

    // ── AgentTurnResult ───────────────────────────────────────────

    #[test]
    fn turn_result_completed() {
        let json = serde_json::json!({
            "conversationId": "conv-123",
            "finishReason": "completed"
        });
        let result: AgentTurnResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.conversation_id.0, "conv-123");
        assert_eq!(result.finish_reason, AgentFinishReason::Completed);
        assert!(result.turn_id.is_none());
    }

    // ── CancelConversationResponse ─────────────────────────────────

    #[test]
    fn cancel_accepted() {
        let json = serde_json::json!({"disposition": "accepted"});
        let resp: CancelConversationResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.disposition, CancelDisposition::Accepted);
    }

    #[test]
    fn cancel_already_stopped() {
        let json = serde_json::json!({"disposition": "alreadyStopped"});
        let resp: CancelConversationResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.disposition, CancelDisposition::AlreadyStopped);
    }

    // ── DiscoverInstallations ─────────────────────────────────────

    #[test]
    fn discover_installations_request() {
        let json = serde_json::json!({
            "providerId": "claude-code",
            "scope": {"type": "global"}
        });
        let req: DiscoverInstallationsRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.provider_id.as_str(), "claude-code");
    }

    #[test]
    fn discover_installations_response() {
        let json = serde_json::json!({
            "installations": [{
                "installationId": "inst-1",
                "displayName": "Claude Code",
                "availability": {"type": "available"}
            }],
            "diagnostics": []
        });
        let resp: DiscoverInstallationsResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.installations.len(), 1);
        assert_eq!(resp.installations[0].display_name, "Claude Code");
    }

    // ── Method Registry ───────────────────────────────────────────

    #[test]
    fn method_registry_has_eight_methods() {
        assert_eq!(AGENT_METHODS.len(), 8);
    }

    #[test]
    fn method_registry_no_duplicates() {
        let mut methods: Vec<&str> = AGENT_METHODS.iter().map(|m| m.method).collect();
        methods.sort();
        let original_len = methods.len();
        methods.dedup();
        assert_eq!(methods.len(), original_len, "duplicate method names found");
    }

    #[test]
    fn method_registry_correct_flags() {
        let discover = AGENT_METHODS
            .iter()
            .find(|m| m.method == "agent.discoverInstallations")
            .unwrap();
        assert!(discover.idempotent);
        assert!(!discover.streaming);
        assert!(!discover.safety);

        let send = AGENT_METHODS
            .iter()
            .find(|m| m.method == "agent.sendMessage")
            .unwrap();
        assert!(!send.idempotent);
        assert!(send.streaming);
        assert!(!send.safety);

        let cancel = AGENT_METHODS
            .iter()
            .find(|m| m.method == "agent.cancelConversation")
            .unwrap();
        assert!(cancel.idempotent);
        assert!(!cancel.streaming);
        assert!(cancel.safety);
    }
}
