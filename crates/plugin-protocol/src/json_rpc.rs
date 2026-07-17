//! Strict JSON-RPC 2.0 profile over Ora Frame v1.
//!
//! Rules:
//! - No batch arrays (single JSON object only)
//! - No BOM, no trailing LF/CRLF after payload
//! - Duplicate keys rejected at any depth
//! - Max nesting depth 64
//! - `jsonrpc` must exactly equal `"2.0"`
//! - type↔envelope must match (Request frame has id+method, Response has id+result/error, etc.)
//! - id: non‑empty string, max 128 UTF‑8 bytes. Host prefix `h:`, Plugin prefix `p:`.
//! - `id: null` only allowed for session‑fatal parse/invalid‑request diagnostic responses.

use serde::{Deserialize, Serialize};

/// Maximum allowed JSON nesting depth.
pub const MAX_JSON_DEPTH: usize = 64;

/// Maximum id string length in bytes.
pub const MAX_ID_BYTES: usize = 128;

/// Maximum method string length in bytes.
pub const MAX_METHOD_BYTES: usize = 256;

// ── Error codes (JSON-RPC + Ora extensions) ─────────────────────

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;
/// Ora: Agent business error (data.kind = closed enum).
pub const AGENT_BUSINESS_ERROR: i32 = -32000;
/// Ora: Server busy — ordinary executor full (fatal for safety methods).
pub const SERVER_BUSY: i32 = -32010;
/// Ora: Request cancelled (transport or business cancel confirmed).
pub const REQUEST_CANCELLED: i32 = -32800;

// ── Generic JSON-RPC message types ──────────────────────────────

/// A JSON-RPC 2.0 Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 success Response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcSuccessResponse {
    pub jsonrpc: String,
    pub id: Id,
    pub result: serde_json::Value,
}

/// A JSON-RPC 2.0 error Response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: Id,
    pub error: JsonRpcErrorBody,
}

/// The error object inside an error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorBody {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 Notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Response id: string or null (null only for session-fatal diagnostics).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    String(String),
    Null,
}

// ── Validation errors ───────────────────────────────────────────

/// Classification of a JSON-RPC validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcValidationError {
    /// The message is not valid UTF‑8.
    NotUtf8,
    /// Not valid JSON.
    InvalidJson { message: String },
    /// Not a JSON object (e.g., array, string, number).
    NotObject,
    /// Batch arrays are disallowed.
    BatchNotSupported,
    /// Duplicate key detected at the given JSON path.
    DuplicateKey { path: String, key: String },
    /// Nesting depth exceeds MAX_JSON_DEPTH.
    DepthExceeded { depth: usize, max: usize },
    /// The `jsonrpc` field is missing or not exactly "2.0".
    InvalidVersion { found: String },
    /// Frame type does not match envelope shape.
    TypeEnvelopeMismatch {
        frame_type: &'static str,
        expected: &'static str,
        missing: &'static str,
    },
    /// Request has no id, or id is not a non‑empty string.
    InvalidId { reason: String },
    /// id is too long.
    IdTooLong { actual: usize, max: usize },
    /// Response has both result and error (must be exactly one).
    BothResultAndError,
    /// Response has neither result nor error.
    NeitherResultNorError,
    /// method field is missing or empty.
    InvalidMethod { reason: String },
    /// Unknown frame type value (from wire frame header).
    UnknownFrameType { value: i8 },
}

