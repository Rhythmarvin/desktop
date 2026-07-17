use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Typed configuration for all plugin-related budget values.
///
/// All budget values are centralized here. They must NOT be scattered across
/// scanner, installer, and runtime as magic numbers. Host policy can only
/// tighten these values — never relax them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginLimits {
    // ── Wire protocol ─────────────────────────────────────────────
    /// Maximum frame payload bytes (fixed at 8 MiB, non-negotiable).
    pub max_frame_bytes: u32,

    // ── Pending requests ──────────────────────────────────────────
    /// Maximum number of concurrent ordinary pending requests per connection.
    pub max_pending_requests: u32,

    // ── Agent event/result size ───────────────────────────────────
    /// Maximum single stream event payload bytes.
    pub max_agent_event_bytes: u32,
    /// Maximum single terminal result/error payload bytes.
    pub max_agent_result_bytes: u32,
    /// Maximum agent prompt bytes.
    pub max_agent_prompt_bytes: u32,

    // ── Conversation ──────────────────────────────────────────────
    /// Maximum concurrent active turns (provisional + bound) per plugin.
    pub max_active_turns: u32,
    /// Maximum page items for paginated responses.
    pub max_page_items: u32,

    // ── Fixed leaf-type caps (v1 hard caps, not dynamic) ──────────
    /// Maximum opaque id / cursor / call-id / turn-id bytes.
    pub max_opaque_id_bytes: u32,
    /// Maximum display name / tool name / status phase / version bytes.
    pub max_display_name_bytes: u32,
    /// Maximum configuration key bytes (ASCII).
    pub max_config_key_bytes: u32,
    /// Maximum diagnostic / description / summary / title / config string bytes.
    pub max_string_bytes: u32,
    /// Maximum config string-list items.
    pub max_string_list_items: u32,
    /// Maximum prompt bytes.
    pub max_prompt_bytes: u32,
    /// Maximum scope workingDirectory bytes.
    pub max_scope_path_bytes: u32,
    /// Maximum discovery installations per response.
    pub max_discovery_installations: u32,
    /// Maximum discovery diagnostics per response.
    pub max_discovery_diagnostics: u32,
    /// Maximum configuration items per response.
    pub max_configuration_items: u32,
    /// Maximum stream event payload bytes (hard cap).
    pub max_stream_event_bytes: u32,
    /// Maximum terminal result/error payload bytes (hard cap).
    pub max_terminal_result_bytes: u32,

    // ── File system ───────────────────────────────────────────────
    /// Maximum manifest (package.json) bytes.
    pub max_manifest_bytes: u32,
    /// Maximum file count in a plugin package.
    pub max_file_count: u32,
    /// Maximum single file bytes in a plugin package.
    pub max_file_bytes: u64,
    /// Maximum total bytes in a plugin package.
    pub max_total_bytes: u64,
    /// Maximum directory depth.
    pub max_directory_depth: u32,
    /// Maximum contribution count per manifest.
    pub max_contributions: u32,
    /// Maximum JSON nesting depth.
    pub max_json_depth: u32,
    /// Maximum plugin id bytes.
    pub max_plugin_id_bytes: u32,
    /// Maximum display name Unicode scalar values.
    pub max_display_name_chars: u32,
    /// Maximum entry relative path bytes.
    pub max_entry_path_bytes: u32,
}

impl Default for PluginLimits {
    fn default() -> Self {
        Self {
            // Wire protocol
            max_frame_bytes: 8 * 1024 * 1024, // 8 MiB

            // Pending requests
            max_pending_requests: 128,

            // Agent event/result size (dynamic, can be tightened per-generation)
            max_agent_event_bytes: 256 * 1024,   // 256 KiB
            max_agent_result_bytes: 1024 * 1024, // 1 MiB
            max_agent_prompt_bytes: 1024 * 1024, // 1 MiB

            // Conversation
            max_active_turns: 64,
            max_page_items: 100,

            // Fixed v1 hard caps
            max_opaque_id_bytes: 256,
            max_display_name_bytes: 512,
            max_config_key_bytes: 512,
            max_string_bytes: 4 * 1024, // 4 KiB
            max_string_list_items: 128,
            max_prompt_bytes: 1024 * 1024,   // 1 MiB
            max_scope_path_bytes: 32 * 1024, // 32 KiB
            max_discovery_installations: 128,
            max_discovery_diagnostics: 64,
            max_configuration_items: 256,
            max_stream_event_bytes: 256 * 1024,     // 256 KiB
            max_terminal_result_bytes: 1024 * 1024, // 1 MiB

            // File system
            max_manifest_bytes: 256 * 1024, // 256 KiB
            max_file_count: 10_000,
            max_file_bytes: 64 * 1024 * 1024,   // 64 MiB
            max_total_bytes: 512 * 1024 * 1024, // 512 MiB
            max_directory_depth: 64,
            max_contributions: 64,
            max_json_depth: 64,
            max_plugin_id_bytes: 128,
            max_display_name_chars: 128,
            max_entry_path_bytes: 512,
        }
    }
}

