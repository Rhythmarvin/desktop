use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::identity::{AgentProviderId, PluginId};

// ── $/initialize ──────────────────────────────────────────────────

/// Parameters sent by Host in `$/initialize` request (17 fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializeParams {
    /// Frame-layer protocol version. Host compile-time locked. NOT the same as pluginApi.
    pub wire_version: u32,
    /// Ora application version.
    pub host_version: String,
    /// Private runtime/bootstrap version.
    pub runtime_version: String,
    /// CSPRNG per-generation session identifier, never reused.
    pub session_id: String,
    /// Verified plugin identity from manifest + receipt.
    pub plugin: InitializePluginIdentity,
    /// Managed paths computed by Host from installation proof.
    pub paths: InitializePaths,
    /// Agents declared in manifest's `contributes.agents`.
    pub declared_agents: Vec<DeclaredAgent>,
    /// Dynamic limits for this generation (7 values, cannot exceed v1 hard caps).
    pub limits: InitializeLimits,
}

/// Plugin identity block in `$/initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializePluginIdentity {
    pub id: PluginId,
    pub version: String,
    pub kind: String,
    pub plugin_api: u32,
    pub content_owner: String,
}

/// Managed paths block in `$/initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializePaths {
    /// Managed code root directory (plugin cwd).
    pub extension_path: String,
    /// Verified absolute entry path for `import()`. Must be under extensionPath.
    pub entry_path: String,
    /// Content-owner scoped mutable data directory.
    pub storage_path: String,
}

/// A declared agent entry from the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct DeclaredAgent {
    pub id: AgentProviderId,
    pub contract_version: u32,
}

/// Dynamic limits block in `$/initialize` (7 values).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializeLimits {
    /// Must exactly equal 8388608 (wire constant, non-negotiable).
    pub max_frame_bytes: u32,
    /// Max concurrent ordinary pending requests.
    pub max_pending_requests: u32,
    /// Max single stream event payload bytes.
    pub max_agent_event_bytes: u32,
    /// Max single terminal result/error payload bytes.
    pub max_agent_result_bytes: u32,
    /// Max agent prompt bytes.
    pub max_agent_prompt_bytes: u32,
    /// Max concurrent active turns per plugin.
    pub max_active_turns: u32,
    /// Max page items for paginated responses.
    pub max_page_items: u32,
}

/// Bootstrap response to `$/initialize` — echoes identity for cross-check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializeResult {
    pub wire_version: u32,
    pub runtime_version: String,
    pub session_id: String,
    pub plugin: InitializePluginEcho,
}

/// Echoed plugin identity in `$/initialize` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializePluginEcho {
    pub id: PluginId,
    pub version: String,
}

// ── $/activate ────────────────────────────────────────────────────

/// Parameters sent by Host in `$/activate` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ActivateParams {
    pub reason: ActivateReason,
}

/// Reason for plugin activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum ActivateReason {
    /// First Agent method invocation triggers lazy start.
    #[serde(rename = "lazyInvocation")]
    LazyInvocation,
    /// User explicitly started the plugin.
    #[serde(rename = "manualStart")]
    ManualStart,
}

/// Bootstrap response to `$/activate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ActivateResult {
    /// MUST deeply equal manifest's `contributes.agents` (sorted by canonical id).
    pub providers: Vec<ActivateProvider>,
}

/// Provider descriptor in activate result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ActivateProvider {
    pub id: AgentProviderId,
    pub contract_version: u32,
}

// ── $/deactivate ──────────────────────────────────────────────────

/// Parameters sent by Host in `$/deactivate` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct DeactivateParams {
    pub reason: DeactivateReason,
}

/// Reason for plugin deactivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum DeactivateReason {
    #[serde(rename = "manualStop")]
    ManualStop,
    #[serde(rename = "disable")]
    Disable,
    #[serde(rename = "uninstall")]
    Uninstall,
    #[serde(rename = "shutdown")]
    Shutdown,
    #[serde(rename = "grantChanged")]
    GrantChanged,
}

// ── Control notifications ─────────────────────────────────────────

/// `$/cancelRequest` notification params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct CancelRequestParams {
    /// The request id to cancel.
    pub id: String,
}

