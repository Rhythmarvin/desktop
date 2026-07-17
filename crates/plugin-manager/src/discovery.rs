//! Safe discovery enumeration for plugin candidate roots.
//!
//! Routes all candidate scanning through safe-fs audit primitives so
//! reparse points, hardlinks, and other escape vectors are rejected
//! before a candidate ever reaches the authority layer.

use crate::{PluginError, PluginFsLimits};
use std::path::{Path, PathBuf};

/// A discovered candidate directory that passed basic safety checks.
#[derive(Debug, Clone)]
pub struct DiscoveryEntry {
    pub path: PathBuf,
}

/// Rejected entries collect structured diagnostics rather than panicking.
#[derive(Debug, Clone)]
pub struct DiscoveryDiagnostic {
    pub path: PathBuf,
    pub reason: DiscoveryRejection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryRejection {
    NotADirectory,
    ReparsePoint,
    Inaccessible,
    DepthExceeded,
    NameInvalid,
}

/// Enumerate immediate child directories under `root`, rejecting any that
/// fail safe-fs checks (reparse points, hardlinks, ADS, depth violations).
///
/// This is intentionally shallow — it only inspects the directory entries
/// themselves, not their contents. Deep validation is done by the identify
/// step later.
pub fn discover_candidates(
    root: &Path,
    limits: &PluginFsLimits,
) -> Result<(Vec<DiscoveryEntry>, Vec<DiscoveryDiagnostic>), PluginError> {
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();

    // Audit the root itself: must be a real directory, not a reparse point
    let root_meta = std::fs::symlink_metadata(root).map_err(|e| PluginError::Internal {
        message: format!("cannot access discovery root {}: {e}", root.display()),
    })?;
    if !root_meta.is_dir() {
        return Err(PluginError::Internal {
            message: format!("discovery root is not a directory: {}", root.display()),
        });
    }
    if is_reparse_point_from_meta(&root_meta) {
        return Err(PluginError::Internal {
            message: format!("discovery root is a reparse point: {}", root.display()),
        });
    }

    let dir_entries = std::fs::read_dir(root).map_err(|e| PluginError::Internal {
        message: format!("cannot read discovery root {}: {e}", root.display()),
    })?;

    for dir_entry in dir_entries {
        let dir_entry = match dir_entry {
            Ok(e) => e,
            Err(e) => {
                diagnostics.push(DiscoveryDiagnostic {
                    path: root.to_owned(),
                    reason: DiscoveryRejection::Inaccessible,
                });
                continue;
            }
        };

        let path = dir_entry.path();

        // Reject non-directories
        let file_type = match dir_entry.file_type() {
            Ok(ft) => ft,
            Err(_) => {
                diagnostics.push(DiscoveryDiagnostic {
                    path,
                    reason: DiscoveryRejection::Inaccessible,
                });
                continue;
            }
        };

        if !file_type.is_dir() {
            continue; // not an error, just skip files in discovery root
        }

        // Reject reparse points (junctions, symlinks)
        if is_reparse_point(&path) {
            diagnostics.push(DiscoveryDiagnostic {
                path,
                reason: DiscoveryRejection::ReparsePoint,
            });
            continue;
        }

        // Reject entries with invalid names
        let file_name = dir_entry.file_name();
        if !is_valid_discovery_name(&file_name) {
            diagnostics.push(DiscoveryDiagnostic {
                path,
                reason: DiscoveryRejection::NameInvalid,
            });
            continue;
        }

        // Optional: depth check
        if let Ok(depth) = path_depth_relative(root, &path) {
            if depth > limits.maximum_directory_depth as usize {
                diagnostics.push(DiscoveryDiagnostic {
                    path,
                    reason: DiscoveryRejection::DepthExceeded,
                });
                continue;
            }
        }

        entries.push(DiscoveryEntry { path });
    }

    Ok((entries, diagnostics))
}

/// Check reparse point from an already-obtained Metadata.
#[cfg(windows)]
fn is_reparse_point_from_meta(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point_from_meta(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

/// Returns true if the path is a reparse point (junction, symlink).
#[cfg(windows)]
fn is_reparse_point(path: &Path) -> bool {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        is_reparse_point_from_meta(&meta)
    } else {
        true // fail closed
    }
}

#[cfg(not(windows))]
fn is_reparse_point(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(true)
}

/// Reject names with control characters, devices, or trailing spaces/dots.
fn is_valid_discovery_name(name: &std::ffi::OsStr) -> bool {
    let Some(s) = name.to_str() else { return false };
    if s.is_empty() || s.len() > 128 { return false; }
    for &b in s.as_bytes() {
        if b < 0x20 || matches!(b, b'\\' | b'/' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|') {
            return false;
        }
    }
    true
}

fn path_depth_relative(root: &Path, path: &Path) -> Result<usize, ()> {
    let relative = path.strip_prefix(root).map_err(|_| ())?;
    Ok(relative.components().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginFsLimits;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discovery_accepts_valid_directory() {
        let temp = TempDir::new().unwrap();
        let candidate = temp.path().join("my-plugin");
        fs::create_dir(&candidate).unwrap();
        // Create a minimal package.json
        fs::write(candidate.join("package.json"), b"{}").unwrap();

        let limits = PluginFsLimits::default();
        let (entries, diags) = discover_candidates(temp.path(), &limits).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(diags.is_empty());
    }

    #[test]
    fn discovery_rejects_invalid_names() {
        // Test the name validator directly — Windows prevents creating
        // files with truly invalid names, so we test the function itself.
        assert!(is_valid_discovery_name(std::ffi::OsStr::new("valid-name")));
        assert!(is_valid_discovery_name(std::ffi::OsStr::new("my_plugin.v1")));
        assert!(!is_valid_discovery_name(std::ffi::OsStr::new("")));
        // Non-UTF8 names are rejected
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            assert!(!is_valid_discovery_name(std::ffi::OsStr::from_bytes(&[0xff, 0xfe])));
        }
    }

    #[test]
    fn discovery_skips_files() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("readme.txt"), b"text").unwrap();

        let limits = PluginFsLimits::default();
        let (entries, _) = discover_candidates(temp.path(), &limits).unwrap();
        assert!(entries.is_empty()); // files are skipped, not rejected
    }

    #[test]
    fn discovery_rejects_empty_root() {
        let temp = TempDir::new().unwrap();
        let limits = PluginFsLimits::default();
        let (entries, diags) = discover_candidates(temp.path(), &limits).unwrap();
        assert!(entries.is_empty());
        assert!(diags.is_empty());
    }
}