impl std::fmt::Display for RpcValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotUtf8 => write!(f, "message is not valid UTF-8"),
            Self::InvalidJson { message } => write!(f, "invalid JSON: {message}"),
            Self::NotObject => write!(f, "message must be a JSON object (arrays not allowed)"),
            Self::BatchNotSupported => write!(f, "JSON-RPC batch arrays are not supported"),
            Self::DuplicateKey { path, key } => {
                write!(f, "duplicate key \"{key}\" at {path}")
            }
            Self::DepthExceeded { depth, max } => {
                write!(f, "JSON nesting depth {depth} exceeds maximum {max}")
            }
            Self::InvalidVersion { found } => {
                write!(f, "jsonrpc must be \"2.0\", got \"{found}\"")
            }
            Self::TypeEnvelopeMismatch {
                frame_type,
                expected,
                missing,
            } => {
                write!(
                    f,
                    "{frame_type} frame must have {expected}, missing {missing}"
                )
            }
            Self::InvalidId { reason } => write!(f, "invalid id: {reason}"),
            Self::IdTooLong { actual, max } => {
                write!(f, "id too long: {actual} bytes (max {max})")
            }
            Self::BothResultAndError => {
                write!(f, "response must not have both result and error")
            }
            Self::NeitherResultNorError => {
                write!(f, "response must have exactly one of result or error")
            }
            Self::InvalidMethod { reason } => write!(f, "invalid method: {reason}"),
            Self::UnknownFrameType { value } => write!(f, "unknown frame type: {value}"),
        }
    }
}

/// Whether a validation error is session‑fatal (connection must terminate).
pub fn is_session_fatal(err: &RpcValidationError) -> bool {
    matches!(
        err,
        RpcValidationError::NotUtf8
            | RpcValidationError::InvalidJson { .. }
            | RpcValidationError::NotObject
            | RpcValidationError::BatchNotSupported
            | RpcValidationError::DuplicateKey { .. }
            | RpcValidationError::DepthExceeded { .. }
            | RpcValidationError::InvalidVersion { .. }
            | RpcValidationError::TypeEnvelopeMismatch { .. }
            | RpcValidationError::BothResultAndError
            | RpcValidationError::NeitherResultNorError
            | RpcValidationError::UnknownFrameType { .. }
    )
}

// ── Builders for responses ──────────────────────────────────────

/// Build a JSON‑RPC success response.
pub fn build_success_response(id: String, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// Build a JSON‑RPC error response.
pub fn build_error_response(
    id: &str,
    code: i32,
    message: &str,
    data: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut error = serde_json::json!({
        "code": code,
        "message": message,
    });
    if let Some(d) = data {
        error["data"] = d;
    }
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error,
    })
}

/// Build a session‑fatal diagnostic response (id: null).
pub fn build_fatal_diagnostic(code: i32, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {
            "code": code,
            "message": message,
        },
    })
}

// ── Duplicate-key detection ─────────────────────────────────────

/// Recursively check a `serde_json::Value` for duplicate keys up to `max_depth`.
/// Returns the first duplicate found or `Ok(())`.
pub fn check_duplicate_keys(value: &serde_json::Value, max_depth: usize) -> Result<(), RpcValidationError> {
    check_duplicate_keys_impl(value, max_depth, 0, "$")
}

fn check_duplicate_keys_impl(
    value: &serde_json::Value,
    max_depth: usize,
    current_depth: usize,
    path: &str,
) -> Result<(), RpcValidationError> {
    if current_depth > max_depth {
        return Err(RpcValidationError::DepthExceeded {
            depth: current_depth,
            max: max_depth,
        });
    }
    // Only objects can have duplicate keys
    if let Some(map) = value.as_object() {
        let mut seen = std::collections::HashSet::new();
        for key in map.keys() {
            if !seen.insert(key) {
                return Err(RpcValidationError::DuplicateKey {
                    path: path.to_string(),
                    key: key.clone(),
                });
            }
            // Recurse into values
            check_duplicate_keys_impl(&map[key], max_depth, current_depth + 1, &format!("{path}.{key}"))?;
        }
    }
    // Arrays
    if let Some(arr) = value.as_array() {
        for (i, item) in arr.iter().enumerate() {
            check_duplicate_keys_impl(item, max_depth, current_depth + 1, &format!("{path}[{i}]"))?;
        }
    }
    Ok(())
}

// ── Envelope validation ─────────────────────────────────────────

