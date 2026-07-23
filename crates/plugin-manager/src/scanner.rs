use crate::config::PluginManagerConfig;
use crate::PluginMetadata;
use serde::Deserialize;
use std::path::PathBuf;

/// A plugin discovered by scanning the plugins directory.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// Canonical plugin id from manifest (e.g. "ora.demo-plugin").
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Kind from manifest ("agent" or "workbench").
    pub kind: String,
    /// Plugin version string.
    pub version: String,
    /// Absolute path to the plugin entry file.
    pub entry_path: PathBuf,
    /// The plugin's root directory.
    pub plugin_dir: PathBuf,
    /// Typed metadata extracted from the manifest based on `kind`.
    pub metadata: PluginMetadata,
}

/// Minimal package.json shape — we only extract the `ora` block.
#[derive(Deserialize)]
struct PackageJson {
    version: Option<String>,
    ora: Option<OraManifest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OraManifest {
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    kind: String,
    main: String,
    /// Agent-specific metadata (only meaningful when kind = "agent").
    #[serde(default)]
    agent: Option<AgentManifest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentManifest {
    cli: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    description: Option<String>,
}

/// Scans `<data_dir>/plugins/` for subdirectories containing a valid package.json
/// with an `ora` manifest block.
pub fn scan_plugins(config: &PluginManagerConfig) -> Vec<DiscoveredPlugin> {
    let plugins_dir = config.plugins_dir();
    let mut discovered = Vec::new();

    let entries = match std::fs::read_dir(&plugins_dir) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("[scanner] plugins dir not found: {}", plugins_dir.display());
            return discovered;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let pkg_path = path.join("package.json");
        let pkg_bytes = match std::fs::read(&pkg_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let pkg: PackageJson = match serde_json::from_slice(&pkg_bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[scanner] invalid package.json in {}: {e}", path.display());
                continue;
            }
        };

        let ora = match pkg.ora {
            Some(o) => o,
            None => continue,
        };

        let version = pkg.version.unwrap_or_else(|| "0.0.0".into());
        let display_name = if ora.display_name.is_empty() {
            ora.id.clone()
        } else {
            ora.display_name
        };
        let entry_path = path.join(&ora.main);

        let metadata = match ora.kind.as_str() {
            "agent" => {
                let agent = ora.agent.unwrap_or_else(|| AgentManifest {
                    cli: ora.id.clone(),
                    display_name: display_name.clone(),
                    description: None,
                });
                PluginMetadata::Agent {
                    cli: agent.cli,
                    display_name: if agent.display_name.is_empty() {
                        display_name.clone()
                    } else {
                        agent.display_name
                    },
                    description: agent.description,
                }
            }
            _ => PluginMetadata::Workbench,
        };

        discovered.push(DiscoveredPlugin {
            id: ora.id,
            display_name,
            kind: ora.kind,
            version,
            entry_path,
            plugin_dir: path,
            metadata,
        });
    }

    discovered.sort_by(|a, b| a.id.cmp(&b.id));
    discovered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginManagerConfig;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discovers_plugin_from_plugins_dir() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("plugins").join("demo-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();

        let pkg = serde_json::json!({
            "name": "@ora-plugins/demo",
            "version": "0.1.0",
            "ora": {
                "manifestVersion": 1,
                "id": "ora.demo",
                "displayName": "Demo",
                "kind": "agent",
                "main": "index.ts",
                "agent": {
                    "cli": "opencode",
                    "displayName": "OpenCode",
                    "description": "OpenCode ACP agent"
                }
            }
        });
        fs::write(plugin_dir.join("package.json"), serde_json::to_string_pretty(&pkg).unwrap()).unwrap();
        fs::write(plugin_dir.join("index.ts"), "// demo").unwrap();

        let config = PluginManagerConfig::new(tmp.path());
        let plugins = scan_plugins(&config);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "ora.demo");
        assert_eq!(plugins[0].display_name, "Demo");
        assert_eq!(plugins[0].kind, "agent");
        assert!(matches!(plugins[0].metadata, PluginMetadata::Agent { .. }));
        if let PluginMetadata::Agent { cli, display_name, .. } = &plugins[0].metadata {
            assert_eq!(cli, "opencode");
            assert_eq!(display_name, "OpenCode");
        }
    }

    #[test]
    fn discovers_workbench_plugin() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("plugins").join("workbench-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();

        let pkg = serde_json::json!({
            "name": "@ora-plugins/workbench",
            "version": "0.1.0",
            "ora": {
                "id": "ora.workbench",
                "displayName": "Workbench",
                "kind": "workbench",
                "main": "index.ts"
            }
        });
        fs::write(plugin_dir.join("package.json"), serde_json::to_string_pretty(&pkg).unwrap()).unwrap();
        fs::write(plugin_dir.join("index.ts"), "// workbench").unwrap();

        let config = PluginManagerConfig::new(tmp.path());
        let plugins = scan_plugins(&config);
        assert_eq!(plugins.len(), 1);
        assert!(matches!(plugins[0].metadata, PluginMetadata::Workbench));
    }

    #[test]
    fn skips_dir_without_manifest() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("plugins").join("no-manifest");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("README.md"), "just a file").unwrap();

        let config = PluginManagerConfig::new(tmp.path());
        let plugins = scan_plugins(&config);
        assert!(plugins.is_empty());
    }
}