impl PluginLimits {
    /// Creates a new PluginLimits with defaults, applying only the 7 dynamic
    /// limits from an initialize-request projection. The caller must ensure
    /// the provided values do not exceed the hard caps in [`Default`].
    pub fn with_dynamic_limits(
        max_frame_bytes: u32,
        max_pending_requests: u32,
        max_agent_event_bytes: u32,
        max_agent_result_bytes: u32,
        max_agent_prompt_bytes: u32,
        max_active_turns: u32,
        max_page_items: u32,
    ) -> Self {
        let mut limits = Self::default();

        // Only allow tightening, never relaxing
        limits.max_frame_bytes = limits.max_frame_bytes.min(max_frame_bytes);
        limits.max_pending_requests = limits.max_pending_requests.min(max_pending_requests);
        limits.max_agent_event_bytes = limits.max_agent_event_bytes.min(max_agent_event_bytes);
        limits.max_agent_result_bytes = limits.max_agent_result_bytes.min(max_agent_result_bytes);
        limits.max_agent_prompt_bytes = limits.max_agent_prompt_bytes.min(max_agent_prompt_bytes);
        limits.max_active_turns = limits.max_active_turns.min(max_active_turns);
        limits.max_page_items = limits.max_page_items.min(max_page_items);

        limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_limits_are_reasonable() {
        let limits = PluginLimits::default();
        assert_eq!(limits.max_frame_bytes, 8 * 1024 * 1024);
        assert_eq!(limits.max_pending_requests, 128);
        assert_eq!(limits.max_active_turns, 64);
        assert_eq!(limits.max_page_items, 100);
        assert_eq!(limits.max_opaque_id_bytes, 256);
        assert_eq!(limits.max_manifest_bytes, 256 * 1024);
    }

    #[test]
    fn dynamic_limits_can_only_tighten() {
        let limits = PluginLimits::with_dynamic_limits(
            8 * 1024 * 1024, // max_frame_bytes (same as default)
            64,              // max_pending_requests (tightened from 128)
            128 * 1024,      // max_agent_event_bytes (tightened from 256 KiB)
            512 * 1024,      // max_agent_result_bytes (tightened from 1 MiB)
            512 * 1024,      // max_agent_prompt_bytes (tightened from 1 MiB)
            32,              // max_active_turns (tightened from 64)
            50,              // max_page_items (tightened from 100)
        );

        assert_eq!(limits.max_pending_requests, 64);
        assert_eq!(limits.max_agent_event_bytes, 128 * 1024);
        assert_eq!(limits.max_active_turns, 32);
        assert_eq!(limits.max_page_items, 50);
        // Fixed caps should remain unchanged
        assert_eq!(limits.max_opaque_id_bytes, 256);
    }

    #[test]
    fn dynamic_limits_cannot_relax_beyond_hard_caps() {
        let limits = PluginLimits::with_dynamic_limits(
            16 * 1024 * 1024, // tries to exceed 8 MiB frame cap
            256,              // tries to exceed 128 pending cap
            512 * 1024,       // tries to exceed 256 KiB event cap
            2 * 1024 * 1024,  // tries to exceed 1 MiB result cap
            2 * 1024 * 1024,  // tries to exceed 1 MiB prompt cap
            128,              // tries to exceed 64 active turns cap
            200,              // tries to exceed 100 page items cap
        );

        // All values should be clamped to hard caps
        assert_eq!(limits.max_frame_bytes, 8 * 1024 * 1024);
        assert_eq!(limits.max_pending_requests, 128);
        assert_eq!(limits.max_agent_event_bytes, 256 * 1024);
        assert_eq!(limits.max_agent_result_bytes, 1024 * 1024);
        assert_eq!(limits.max_agent_prompt_bytes, 1024 * 1024);
        assert_eq!(limits.max_active_turns, 64);
        assert_eq!(limits.max_page_items, 100);
    }

    #[test]
    fn limits_serde_roundtrip() {
        let limits = PluginLimits::default();
        let json = serde_json::to_value(&limits).unwrap();
        let decoded: PluginLimits = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.max_frame_bytes, 8 * 1024 * 1024);
        assert_eq!(decoded.max_opaque_id_bytes, 256);
    }
}