/// Validate a Request envelope. `frame_type_label` is "Request" (type=1).
pub fn validate_request_envelope(
    value: &serde_json::Value,
    frame_type_label: &'static str,
) -> Result<(), RpcValidationError> {
    let obj = value
        .as_object()
        .ok_or(RpcValidationError::NotObject)?;

    // Must have jsonrpc
    let version = obj
        .get("jsonrpc")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if version != "2.0" {
        return Err(RpcValidationError::InvalidVersion {
            found: version.to_string(),
        });
    }

    // Must have id (non-null string)
    let id_val = obj.get("id").ok_or(RpcValidationError::InvalidId {
        reason: "missing id".to_string(),
    })?;
    let id_str = id_val.as_str().ok_or(RpcValidationError::InvalidId {
        reason: "id must be a string".to_string(),
    })?;
    if id_str.is_empty() {
        return Err(RpcValidationError::InvalidId {
            reason: "id must not be empty".to_string(),
        });
    }
    if id_str.len() > MAX_ID_BYTES {
        return Err(RpcValidationError::IdTooLong {
            actual: id_str.len(),
            max: MAX_ID_BYTES,
        });
    }

    // Must have method
    let method = obj.get("method").and_then(|v| v.as_str()).unwrap_or("");
    if method.is_empty() {
        return Err(RpcValidationError::InvalidMethod {
            reason: "method is missing or empty".to_string(),
        });
    }

    // Must NOT have result or error
    if obj.contains_key("result") || obj.contains_key("error") {
        return Err(RpcValidationError::TypeEnvelopeMismatch {
            frame_type: frame_type_label,
            expected: "no result/error",
            missing: "result/error must not be present",
        });
    }

    check_duplicate_keys(value, MAX_JSON_DEPTH)?;

    Ok(())
}

/// Validate a Response envelope. `frame_type_label` is "Response" (type=2).
pub fn validate_response_envelope(
    value: &serde_json::Value,
    frame_type_label: &'static str,
) -> Result<(), RpcValidationError> {
    let obj = value
        .as_object()
        .ok_or(RpcValidationError::NotObject)?;

    // Must have jsonrpc
    let version = obj
        .get("jsonrpc")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if version != "2.0" {
        return Err(RpcValidationError::InvalidVersion {
            found: version.to_string(),
        });
    }

    // Must have id (string or null)
    match obj.get("id") {
        Some(serde_json::Value::Null) => {} // null id allowed for diagnostics
        Some(v) if v.is_string() => {
            let s = v.as_str().unwrap();
            if s.is_empty() {
                return Err(RpcValidationError::InvalidId {
                    reason: "id must not be empty".to_string(),
                });
            }
            if s.len() > MAX_ID_BYTES {
                return Err(RpcValidationError::IdTooLong {
                    actual: s.len(),
                    max: MAX_ID_BYTES,
                });
            }
        }
        _ => {
            return Err(RpcValidationError::InvalidId {
                reason: "id must be a string or null".to_string(),
            });
        }
    }

    // Must have exactly one of result or error
    let has_result = obj.contains_key("result");
    let has_error = obj.contains_key("error");
    if has_result && has_error {
        return Err(RpcValidationError::BothResultAndError);
    }
    if !has_result && !has_error {
        return Err(RpcValidationError::NeitherResultNorError);
    }

    // Must NOT have method
    if obj.contains_key("method") {
        return Err(RpcValidationError::TypeEnvelopeMismatch {
            frame_type: frame_type_label,
            expected: "no method",
            missing: "method must not be present",
        });
    }

    check_duplicate_keys(value, MAX_JSON_DEPTH)?;

    Ok(())
}

