use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::{AgentEvent, HostResolvedAbsolutePath, JsonSafeU64};
use crate::identity::{AgentProviderId, ContentOwnerId, PluginId, PluginVersion};
use crate::manifest::PluginKind;

// ── A-side compat aliases ──────────────────────────────────────
pub type InitializePlugin = InitializePluginIdentity;
pub type InitializeResultPlugin = InitializePluginEcho;

// ── $/initialize ──────────────────────────────────────────────────

/// Parameters sent by Host in `$/initialize` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializeParams {
    pub wire_version: u32,
    pub host_version: PluginVersion,
    pub runtime_version: PluginVersion,
    pub session_id: String,
    pub plugin: InitializePluginIdentity,
    pub paths: InitializePaths,
    pub declared_agents: Vec<DeclaredAgent>,
    pub limits: InitializeLimits,
}

impl InitializeParams {
    pub fn validate(&self) -> Result<(), String> {
        if self.wire_version != 1 {
            return Err("unsupported wire version".into());
        }
        if self.plugin.plugin_api != 1 {
            return Err("unsupported plugin API version".into());
        }
        Ok(())
    }
}

/// Plugin identity block in `$/initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializePluginIdentity {
    pub id: PluginId,
    pub version: PluginVersion,
    pub kind: PluginKind,
    pub plugin_api: u32,
    pub content_owner: ContentOwnerId,
}

/// Managed paths block in `$/initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializePaths {
    pub extension_path: HostResolvedAbsolutePath,
    pub entry_path: HostResolvedAbsolutePath,
    pub storage_path: HostResolvedAbsolutePath,
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

impl InitializeLimits {
    pub fn v1_defaults() -> Self {
        Self {
            max_frame_bytes: 8_388_608,
            max_pending_requests: 128,
            max_agent_event_bytes: 262_144,
            max_agent_result_bytes: 1_048_576,
            max_agent_prompt_bytes: 1_048_576,
            max_active_turns: 64,
            max_page_items: 100,
        }
    }

    pub fn new(
        max_frame_bytes: u32,
        max_pending_requests: u32,
        max_agent_event_bytes: u32,
        max_agent_result_bytes: u32,
        max_agent_prompt_bytes: u32,
        max_active_turns: u32,
        max_page_items: u32,
    ) -> Self {
        Self {
            max_frame_bytes,
            max_pending_requests,
            max_agent_event_bytes,
            max_agent_result_bytes,
            max_agent_prompt_bytes,
            max_active_turns,
            max_page_items,
        }
    }
}

/// Bootstrap response to `$/initialize` — echoes identity for cross-check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializeResult {
    pub wire_version: u32,
    pub runtime_version: PluginVersion,
    pub session_id: String,
    pub plugin: InitializePluginEcho,
}

/// Echoed plugin identity in `$/initialize` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct InitializePluginEcho {
    pub id: PluginId,
    pub version: PluginVersion,
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

