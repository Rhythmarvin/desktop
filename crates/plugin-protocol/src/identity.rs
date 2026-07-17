use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Canonical plugin identifier following `publisher.name` format.
///
/// Grammar: `^[a-z0-9][a-z0-9-]{0,62}(\.[a-z0-9][a-z0-9-]{0,62})+$`
/// - ASCII lowercase only; no Unicode normalization or locale case-folding
/// - Max 128 bytes
/// - Rejects Windows device names (CON, PRN, AUX, NUL, COM1..9, LPT1..9)
/// - Rejects trailing dots, trailing spaces, colons, ADS syntax
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginId(String);

/// Plugin-local agent provider identifier.
///
/// Grammar: `^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$`
/// - 1..=63 bytes ASCII lowercase
/// - No dots, underscores, leading/trailing hyphens, Unicode, or case-folding
/// - Must be unique within a manifest's `contributes.agents`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentProviderId(String);

/// Globally unique agent provider key: structured pair `(PluginId, AgentProviderId)`.
/// Must NOT be constructed via unescaped string concatenation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export_to = "plugin-protocol.ts")]
pub struct AgentProviderKey {
    pub plugin_id: PluginId,
    pub provider_id: AgentProviderId,
}

/// Windows device names that are rejected in PluginId segments.
const WINDOWS_DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// PluginId validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginIdError {
    Empty,
    TooLong { max: usize, actual: usize },
    NotAsciiLowercase,
    InvalidSegment { segment: String, reason: String },
    SingleSegment,
    DeviceName { segment: String },
    TrailingDotOrSpace,
    ColonOrAds,
}

impl std::fmt::Display for PluginIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "plugin id must not be empty"),
            Self::TooLong { max, actual } => {
                write!(f, "plugin id too long: max {max} bytes, got {actual}")
            }
            Self::NotAsciiLowercase => write!(f, "plugin id must be ASCII lowercase"),
            Self::InvalidSegment { segment, reason } => {
                write!(f, "invalid plugin id segment '{segment}': {reason}")
            }
            Self::SingleSegment => {
                write!(f, "plugin id must have at least two dot-separated segments")
            }
            Self::DeviceName { segment } => {
                write!(
                    f,
                    "plugin id segment '{segment}' is a reserved Windows device name"
                )
            }
            Self::TrailingDotOrSpace => write!(f, "plugin id must not end with dot or space"),
            Self::ColonOrAds => write!(f, "plugin id must not contain colons or ADS syntax"),
        }
    }
}

/// AgentProviderId validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentProviderIdError {
    Empty,
    TooLong { max: usize, actual: usize },
    NotAsciiLowercase,
    InvalidChars { found: String },
    LeadingHyphen,
    TrailingHyphen,
    ContainsDot,
    ContainsUnderscore,
}

impl std::fmt::Display for AgentProviderIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "agent provider id must not be empty"),
            Self::TooLong { max, actual } => {
                write!(
                    f,
                    "agent provider id too long: max {max} bytes, got {actual}"
                )
            }
            Self::NotAsciiLowercase => write!(f, "agent provider id must be ASCII lowercase"),
            Self::InvalidChars { found } => {
                write!(f, "agent provider id contains invalid characters: {found}")
            }
            Self::LeadingHyphen => write!(f, "agent provider id must not start with a hyphen"),
            Self::TrailingHyphen => write!(f, "agent provider id must not end with a hyphen"),
            Self::ContainsDot => write!(f, "agent provider id must not contain dots"),
            Self::ContainsUnderscore => write!(f, "agent provider id must not contain underscores"),
        }
    }
}

impl PluginId {
    /// Maximum length in bytes for a PluginId.
    pub const MAX_BYTES: usize = 128;

    /// Validates and constructs a PluginId from a raw string.
    pub fn new(raw: &str) -> Result<Self, PluginIdError> {
        if raw.is_empty() {
            return Err(PluginIdError::Empty);
        }
        if raw.len() > Self::MAX_BYTES {
            return Err(PluginIdError::TooLong {
                max: Self::MAX_BYTES,
                actual: raw.len(),
            });
        }
        // Reject colons and ADS syntax early
        if raw.contains(':') {
            return Err(PluginIdError::ColonOrAds);
        }
        if !raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        {
            return Err(PluginIdError::NotAsciiLowercase);
        }
        if raw.ends_with('.') || raw.ends_with(' ') {
            return Err(PluginIdError::TrailingDotOrSpace);
        }

        let segments: Vec<&str> = raw.split('.').collect();
        if segments.len() < 2 {
            return Err(PluginIdError::SingleSegment);
        }

        for segment in &segments {
            validate_plugin_id_segment(segment)?;
        }

        Ok(Self(raw.to_string()))
    }

