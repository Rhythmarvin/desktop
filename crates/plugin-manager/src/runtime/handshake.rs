//! Handshake helpers — build `$/initialize` and `$/activate` messages and validate responses.

use crate::transport::frame::FrameType;

/// Result of the `$/initialize` handshake.
#[derive(Debug, Clone)]
pub struct InitializeOutcome {
    pub wire_version: u32,
    pub runtime_version: String,
    pub session_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
}

/// Result of the `$/activate` handshake.
#[derive(Debug, Clone)]
pub struct ActivateOutcome {
    pub providers: Vec<ProviderDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: String,
    pub contract_version: u32,
}

/// Build the JSON-RPC request payload for `$/initialize`.
pub fn build_initialize_request(
    id: &str,
    wire_version: u32,
    host_version: &str,
    runtime_version: &str,
    session_id: &str,
    plugin_id: &str,
    plugin_version: &str,
    plugin_kind: &str,
    plugin_api: u32,
    content_owner: &str,
    extension_path: &str,
    entry_path: &str,
    storage_path: &str,
    declared_agents: &[ProviderDescriptor],
    limits: &InitializeLimits,
) -> String {
    let agents: Vec<serde_json::Value> = declared_agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "contractVersion": a.contract_version,
            })
        })
        .collect();

    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "$/initialize",
        "params": {
            "wireVersion": wire_version,
            "hostVersion": host_version,
            "runtimeVersion": runtime_version,
            "sessionId": session_id,
            "plugin": {
                "id": plugin_id,
                "version": plugin_version,
                "kind": plugin_kind,
                "pluginApi": plugin_api,
                "contentOwner": content_owner,
            },
            "paths": {
                "extensionPath": extension_path,
                "entryPath": entry_path,
                "storagePath": storage_path,
            },
            "declaredAgents": agents,
            "limits": {
                "maxFrameBytes": limits.max_frame_bytes,
                "maxPendingRequests": limits.max_pending_requests,
                "maxAgentEventBytes": limits.max_agent_event_bytes,
                "maxAgentResultBytes": limits.max_agent_result_bytes,
                "maxAgentPromptBytes": limits.max_agent_prompt_bytes,
                "maxActiveTurns": limits.max_active_turns,
                "maxPageItems": limits.max_page_items,
            },
        },
    }))
    .unwrap()
}

/// Dynamic limits sent in `$/initialize`.
#[derive(Debug, Clone)]
pub struct InitializeLimits {
    pub max_frame_bytes: u32,
    pub max_pending_requests: u32,
    pub max_agent_event_bytes: u32,
    pub max_agent_result_bytes: u32,
    pub max_agent_prompt_bytes: u32,
    pub max_active_turns: u32,
    pub max_page_items: u32,
}

impl Default for InitializeLimits {
    fn default() -> Self {
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
}

/// Build the response to `$/initialize` (identity echo).
pub fn build_initialize_response(
    id: &str,
    wire_version: u32,
    runtime_version: &str,
    session_id: &str,
    plugin_id: &str,
    plugin_version: &str,
) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "wireVersion": wire_version,
            "runtimeVersion": runtime_version,
            "sessionId": session_id,
            "plugin": {
                "id": plugin_id,
                "version": plugin_version,
            },
        },
    }))
    .unwrap()
}

/// Build the JSON-RPC request for `$/activate`.
pub fn build_activate_request(id: &str, reason: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "$/activate",
        "params": { "reason": reason },
    }))
    .unwrap()
}

/// Build the JSON-RPC request for `$/deactivate`.
pub fn build_deactivate_request(id: &str, reason: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "$/deactivate",
        "params": { "reason": reason },
    }))
    .unwrap()
}

/// Build the `$/exit` notification.
pub fn build_exit_notification() -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "$/exit",
    }))
    .unwrap()
}

/// Build a `$/cancelRequest` notification.
pub fn build_cancel_request(id: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "$/cancelRequest",
        "params": { "id": id },
    }))
    .unwrap()
}

