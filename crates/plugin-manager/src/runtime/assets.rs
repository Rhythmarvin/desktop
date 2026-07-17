//! Runtime asset management — locked Bun version, bootstrap, and config.
//!
//! Assets are shipped with the Ora application and verified at spawn time.
//! Each versioned asset set has a receipt and is atomically deployed to a
//! versioned directory. Active assets are reference-counted via leases.

use std::path::PathBuf;

/// Identifies a pinned runtime asset version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetIdentity {
    /// Locked Bun version (e.g., "1.3.14").
    pub bun_version: String,
    /// SHA-256 of bun.exe.
    pub bun_sha256: String,
    /// SHA-256 of plugin-host-bootstrap.js.
    pub bootstrap_sha256: String,
    /// Wire protocol version (must match Rust host).
    pub wire_version: u32,
    /// Windows target triple.
    pub target: String,
}

/// Runtime asset receipt — stored alongside deployed assets.
#[derive(Debug, Clone)]
pub struct AssetReceipt {
    pub identity: AssetIdentity,
    /// The versioned directory name (e.g., "v0.1.0").
    pub version: String,
    /// Path to the deployed asset root.
    pub path: PathBuf,
    /// Whether this is the currently active version.
    pub active: bool,
}

/// Store managing runtime asset deployment and verification.
pub struct RuntimeAssetStore {
    /// Root directory for runtime assets.
    root: PathBuf,
    /// Receipts for all deployed versions.
    receipts: Vec<AssetReceipt>,
}

impl RuntimeAssetStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            receipts: Vec::new(),
        }
    }

    /// Get the active asset receipt, if any.
    pub fn active_receipt(&self) -> Option<&AssetReceipt> {
        self.receipts.iter().find(|r| r.active)
    }

    /// Get the path to the active bun executable.
    pub fn bun_path(&self) -> Option<PathBuf> {
        self.active_receipt()
            .map(|r| r.path.join("bun.exe"))
    }

    /// Get the path to the active bootstrap script.
    pub fn bootstrap_path(&self) -> Option<PathBuf> {
        self.active_receipt()
            .map(|r| r.path.join("plugin-host-bootstrap.js"))
    }

    /// Get the path to the empty bunfig.toml.
    pub fn bunfig_path(&self) -> Option<PathBuf> {
        self.active_receipt()
            .map(|r| r.path.join("empty-bunfig.toml"))
    }

    /// Register a deployed asset version.
    pub fn register(&mut self, receipt: AssetReceipt) {
        self.receipts.push(receipt);
    }

    /// Set the active version.
    pub fn set_active(&mut self, version: &str) -> Result<(), String> {
        // First, find the target index
        let target_idx = self
            .receipts
            .iter()
            .position(|r| r.version == version)
            .ok_or_else(|| format!("no asset version: {version}"))?;
        // Deactivate all
        for r in &mut self.receipts {
            r.active = false;
        }
        // Activate the target
        self.receipts[target_idx].active = true;
        Ok(())
    }

    /// Whether a valid active asset is available.
    pub fn is_ready(&self) -> bool {
        self.active_receipt().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_store() -> RuntimeAssetStore {
        RuntimeAssetStore::new(PathBuf::from("/test/runtime"))
    }

    fn make_receipt(version: &str, path: &str) -> AssetReceipt {
        AssetReceipt {
            identity: AssetIdentity {
                bun_version: "1.3.14".into(),
                bun_sha256: "abc123".into(),
                bootstrap_sha256: "def456".into(),
                wire_version: 1,
                target: "x86_64-pc-windows-msvc".into(),
            },
            version: version.to_string(),
            path: PathBuf::from(path),
            active: false,
        }
    }

    #[test]
    fn store_starts_not_ready() {
        let store = make_store();
        assert!(!store.is_ready());
        assert!(store.active_receipt().is_none());
    }

    #[test]
    fn set_active_makes_ready() {
        let mut store = make_store();
        store.register(make_receipt("v0.1.0", "/test/runtime/v0.1.0"));
        store.set_active("v0.1.0").unwrap();
        assert!(store.is_ready());
        assert_eq!(
            store.active_receipt().unwrap().version,
            "v0.1.0"
        );
    }

    #[test]
    fn only_one_active_at_a_time() {
        let mut store = make_store();
        store.register(make_receipt("v0.1.0", "/test/v0.1.0"));
        store.register(make_receipt("v0.2.0", "/test/v0.2.0"));
        store.set_active("v0.1.0").unwrap();
        store.set_active("v0.2.0").unwrap();
        let active_versions: Vec<_> = store.receipts.iter().filter(|r| r.active).collect();
        assert_eq!(active_versions.len(), 1);
        assert_eq!(active_versions[0].version, "v0.2.0");
    }

    #[test]
    fn unknown_version_errors() {
        let mut store = make_store();
        assert!(store.set_active("nonexistent").is_err());
    }

    #[test]
    fn paths_resolve_correctly() {
        let mut store = make_store();
        store.register(make_receipt("v0.1.0", "/test/runtime/v0.1.0"));
        store.set_active("v0.1.0").unwrap();
        assert!(store.bun_path().unwrap().ends_with("bun.exe"));
        assert!(store.bootstrap_path().unwrap().ends_with("plugin-host-bootstrap.js"));
        assert!(store.bunfig_path().unwrap().ends_with("empty-bunfig.toml"));
    }
}