    /// Returns the raw string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_plugin_id_segment(segment: &str) -> Result<(), PluginIdError> {
    if segment.is_empty() {
        return Err(PluginIdError::InvalidSegment {
            segment: segment.to_string(),
            reason: "segment must not be empty".to_string(),
        });
    }
    if segment.len() > 63 {
        return Err(PluginIdError::InvalidSegment {
            segment: segment.to_string(),
            reason: format!("segment too long: max 63, got {}", segment.len()),
        });
    }
    // Check first char: must be a-z0-9 (lowercase alphanumeric)
    let first = segment.chars().next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return Err(PluginIdError::InvalidSegment {
            segment: segment.to_string(),
            reason: "segment must start with a-z or 0-9".to_string(),
        });
    }
    // Check last char: must be a-z0-9
    let last = segment.chars().last().unwrap();
    if !last.is_ascii_alphanumeric() {
        return Err(PluginIdError::InvalidSegment {
            segment: segment.to_string(),
            reason: "segment must end with a-z or 0-9".to_string(),
        });
    }
    // Check Windows device names (case-insensitive)
    let upper = segment.to_ascii_uppercase();
    let base_name = upper.split('.').next().unwrap_or(&upper);
    if WINDOWS_DEVICE_NAMES.contains(&base_name) {
        return Err(PluginIdError::DeviceName {
            segment: segment.to_string(),
        });
    }
    Ok(())
}

impl AgentProviderId {
    /// Maximum length in bytes for an AgentProviderId.
    pub const MAX_BYTES: usize = 63;