/// Parse the `$/initialize` response and extract the echoed identity.
pub fn parse_initialize_response(
    result: &serde_json::Value,
) -> Result<InitializeOutcome, String> {
    let wire_version = result["wireVersion"]
        .as_u64()
        .ok_or("missing wireVersion")? as u32;
    let runtime_version = result["runtimeVersion"]
        .as_str()
        .ok_or("missing runtimeVersion")?
        .to_string();
    let session_id = result["sessionId"]
        .as_str()
        .ok_or("missing sessionId")?
        .to_string();
    let plugin = &result["plugin"];
    let plugin_id = plugin["id"].as_str().ok_or("missing plugin.id")?.to_string();
    let plugin_version = plugin["version"]
        .as_str()
        .ok_or("missing plugin.version")?
        .to_string();

    Ok(InitializeOutcome {
        wire_version,
        runtime_version,
        session_id,
        plugin_id,
        plugin_version,
    })
}

/// Parse the `$/activate` response and extract the provider list.
pub fn parse_activate_response(
    result: &serde_json::Value,
) -> Result<ActivateOutcome, String> {
    let providers = result["providers"]
        .as_array()
        .ok_or("missing providers array")?;
    let parsed: Vec<ProviderDescriptor> = providers
        .iter()
        .map(|p| {
            Ok(ProviderDescriptor {
                id: p["id"].as_str().ok_or("missing provider id")?.to_string(),
                contract_version: p["contractVersion"]
                    .as_u64()
                    .ok_or("missing contractVersion")? as u32,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(ActivateOutcome { providers: parsed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_initialize_request_contains_all_fields() {
        let req = build_initialize_request(
            "h:1", 1, "0.1.0", "0.1.0", "sess-1",
            "ora.test", "1.0.0", "agent", 1, "sha256-abc",
            "/p/test", "/p/test/dist/index.js", "/d/test",
            &[ProviderDescriptor { id: "agent1".into(), contract_version: 1 }],
            &InitializeLimits::default(),
        );
        let parsed: serde_json::Value = serde_json::from_str(&req).unwrap();
        assert_eq!(parsed["method"], "$/initialize");
        assert_eq!(parsed["params"]["plugin"]["id"], "ora.test");
        assert_eq!(parsed["params"]["limits"]["maxFrameBytes"], 8_388_608);
    }

    #[test]
    fn build_initialize_response_echoes_identity() {
        let resp = build_initialize_response("h:1", 1, "0.1.0", "sess-1", "ora.test", "1.0.0");
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        let result = &parsed["result"];
        assert_eq!(result["plugin"]["id"], "ora.test");
        assert_eq!(result["sessionId"], "sess-1");
    }

    #[test]
    fn build_activate_has_reason() {
        let req = build_activate_request("h:2", "lazyInvocation");
        let parsed: serde_json::Value = serde_json::from_str(&req).unwrap();
        assert_eq!(parsed["method"], "$/activate");
        assert_eq!(parsed["params"]["reason"], "lazyInvocation");
    }

    #[test]
    fn parse_activate_response_extracts_providers() {
        let json = serde_json::json!({
            "providers": [
                {"id": "agent1", "contractVersion": 1},
                {"id": "agent2", "contractVersion": 1}
            ]
        });
        let outcome = parse_activate_response(&json).unwrap();
        assert_eq!(outcome.providers.len(), 2);
        assert_eq!(outcome.providers[0].id, "agent1");
    }

    #[test]
    fn build_deactivate_and_exit() {
        let deact = build_deactivate_request("h:99", "shutdown");
        let parsed: serde_json::Value = serde_json::from_str(&deact).unwrap();
        assert_eq!(parsed["method"], "$/deactivate");

        let exit = build_exit_notification();
        let parsed: serde_json::Value = serde_json::from_str(&exit).unwrap();
        assert_eq!(parsed["method"], "$/exit");
        assert!(parsed.get("id").is_none());
    }
}