/// `$/stream` notification params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct StreamParams {
    /// The request id this stream event belongs to.
    pub id: String,
    /// Strictly increasing sequence number starting at 1.
    #[ts(type = "number")]
    pub seq: u64,
    /// Typed stream-event discriminated union value.
    #[ts(type = "any")]
    pub value: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ── $/initialize params ───────────────────────────────────────

    #[test]
    fn initialize_params_full_serde() {
        let json = serde_json::json!({
            "wireVersion": 1,
            "hostVersion": "0.1.0",
            "runtimeVersion": "0.1.0",
            "sessionId": "session-abc-123",
            "plugin": {
                "id": "ora.claude-code",
                "version": "0.1.0",
                "kind": "agent",
                "pluginApi": 1,
                "contentOwner": "sha256-abc123"
            },
            "paths": {
                "extensionPath": "D:\\plugins\\ora.claude-code",
                "entryPath": "D:\\plugins\\ora.claude-code\\dist\\index.js",
                "storagePath": "D:\\plugin-data\\ora.claude-code\\sha256-abc123"
            },
            "declaredAgents": [
                {"id": "claude-code", "contractVersion": 1}
            ],
            "limits": {
                "maxFrameBytes": 8388608,
                "maxPendingRequests": 128,
                "maxAgentEventBytes": 262144,
                "maxAgentResultBytes": 1048576,
                "maxAgentPromptBytes": 1048576,
                "maxActiveTurns": 64,
                "maxPageItems": 100
            }
        });

        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.wire_version, 1);
        assert_eq!(params.plugin.id.as_str(), "ora.claude-code");
        assert_eq!(params.plugin.plugin_api, 1);
        assert_eq!(params.declared_agents.len(), 1);
        assert_eq!(params.declared_agents[0].id.as_str(), "claude-code");
        assert_eq!(params.limits.max_frame_bytes, 8_388_608);
        assert_eq!(params.limits.max_pending_requests, 128);
    }

    // ── $/initialize result ───────────────────────────────────────

    #[test]
    fn initialize_result_serde() {
        let result = InitializeResult {
            wire_version: 1,
            runtime_version: "0.1.0".to_string(),
            session_id: "session-abc-123".to_string(),
            plugin: InitializePluginEcho {
                id: PluginId::new("ora.claude-code").unwrap(),
                version: "0.1.0".to_string(),
            },
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["wireVersion"], 1);
        assert_eq!(json["sessionId"], "session-abc-123");

        let decoded: InitializeResult = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.plugin.id.as_str(), "ora.claude-code");
    }

    // ── $/activate ────────────────────────────────────────────────

    #[test]
    fn activate_params_serde() {
        let json = serde_json::json!({"reason": "lazyInvocation"});
        let params: ActivateParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.reason, ActivateReason::LazyInvocation));

        let json = serde_json::json!({"reason": "manualStart"});
        let params: ActivateParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.reason, ActivateReason::ManualStart));
    }

    #[test]
    fn activate_result_serde() {
        let result = ActivateResult {
            providers: vec![ActivateProvider {
                id: AgentProviderId::new("claude-code").unwrap(),
                contract_version: 1,
            }],
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["providers"][0]["id"], "claude-code");

        let decoded: ActivateResult = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.providers[0].id.as_str(), "claude-code");
    }

    // ── $/deactivate ──────────────────────────────────────────────

    #[test]
    fn deactivate_params_all_reasons() {
        for (wire, expected) in [
            ("manualStop", DeactivateReason::ManualStop),
            ("disable", DeactivateReason::Disable),
            ("uninstall", DeactivateReason::Uninstall),
            ("shutdown", DeactivateReason::Shutdown),
            ("grantChanged", DeactivateReason::GrantChanged),
        ] {
            let json = serde_json::json!({"reason": wire});
            let params: DeactivateParams = serde_json::from_value(json).unwrap();
            assert_eq!(params.reason, expected, "failed for {wire}");
        }
    }

    // ── Control notifications ─────────────────────────────────────

    #[test]
    fn cancel_request_params_serde() {
        let params = CancelRequestParams {
            id: "h:5".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["id"], "h:5");
    }

    #[test]
    fn stream_params_serde() {
        let params = StreamParams {
            id: "h:1".to_string(),
            seq: 1,
            value: serde_json::json!({"kind": "textDelta", "text": "hello"}),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["seq"], 1);
        assert_eq!(json["value"]["kind"], "textDelta");
    }

    // ── DeclaredAgent ─────────────────────────────────────────────

    #[test]
    fn declared_agent_serde() {
        let json = serde_json::json!({"id": "claude-code", "contractVersion": 1});
        let agent: DeclaredAgent = serde_json::from_value(json).unwrap();
        assert_eq!(agent.id.as_str(), "claude-code");
        assert_eq!(agent.contract_version, 1);
    }
}
