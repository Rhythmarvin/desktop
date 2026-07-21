use std::path::{Path, PathBuf};

/// Carries typed production policy and derives every managed plugin path from one data root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManagerConfig {
    data_dir: PathBuf,
}

impl PluginManagerConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Directory scanned for plugin candidates.
    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir.join("plugins")
    }

    /// Per-plugin mutable data root.
    pub fn plugin_data_dir(&self) -> PathBuf {
        self.data_dir.join("plugin-data")
    }
}