    /// Validates and constructs an AgentProviderId from a raw string.
    pub fn new(raw: &str) -> Result<Self, AgentProviderIdError> {
        if raw.is_empty() {
            return Err(AgentProviderIdError::Empty);
        }
        if raw.len() > Self::MAX_BYTES {
            return Err(AgentProviderIdError::TooLong {
                max: Self::MAX_BYTES,
                actual: raw.len(),
            });
        }
        if raw.starts_with('-') {
            return Err(AgentProviderIdError::LeadingHyphen);
        }
        if raw.ends_with('-') {
            return Err(AgentProviderIdError::TrailingHyphen);
        }
        if raw.contains('.') {
            return Err(AgentProviderIdError::ContainsDot);
        }
        if raw.contains('_') {
            return Err(AgentProviderIdError::ContainsUnderscore);
        }
        if !raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit())
        {
            // Build list of invalid chars for error message
            let invalid: String = raw
                .chars()
                .filter(|c| !c.is_ascii_lowercase() && *c != '-' && !c.is_ascii_digit())
                .collect();
            if invalid.is_empty() {
                // Must have been mixed case
                return Err(AgentProviderIdError::NotAsciiLowercase);
            }
            return Err(AgentProviderIdError::InvalidChars { found: invalid });
        }
        Ok(Self(raw.to_string()))
    }

    /// Returns the raw string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AgentProviderKey {
    pub fn new(plugin_id: PluginId, provider_id: AgentProviderId) -> Self {
        Self {
            plugin_id,
            provider_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ── PluginId ──────────────────────────────────────────────────

    #[test]
    fn valid_plugin_id_two_segments() {
        let id = PluginId::new("ora.claude-code").unwrap();
        assert_eq!(id.as_str(), "ora.claude-code");
    }

    #[test]
    fn valid_plugin_id_three_segments() {
        let id = PluginId::new("com.example.plugin").unwrap();
        assert_eq!(id.as_str(), "com.example.plugin");
    }

    #[test]
    fn valid_plugin_id_numeric_segments() {
        let id = PluginId::new("com.123plugin.test2").unwrap();
        assert_eq!(id.as_str(), "com.123plugin.test2");
    }

    #[test]
    fn valid_plugin_id_with_hyphens() {
        let id = PluginId::new("ora.my-plugin.test").unwrap();
        assert_eq!(id.as_str(), "ora.my-plugin.test");
    }

    #[test]
    fn reject_single_segment() {
        let err = PluginId::new("claude-code").unwrap_err();
        assert!(matches!(err, PluginIdError::SingleSegment));
    }

    #[test]
    fn reject_empty() {
        let err = PluginId::new("").unwrap_err();
        assert!(matches!(err, PluginIdError::Empty));
    }

    #[test]
    fn reject_trailing_dot() {
        let err = PluginId::new("ora.test.").unwrap_err();
        assert!(matches!(err, PluginIdError::TrailingDotOrSpace));
    }

    #[test]
    fn reject_colon() {
        let err = PluginId::new("ora:test.plugin").unwrap_err();
        assert!(matches!(err, PluginIdError::ColonOrAds));
    }

    #[test]
    fn reject_device_name_con() {
        let err = PluginId::new("con.helper").unwrap_err();
        assert!(matches!(err, PluginIdError::DeviceName { .. }));
    }

    #[test]
    fn reject_device_name_nul() {
        let err = PluginId::new("nul.tool").unwrap_err();
        assert!(matches!(err, PluginIdError::DeviceName { .. }));
    }

    #[test]
    fn reject_device_name_com1() {
        let err = PluginId::new("com1.port").unwrap_err();
        assert!(matches!(err, PluginIdError::DeviceName { .. }));
    }

    #[test]
    fn reject_device_name_lpt1() {
        let err = PluginId::new("lpt1.printer").unwrap_err();
        assert!(matches!(err, PluginIdError::DeviceName { .. }));
    }

    #[test]
    fn reject_device_name_case_insensitive() {
        let err = PluginId::new("Con.helper").unwrap_err();
        // "Con" should fail on NotAsciiLowercase OR DeviceName
        // Either is acceptable - it's invalid
        assert!(
            err.to_string().contains("lowercase")
                || err.to_string().contains("DeviceName")
                || err.to_string().contains("device")
        );
    }

    #[test]
    fn reject_empty_segment() {
        let err = PluginId::new("ora..test").unwrap_err();
        assert!(matches!(err, PluginIdError::InvalidSegment { .. }));
    }

    #[test]
    fn reject_segment_starting_with_hyphen() {
        let err = PluginId::new("-test.plugin").unwrap_err();
        assert!(matches!(err, PluginIdError::InvalidSegment { .. }));
    }

    #[test]
    fn reject_segment_ending_with_hyphen() {
        let err = PluginId::new("test-.plugin").unwrap_err();
        assert!(matches!(err, PluginIdError::InvalidSegment { .. }));
    }

    #[test]
    fn reject_too_long() {
        let long = "a".repeat(130);
        let err = PluginId::new(&long).unwrap_err();
        assert!(matches!(err, PluginIdError::TooLong { .. }));
    }

    #[test]
    fn serde_roundtrip_plugin_id() {
        let id = PluginId::new("ora.claude-code").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""ora.claude-code""#);
        let decoded: PluginId = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, id);
    }

    // ── AgentProviderId ───────────────────────────────────────────

    #[test]
    fn valid_provider_id_simple() {
        let id = AgentProviderId::new("claude-code").unwrap();
        assert_eq!(id.as_str(), "claude-code");
    }

    #[test]
    fn valid_provider_id_single_char() {
        let id = AgentProviderId::new("a").unwrap();
        assert_eq!(id.as_str(), "a");
    }

    #[test]
    fn valid_provider_id_numeric() {
        let id = AgentProviderId::new("agent42").unwrap();
        assert_eq!(id.as_str(), "agent42");
    }

    #[test]
    fn reject_provider_id_with_dots() {
        let err = AgentProviderId::new("claude.code").unwrap_err();
        assert!(matches!(err, AgentProviderIdError::ContainsDot));
    }

    #[test]
    fn reject_provider_id_with_underscores() {
        let err = AgentProviderId::new("claude_code").unwrap_err();
        assert!(matches!(err, AgentProviderIdError::ContainsUnderscore));
    }

    #[test]
    fn reject_provider_id_leading_hyphen() {
        let err = AgentProviderId::new("-claude").unwrap_err();
        assert!(matches!(err, AgentProviderIdError::LeadingHyphen));
    }

    #[test]
    fn reject_provider_id_trailing_hyphen() {
        let err = AgentProviderId::new("claude-").unwrap_err();
        assert!(matches!(err, AgentProviderIdError::TrailingHyphen));
    }

    #[test]
    fn reject_provider_id_empty() {
        let err = AgentProviderId::new("").unwrap_err();
        assert!(matches!(err, AgentProviderIdError::Empty));
    }

    #[test]
    fn reject_provider_id_too_long() {
        let long = "a".repeat(64);
        let err = AgentProviderId::new(&long).unwrap_err();
        assert!(matches!(err, AgentProviderIdError::TooLong { .. }));
    }

    #[test]
    fn reject_provider_id_with_unicode() {
        let err = AgentProviderId::new("claudé").unwrap_err();
        assert!(err.to_string().contains("invalid"));
    }

    #[test]
    fn serde_roundtrip_provider_id() {
        let id = AgentProviderId::new("claude-code").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""claude-code""#);
        let decoded: AgentProviderId = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, id);
    }

    // ── AgentProviderKey ──────────────────────────────────────────

    #[test]
    fn agent_provider_key_pair() {
        let plugin_id = PluginId::new("ora.claude-code").unwrap();
        let provider_id = AgentProviderId::new("claude-code").unwrap();
        let key = AgentProviderKey::new(plugin_id, provider_id);
        assert_eq!(key.plugin_id.as_str(), "ora.claude-code");
        assert_eq!(key.provider_id.as_str(), "claude-code");
    }

    #[test]
    fn agent_provider_key_serde_roundtrip() {
        let key = AgentProviderKey::new(
            PluginId::new("ora.claude-code").unwrap(),
            AgentProviderId::new("claude-code").unwrap(),
        );
        let json = serde_json::to_value(&key).unwrap();
        let decoded: AgentProviderKey = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.plugin_id.as_str(), "ora.claude-code");
        assert_eq!(decoded.provider_id.as_str(), "claude-code");
    }
}