impl ActivateResult {
    pub fn validate_declared_providers(
        &self,
        declared_agents: &[DeclaredAgent],
    ) -> Result<(), String> {
        if self.providers.len() != declared_agents.len() {
            return Err("provider count mismatch".into());
        }
        for (p, d) in self.providers.iter().zip(declared_agents.iter()) {
            if p.id != d.id {
                return Err(format!(
                    "provider id mismatch: {} vs {}",
                    p.id.as_str(),
                    d.id.as_str()
                ));
            }
            if p.contract_version != d.contract_version {
                return Err("contract version mismatch".into());
            }
        }
        Ok(())
    }
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
    pub id: String,
    #[ts(type = "number")]
    pub seq: JsonSafeU64,
    #[ts(type = "any")]
    pub value: AgentEvent,
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
                "contentOwner": "sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
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
            runtime_version: PluginVersion::parse("0.1.0").unwrap(),
            session_id: "session-abc-123".to_string(),
            plugin: InitializePluginEcho {
                id: PluginId::parse("ora.claude-code").unwrap(),
                version: PluginVersion::parse("0.1.0").unwrap(),
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
                id: AgentProviderId::parse("claude-code").unwrap(),
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
        let value_json =
            serde_json::json!({"kind": "textDelta", "channel": "assistant", "text": "hello"});
        let value: AgentEvent = serde_json::from_value(value_json).unwrap();
        let params = StreamParams {
            id: "h:1".to_string(),
            seq: JsonSafeU64::new(1).unwrap(),
            value,
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

    // ── InitializeParams::validate ─────────────────────────────────

    fn make_valid_params() -> InitializeParams {
        InitializeParams {
            wire_version: 1,
            host_version: PluginVersion::parse("0.1.0").unwrap(),
            runtime_version: PluginVersion::parse("0.1.0").unwrap(),
            session_id: "s".into(),
            plugin: InitializePluginIdentity {
                id: PluginId::parse("ora.test").unwrap(),
                version: PluginVersion::parse("0.1.0").unwrap(),
                kind: PluginKind::Agent,
                plugin_api: 1,
                content_owner: ContentOwnerId::parse(format!(
                    "sha256-{}",
                    "a".repeat(64)
                ))
                .unwrap(),
            },
            paths: InitializePaths {
                extension_path: HostResolvedAbsolutePath::parse("D:\\ext").unwrap(),
                entry_path: HostResolvedAbsolutePath::parse("D:\\ext\\index.js").unwrap(),
                storage_path: HostResolvedAbsolutePath::parse("D:\\data").unwrap(),
            },
            declared_agents: vec![],
            limits: InitializeLimits::v1_defaults(),
        }
    }

    #[test]
    fn initialize_params_validate_wire_version_rejected() {
        let mut params = make_valid_params();
        params.wire_version = 2;
        assert!(params.validate().is_err());
    }

    #[test]
    fn initialize_params_validate_plugin_api_rejected() {
        let mut params = make_valid_params();
        params.plugin.plugin_api = 2;
        assert!(params.validate().is_err());
    }

    #[test]
    fn initialize_params_validate_v1_passes() {
        let params = make_valid_params();
        assert!(params.validate().is_ok());
    }

    // ── ActivateResult::validate_declared_providers ────────────────

    #[test]
    fn activate_result_validate_providers_match() {
        let declared = vec![DeclaredAgent {
            id: AgentProviderId::parse("claude-code").unwrap(),
            contract_version: 1,
        }];
        let result = ActivateResult {
            providers: vec![ActivateProvider {
                id: AgentProviderId::parse("claude-code").unwrap(),
                contract_version: 1,
            }],
        };
        assert!(result.validate_declared_providers(&declared).is_ok());
    }

    #[test]
    fn activate_result_validate_count_mismatch() {
        let declared = vec![
            DeclaredAgent {
                id: AgentProviderId::parse("a").unwrap(),
                contract_version: 1,
            },
            DeclaredAgent {
                id: AgentProviderId::parse("b").unwrap(),
                contract_version: 1,
            },
        ];
        let result = ActivateResult {
            providers: vec![ActivateProvider {
                id: AgentProviderId::parse("a").unwrap(),
                contract_version: 1,
            }],
        };
        assert!(result.validate_declared_providers(&declared).is_err());
    }

    #[test]
    fn activate_result_validate_id_mismatch() {
        let declared = vec![DeclaredAgent {
            id: AgentProviderId::parse("expected").unwrap(),
            contract_version: 1,
        }];
        let result = ActivateResult {
            providers: vec![ActivateProvider {
                id: AgentProviderId::parse("actual").unwrap(),
                contract_version: 1,
            }],
        };
        assert!(result.validate_declared_providers(&declared).is_err());
    }

    #[test]
    fn activate_result_validate_contract_version_mismatch() {
        let declared = vec![DeclaredAgent {
            id: AgentProviderId::parse("p").unwrap(),
            contract_version: 1,
        }];
        let result = ActivateResult {
            providers: vec![ActivateProvider {
                id: AgentProviderId::parse("p").unwrap(),
                contract_version: 2,
            }],
        };
        assert!(result.validate_declared_providers(&declared).is_err());
    }

    #[test]
    fn activate_result_validate_empty_both() {
        assert!(ActivateResult {
            providers: vec![]
        }
        .validate_declared_providers(&[])
        .is_ok());
    }
}
