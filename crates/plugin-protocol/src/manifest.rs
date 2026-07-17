use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::identity::{AgentProviderId, PluginId};

/// Relative path to a plugin's main entry file. Must be within the plugin root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginRelativePath(String);

/// Plugin kind discriminant — must be an enum with associated data, not a struct with Option fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum PluginKindManifest {
    #[serde(rename = "agent")]
    Agent {
        main: PluginRelativePath,
        contributes: AgentContributions,
    },
    #[serde(rename = "workbench")]
    Workbench { contributes: WorkbenchContributions },
}

/// Agent contribution declarations in a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentContributions {
    pub agents: Vec<AgentContribution>,
}

/// Workbench contribution declarations (v1 placeholder).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct WorkbenchContributions {
    #[serde(rename = "workbench")]
    pub workbench: WorkbenchContributionSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct WorkbenchContributionSet {
    pub schema_version: u32,
}

/// A single agent contribution entry in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentContribution {
    pub id: AgentProviderId,
    pub display_name: String,
    pub contract_version: u32,
}

/// Engine compatibility constraints declared in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginEngines {
    /// SemVer range for the Ora application (e.g., ">=0.1.0 <0.2.0")
    pub ora: String,
    /// Exact plugin API version — must be exactly 1 for Agent v1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_api: Option<u32>,
    /// SemVer range for Bun runtime (e.g., ">=1.0.0 <2.0.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bun: Option<String>,
}

/// The complete `ora` object within a plugin's `package.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginManifest {
    pub manifest_version: u32,
    pub id: PluginId,
    pub display_name: String,
    pub kind: PluginKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<PluginRelativePath>,
    pub engines: PluginEngines,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributes: Option<ManifestContributes>,
}

/// The `kind` field in the manifest — just the string discriminant for routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "plugin-protocol.ts")]
pub enum PluginKind {
    Agent,
    Workbench,
}

/// Contribution wrapper in the manifest `ora` object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ManifestContributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<AgentContribution>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workbench: Option<WorkbenchContributionSet>,
}

/// Manifest file validity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestValidity {
    Valid,
    Invalid {
        diagnostics: Vec<ManifestDiagnostic>,
    },
}

/// Whether the current Ora/Bun/OS platform supports this plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCompatibility {
    Compatible,
    Incompatible { reasons: Vec<String> },
}

/// Whether the current Host implements an executor for this plugin kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSupport {
    Supported,
    Unsupported { reason: String },
}

/// Integrity of the managed copy vs receipt digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityStatus {
    Intact,
    Mismatch { expected: String, actual: String },
    MissingReceipt,
}

/// A diagnostic message from manifest validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct ManifestDiagnostic {
    pub field: Option<String>,
    pub message: String,
}

