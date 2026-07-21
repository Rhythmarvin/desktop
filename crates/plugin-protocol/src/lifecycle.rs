use serde::{Deserialize, Serialize};

/// Parameters sent by the Host in the `$/initialize` Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub wire_version: i32,
    pub host_version: String,
    pub runtime_version: String,
    pub session_id: String,
    pub plugin: PluginIdentity,
    pub paths: PluginPaths,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPaths {
    pub extension_path: String,
    pub entry_path: String,
    pub storage_path: String,
}

/// Response sent by the bootstrap in the `$/initialize` Result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub wire_version: i32,
    pub runtime_version: String,
    pub session_id: String,
    pub plugin: PluginIdentity,
}

/// Parameters sent by the Host in the optional `$/activate` Request (deferred to post-MVP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateParams {
    pub reason: ActivationReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivationReason {
    LazyInvocation,
    ManualStart,
}

/// Parameters sent by the Host in the `$/deactivate` Request (deferred to post-MVP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateParams {
    pub reason: DeactivationReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeactivationReason {
    ManualStop,
    Disable,
    Uninstall,
    Shutdown,
    GrantChanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_params_roundtrip() {
        let json = serde_json::json!({
            "wireVersion": 1,
            "hostVersion": "0.1.0",
            "runtimeVersion": "0.1.0",
            "sessionId": "abc-123",
            "plugin": { "id": "test.plugin", "version": "0.1.0" },
            "paths": {
                "extensionPath": "/tmp/ext",
                "entryPath": "/tmp/ext/index.ts",
                "storagePath": "/tmp/data"
            }
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.wire_version, 1);
        assert_eq!(params.plugin.id, "test.plugin");
        assert_eq!(params.paths.entry_path, "/tmp/ext/index.ts");
    }

    #[test]
    fn initialize_result_roundtrip() {
        let result = InitializeResult {
            wire_version: 1,
            runtime_version: "0.1.0".into(),
            session_id: "abc-123".into(),
            plugin: PluginIdentity {
                id: "test.plugin".into(),
                version: "0.1.0".into(),
            },
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["sessionId"], "abc-123");
    }
}