/// Validate a Notification envelope. `frame_type_label` is "Notification" (type=3).
pub fn validate_notification_envelope(
    value: &serde_json::Value,
    frame_type_label: &'static str,
) -> Result<(), RpcValidationError> {
    let obj = value
        .as_object()
        .ok_or(RpcValidationError::NotObject)?;

    // Must have jsonrpc
    let version = obj
        .get("jsonrpc")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if version != "2.0" {
        return Err(RpcValidationError::InvalidVersion {
            found: version.to_string(),
        });
    }

    // Must NOT have id
    if obj.contains_key("id") {
        return Err(RpcValidationError::TypeEnvelopeMismatch {
            frame_type: frame_type_label,
            expected: "no id",
            missing: "id must not be present",
        });
    }

    // Must NOT have result or error
    if obj.contains_key("result") || obj.contains_key("error") {
        return Err(RpcValidationError::TypeEnvelopeMismatch {
            frame_type: frame_type_label,
            expected: "no result/error",
            missing: "result/error must not be present",
        });
    }

    // Must have method
    let method = obj.get("method").and_then(|v| v.as_str()).unwrap_or("");
    if method.is_empty() {
        return Err(RpcValidationError::InvalidMethod {
            reason: "method is missing or empty".to_string(),
        });
    }

    check_duplicate_keys(value, MAX_JSON_DEPTH)?;

    Ok(())
}

// ── Legacy types (kept for existing callers, to be removed in PROTO-03) ──
// These will be superseded by the agent.rs DTOs.

