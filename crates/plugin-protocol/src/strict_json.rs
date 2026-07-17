//! Minimal strict JSON parser — restored for A-side manifest.rs compatibility.
//! Full validation (duplicate keys, depth, batch rejection) is in json_rpc.rs.

/// Error from strict JSON parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StrictJsonError {
    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(String),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error("JSON nesting depth {depth} exceeds maximum {max}")]
    DepthExceeded { depth: usize, max: usize },
    #[error("duplicate key: {key}")]
    DuplicateKey { key: String },
    #[error("JSON must be a single object (arrays not allowed)")]
    NotObject,
}

/// Parse bytes as a strict JSON value with depth checking.
pub fn parse_strict_json(
    bytes: &[u8],
    max_depth: usize,
) -> Result<serde_json::Value, StrictJsonError> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| StrictJsonError::InvalidUtf8(e.to_string()))?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| StrictJsonError::Json(e.to_string()))?;
    check_depth(&value, max_depth, 0)?;
    if !value.is_object() {
        return Err(StrictJsonError::NotObject);
    }
    Ok(value)
}

fn check_depth(
    value: &serde_json::Value,
    max: usize,
    current: usize,
) -> Result<(), StrictJsonError> {
    if current > max {
        return Err(StrictJsonError::DepthExceeded {
            depth: current,
            max,
        });
    }
    match value {
        serde_json::Value::Object(map) => {
            for v in map.values() {
                check_depth(v, max, current + 1)?;
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                check_depth(v, max, current + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