impl PluginRelativePath {
    pub fn new(raw: &str) -> Result<Self, String> {
        if raw.is_empty() {
            return Err("relative path must not be empty".to_string());
        }
        if raw.starts_with('/') || raw.starts_with('\\') {
            return Err("relative path must not be absolute".to_string());
        }
        if raw.contains("..") {
            return Err("relative path must not contain '..'".to_string());
        }
        if raw.contains(':') {
            return Err("relative path must not contain colons (no ADS)".to_string());
        }
        // Check for Windows drive letter patterns
        if raw.len() >= 2 && raw.as_bytes().get(1) == Some(&b':') {
            return Err("relative path must not look like absolute Windows path".to_string());
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ── PluginRelativePath ────────────────────────────────────────

    #[test]
    fn valid_relative_path() {
        let p = PluginRelativePath::new("dist/index.js").unwrap();
        assert_eq!(p.as_str(), "dist/index.js");
    }

    #[test]
    fn reject_absolute_path() {
        assert!(PluginRelativePath::new("/etc/passwd").is_err());
    }

    #[test]
    fn reject_parent_traversal() {
        assert!(PluginRelativePath::new("../dist/index.js").is_err());
    }

    #[test]
    fn reject_colon() {
        assert!(PluginRelativePath::new("file:stream").is_err());
    }

    // ── PluginKindManifest serde ──────────────────────────────────

    #[test]
    fn agent_manifest_serde_roundtrip() {
        let manifest = PluginKindManifest::Agent {
            main: PluginRelativePath::new("dist/index.js").unwrap(),
            contributes: AgentContributions {
                agents: vec![AgentContribution {
                    id: AgentProviderId::new("claude-code").unwrap(),
                    display_name: "Claude Code".to_string(),
                    contract_version: 1,
                }],
            },
        };

        let json = serde_json::to_value(&manifest).unwrap();
        assert_eq!(json["kind"], "agent");
        assert_eq!(json["main"], "dist/index.js");

        let decoded: PluginKindManifest = serde_json::from_value(json).unwrap();
        match decoded {
            PluginKindManifest::Agent { main, contributes } => {
                assert_eq!(main.as_str(), "dist/index.js");
                assert_eq!(contributes.agents.len(), 1);
                assert_eq!(contributes.agents[0].id.as_str(), "claude-code");
            }
            _ => panic!("expected Agent variant"),
        }
    }

    #[test]
    fn workbench_manifest_serde_roundtrip() {
        let manifest = PluginKindManifest::Workbench {
            contributes: WorkbenchContributions {
                workbench: WorkbenchContributionSet { schema_version: 1 },
            },
        };

        let json = serde_json::to_value(&manifest).unwrap();
        assert_eq!(json["kind"], "workbench");

        let decoded: PluginKindManifest = serde_json::from_value(json).unwrap();
        assert!(matches!(decoded, PluginKindManifest::Workbench { .. }));
    }

    // ── PluginManifest ────────────────────────────────────────────

    #[test]
    fn full_agent_manifest_serde() {
        let json = serde_json::json!({
            "manifestVersion": 1,
            "id": "ora.claude-code",
            "displayName": "Claude Code",
            "kind": "agent",
            "main": "dist/index.js",
            "engines": {
                "ora": ">=0.1.0 <0.2.0",
                "pluginApi": 1,
                "bun": ">=1.0.0 <2.0.0"
            },
            "contributes": {
                "agents": [{
                    "id": "claude-code",
                    "displayName": "Claude Code",
                    "contractVersion": 1
                }]
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert_eq!(manifest.manifest_version, 1);
        assert_eq!(manifest.id.as_str(), "ora.claude-code");
        assert_eq!(manifest.display_name, "Claude Code");
        assert!(matches!(manifest.kind, PluginKind::Agent));

        let back = serde_json::to_value(&manifest).unwrap();
        let decoded: PluginManifest = serde_json::from_value(back).unwrap();
        assert_eq!(decoded.id.as_str(), "ora.claude-code");
    }

    #[test]
    fn workbench_manifest_serde() {
        let json = serde_json::json!({
            "manifestVersion": 1,
            "id": "ora.example-workbench",
            "displayName": "Example Workbench",
            "kind": "workbench",
            "engines": {
                "ora": ">=0.1.0 <0.2.0"
            },
            "contributes": {
                "workbench": {
                    "schemaVersion": 1
                }
            }
        });

        let manifest: PluginManifest = serde_json::from_value(json).unwrap();
        assert!(matches!(manifest.kind, PluginKind::Workbench));
    }

    // ── Validation tiers ─────────────────────────────────────────

    #[test]
    fn manifest_validity_variants() {
        let valid = ManifestValidity::Valid;
        assert!(matches!(valid, ManifestValidity::Valid));

        let invalid = ManifestValidity::Invalid {
            diagnostics: vec![ManifestDiagnostic {
                field: Some("id".to_string()),
                message: "invalid plugin id".to_string(),
            }],
        };
        match invalid {
            ManifestValidity::Invalid { diagnostics } => {
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].field.as_deref(), Some("id"));
            }
            _ => panic!("expected Invalid"),
        }
    }

    #[test]
    fn runtime_compatibility_variants() {
        let compat = RuntimeCompatibility::Compatible;
        assert!(matches!(compat, RuntimeCompatibility::Compatible));

        let incompat = RuntimeCompatibility::Incompatible {
            reasons: vec!["bun version too old".to_string()],
        };
        match incompat {
            RuntimeCompatibility::Incompatible { reasons } => {
                assert_eq!(reasons.len(), 1);
            }
            _ => panic!("expected Incompatible"),
        }
    }

    #[test]
    fn runtime_support_unsupported_workbench() {
        let support = RuntimeSupport::Unsupported {
            reason: "Workbench executor not implemented in MVP".to_string(),
        };
        assert!(matches!(support, RuntimeSupport::Unsupported { .. }));
    }

    #[test]
    fn integrity_status_mismatch() {
        let status = IntegrityStatus::Mismatch {
            expected: "sha256:abc".to_string(),
            actual: "sha256:def".to_string(),
        };
        assert!(matches!(status, IntegrityStatus::Mismatch { .. }));
    }
}