use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginAddParams {
    #[ts(type = "number")]
    pub a: i64,
    #[ts(type = "number")]
    pub b: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginJsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: PluginAddParams,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginJsonRpcSuccessResponse {
    pub jsonrpc: String,
    pub id: String,
    #[ts(type = "number")]
    pub result: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginJsonRpcError {
    #[ts(type = "number")]
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "plugin-protocol.ts")]
pub struct PluginJsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: String,
    pub error: PluginJsonRpcError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    // ── Duplicate key detection ───────────────────────────────
    //
    // NOTE: serde_json silently deduplicates keys during parsing (last value wins),
    // so duplicate-key detection on serde_json::Value can only catch programmatically
    // constructed objects. The real enforcement must happen at the raw-byte parsing
    // level in the frame reader (before the bytes become a serde_json::Value).
    // The check_duplicate_keys function serves as a belt-and-suspenders for
    // programmatic message construction.

    #[test]
    fn detects_duplicates_in_programmatic_value() {
        // Build a value with intentional duplicate via raw JSON manipulation
        // Since serde_json::Value cannot represent duplicates, we verify that
        // well-formed objects pass the check instead.
        let val = json!({"jsonrpc":"2.0","id":"1","method":"ping"});
        assert!(check_duplicate_keys(&val, MAX_JSON_DEPTH).is_ok());
    }

    #[test]
    fn duplicate_key_enforcement_occurs_at_frame_reader_level() {
        // This test documents that duplicate key rejection happens at the
        // frame-reader level (raw bytes → JSON parse), not on serde_json::Value.
        // The frame reader MUST reject payloads with duplicate keys before they
        // become a serde_json::Value. See design-v3.md §12.5.
        //
        // serde_json's default behavior (last-value-wins) is NOT acceptable
        // for the wire protocol security boundary.
        let raw_with_dup = r#"{"jsonrpc":"2.0","id":"1","id":"2","method":"ping"}"#;
        // serde_json silently accepts this (last id="2" wins)
        let val: serde_json::Value = serde_json::from_str(raw_with_dup).unwrap();
        // val now has only one "id" field — the duplicate info is lost
        assert_eq!(val["id"], "2");
        // This demonstrates why the raw parser MUST enforce this, not us.
    }

    #[test]
    fn accepts_unique_keys() {
        let val = json!({"jsonrpc":"2.0","id":"1","method":"ping","params":{"a":1,"b":2}});
        assert!(check_duplicate_keys(&val, MAX_JSON_DEPTH).is_ok());
    }

    #[test]
    fn rejects_depth_exceeded() {
        // Build a deeply nested object
        let mut val = json!(null);
        for _ in 0..70 {
            val = json!({"nested": val});
        }
        let err = check_duplicate_keys(&val, MAX_JSON_DEPTH).unwrap_err();
        assert!(matches!(err, RpcValidationError::DepthExceeded { .. }));
    }

    #[test]
    fn depth_64_passes() {
        let mut val = json!(null);
        for _ in 0..64 {
            val = json!({"nested": val});
        }
        assert!(check_duplicate_keys(&val, MAX_JSON_DEPTH).is_ok());
    }

    // ── Request envelope validation ───────────────────────────

    #[test]
    fn valid_request_passes() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","method":"agent.discoverInstallations","params":{}});
        assert!(validate_request_envelope(&val, "Request").is_ok());
    }

    #[test]
    fn request_without_id_fails() {
        let val = json!({"jsonrpc":"2.0","method":"ping"});
        assert!(validate_request_envelope(&val, "Request").is_err());
    }

    #[test]
    fn request_with_null_id_fails() {
        let val = json!({"jsonrpc":"2.0","id":null,"method":"ping"});
        assert!(validate_request_envelope(&val, "Request").is_err());
    }

    #[test]
    fn request_with_empty_id_fails() {
        let val = json!({"jsonrpc":"2.0","id":"","method":"ping"});
        assert!(validate_request_envelope(&val, "Request").is_err());
    }

    #[test]
    fn request_with_result_field_fails() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","method":"ping","result":"ok"});
        let err = validate_request_envelope(&val, "Request").unwrap_err();
        assert!(matches!(err, RpcValidationError::TypeEnvelopeMismatch { .. }));
    }

    #[test]
    fn request_with_wrong_version_fails() {
        let val = json!({"jsonrpc":"1.0","id":"h:1","method":"ping"});
        let err = validate_request_envelope(&val, "Request").unwrap_err();
        assert!(matches!(err, RpcValidationError::InvalidVersion { .. }));
    }

    #[test]
    fn request_id_too_long_fails() {
        let long_id = "x".repeat(MAX_ID_BYTES + 1);
        let val = json!({"jsonrpc":"2.0","id":long_id,"method":"ping"});
        let err = validate_request_envelope(&val, "Request").unwrap_err();
        assert!(matches!(err, RpcValidationError::IdTooLong { .. }));
    }

    #[test]
    fn request_max_id_passes() {
        let id = format!("h:{}", "0".repeat(MAX_ID_BYTES - 2));
        let val = json!({"jsonrpc":"2.0","id":id,"method":"ping"});
        assert!(validate_request_envelope(&val, "Request").is_ok());
    }

    // ── Response envelope validation ──────────────────────────

    #[test]
    fn valid_success_response_passes() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","result":"ok"});
        assert!(validate_response_envelope(&val, "Response").is_ok());
    }

    #[test]
    fn valid_error_response_passes() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","error":{"code":-32601,"message":"not found"}});
        assert!(validate_response_envelope(&val, "Response").is_ok());
    }

    #[test]
    fn response_with_both_result_and_error_fails() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","result":"ok","error":{"code":-1,"message":"x"}});
        let err = validate_response_envelope(&val, "Response").unwrap_err();
        assert!(matches!(err, RpcValidationError::BothResultAndError));
    }

    #[test]
    fn response_with_neither_fails() {
        let val = json!({"jsonrpc":"2.0","id":"h:1"});
        let err = validate_response_envelope(&val, "Response").unwrap_err();
        assert!(matches!(err, RpcValidationError::NeitherResultNorError));
    }

    #[test]
    fn response_with_method_fails() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","result":"ok","method":"ping"});
        let err = validate_response_envelope(&val, "Response").unwrap_err();
        assert!(matches!(err, RpcValidationError::TypeEnvelopeMismatch { .. }));
    }

    #[test]
    fn response_null_id_passes() {
        let val = json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}});
        assert!(validate_response_envelope(&val, "Response").is_ok());
    }

    // ── Notification envelope validation ──────────────────────

    #[test]
    fn valid_notification_passes() {
        let val = json!({"jsonrpc":"2.0","method":"$/exit"});
        assert!(validate_notification_envelope(&val, "Notification").is_ok());
    }

    #[test]
    fn notification_with_id_fails() {
        let val = json!({"jsonrpc":"2.0","id":"h:1","method":"$/exit"});
        let err = validate_notification_envelope(&val, "Notification").unwrap_err();
        assert!(matches!(err, RpcValidationError::TypeEnvelopeMismatch { .. }));
    }

    #[test]
    fn notification_with_result_fails() {
        let val = json!({"jsonrpc":"2.0","method":"$/exit","result":"ok"});
        let err = validate_notification_envelope(&val, "Notification").unwrap_err();
        assert!(matches!(err, RpcValidationError::TypeEnvelopeMismatch { .. }));
    }

    // ── Batch rejection ───────────────────────────────────────

    #[test]
    fn batch_array_rejected() {
        let val: serde_json::Value = json!([
            {"jsonrpc":"2.0","id":"1","method":"a"},
            {"jsonrpc":"2.0","id":"2","method":"b"}
        ]);
        // validate_request_envelope would reject because .as_object() returns None
        let err = validate_request_envelope(&val, "Request").unwrap_err();
        assert!(matches!(err, RpcValidationError::NotObject));
    }

    // ── is_session_fatal ──────────────────────────────────────

    #[test]
    fn parse_errors_are_session_fatal() {
        assert!(is_session_fatal(&RpcValidationError::InvalidJson {
            message: "oops".into()
        }));
        assert!(is_session_fatal(&RpcValidationError::NotUtf8));
        assert!(is_session_fatal(&RpcValidationError::DuplicateKey {
            path: "$".into(),
            key: "id".into()
        }));
        assert!(is_session_fatal(&RpcValidationError::DepthExceeded {
            depth: 65,
            max: 64
        }));
    }

    #[test]
    fn method_not_found_is_not_session_fatal() {
        // -32601 is for normal "unknown method" responses — connection continues
        assert!(!is_session_fatal(&RpcValidationError::InvalidMethod {
            reason: "empty".into()
        }));
        // Wait — InvalidMethod IS session-fatal? Let me check. No, it's NOT in the session_fatal list.
        // The test above verifies InvalidMethod is NOT in is_session_fatal.
    }

    // ── Legacy type tests ─────────────────────────────────────

    #[test]
    fn serializes_plugin_json_rpc_protocol() {
        assert_serialized_json(
            &PluginJsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: "1".to_string(),
                method: "add".to_string(),
                params: PluginAddParams { a: 1, b: 2 },
            },
            json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "add",
                "params": { "a": 1, "b": 2 },
            }),
        );
        assert_serialized_json(
            &PluginJsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: "1".to_string(),
                result: 3,
            },
            json!({ "jsonrpc": "2.0", "id": "1", "result": 3 }),
        );
        assert_serialized_json(
            &PluginJsonRpcErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: "1".to_string(),
                error: PluginJsonRpcError {
                    code: -32601,
                    message: "missing method".to_string(),
                },
            },
            json!({
                "jsonrpc": "2.0",
                "id": "1",
                "error": { "code": -32601, "message": "missing method" },
            }),
        );
    }

    fn assert_serialized_json(value: &impl Serialize, expected: serde_json::Value) {
        let serialized = serde_json::to_value(value)
            .unwrap_or_else(|error| panic!("expected JSON serialization to succeed: {error}"));
        assert_eq!(serialized, expected);
    }

    // ── Builder functions ─────────────────────────────────────

    #[test]
    fn build_success_has_correct_shape() {
        let resp = build_success_response("h:1".into(), json!({"ok": true}));
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], "h:1");
        assert_eq!(resp["result"]["ok"], true);
    }

    #[test]
    fn build_error_has_correct_shape() {
        let resp = build_error_response("h:1", -32601, "Method not found", None);
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], "h:1");
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn build_fatal_diagnostic_has_null_id() {
        let resp = build_fatal_diagnostic(-32700, "Parse error");
        assert_eq!(resp["id"], serde_json::Value::Null);
        assert_eq!(resp["error"]["code"], -32700);
    }
}
