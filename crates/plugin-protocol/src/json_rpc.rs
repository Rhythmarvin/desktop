use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// A validated JSON-RPC 2.0 request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: JsonRpcVersion,
    pub id: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A validated JSON-RPC 2.0 response (success or error).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcResponse {
    Success {
        jsonrpc: JsonRpcVersion,
        id: String,
        result: serde_json::Value,
    },
    Error {
        jsonrpc: JsonRpcVersion,
        id: String,
        error: RpcError,
    },
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A validated JSON-RPC 2.0 notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: JsonRpcVersion,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// The only accepted JSON-RPC version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsonRpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

/// All JSON-RPC envelope variants that can arrive on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundEnvelope {
    Request(RpcRequest),
    Response(RpcResponse),
    Notification(RpcNotification),
}

/// Well-known JSON-RPC 2.0 error codes.
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const REQUEST_CANCELLED: i32 = -32800;
}

impl RpcRequest {
    /// Validates that the request ID follows Host/Plugin naming conventions.
    pub fn validate_id(&self) -> Result<(), String> {
        validate_request_id(&self.id)
    }
}

impl RpcResponse {
    pub fn success(id: impl Into<String>, result: serde_json::Value) -> Self {
        Self::Success {
            jsonrpc: JsonRpcVersion::V2,
            id: id.into(),
            result,
        }
    }

    pub fn error(id: impl Into<String>, code: i32, message: impl Into<String>) -> Self {
        Self::Error {
            jsonrpc: JsonRpcVersion::V2,
            id: id.into(),
            error: RpcError {
                code,
                message: message.into(),
                data: None,
            },
        }
    }
}

impl RpcNotification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JsonRpcVersion::V2,
            method: method.into(),
            params,
        }
    }
}

/// Parses a raw JSON payload into the correct envelope type, validated
/// against the declared frame type.
pub fn parse_inbound_envelope(
    payload: &[u8],
    declared_type: super::frame::FrameType,
    max_depth: usize,
) -> Result<InboundEnvelope, EnvelopeError> {
    // Check for BOM
    if payload.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(EnvelopeError::InvalidJson("BOM is not allowed".into()));
    }

    // Check for duplicate keys before parsing (serde_json silently keeps last value)
    check_duplicate_keys_in_json(payload, max_depth)?;

    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|e| EnvelopeError::InvalidJson(e.to_string()))?;

    let obj = value
        .as_object()
        .ok_or(EnvelopeError::NotAnObject)?;

    let version = obj
        .get("jsonrpc")
        .and_then(|v| v.as_str())
        .ok_or(EnvelopeError::MissingJsonRpc)?;
    if version != "2.0" {
        return Err(EnvelopeError::InvalidVersion(version.to_owned()));
    }

    let has_id = obj.contains_key("id");
    let has_method = obj.contains_key("method");
    let has_result = obj.contains_key("result");
    let has_error = obj.contains_key("error");

    match declared_type {
        super::frame::FrameType::Request => {
            if !has_id || !has_method {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Request",
                    reason: "must have id and method".into(),
                });
            }
            if has_result || has_error {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Request",
                    reason: "must not have result or error".into(),
                });
            }
            let request: RpcRequest = serde_json::from_value(value)
                .map_err(|e| EnvelopeError::InvalidJson(e.to_string()))?;
            validate_request_id(&request.id)?;
            Ok(InboundEnvelope::Request(request))
        }
        super::frame::FrameType::Response => {
            if !has_id {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Response",
                    reason: "must have id".into(),
                });
            }
            if has_method {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Response",
                    reason: "must not have method".into(),
                });
            }
            if has_result == has_error {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Response",
                    reason: "must have exactly one of result or error".into(),
                });
            }
            let response: RpcResponse = serde_json::from_value(value)
                .map_err(|e| EnvelopeError::InvalidJson(e.to_string()))?;
            Ok(InboundEnvelope::Response(response))
        }
        super::frame::FrameType::Notification => {
            if !has_method {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Notification",
                    reason: "must have method".into(),
                });
            }
            if has_id || has_result || has_error {
                return Err(EnvelopeError::TypeMismatch {
                    declared: "Notification",
                    reason: "must not have id, result, or error".into(),
                });
            }
            let notification: RpcNotification = serde_json::from_value(value)
                .map_err(|e| EnvelopeError::InvalidJson(e.to_string()))?;
            Ok(InboundEnvelope::Notification(notification))
        }
    }
}

/// Parse a payload as a `RpcResponse` directly (for the caller side).
pub fn parse_response(payload: &[u8]) -> Result<RpcResponse, EnvelopeError> {
    serde_json::from_slice::<RpcResponse>(payload)
        .map_err(|e| EnvelopeError::InvalidJson(e.to_string()))
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum EnvelopeError {
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    #[error("top-level value must be a JSON object")]
    NotAnObject,
    #[error("missing jsonrpc field")]
    MissingJsonRpc,
    #[error("invalid jsonrpc version: {0}")]
    InvalidVersion(String),
    #[error("duplicate key: {0}")]
    DuplicateKey(String),
    #[error("nesting depth {0} exceeds maximum {1}")]
    DepthExceeded(usize, usize),
    #[error("type mismatch: declared {declared}, {reason}")]
    TypeMismatch {
        declared: &'static str,
        reason: Cow<'static, str>,
    },
    #[error("invalid request id: {0}")]
    InvalidId(String),
}

impl From<String> for EnvelopeError {
    fn from(s: String) -> Self {
        EnvelopeError::InvalidId(s)
    }
}

fn validate_request_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("id must be non-empty".into());
    }
    if id.len() > 128 {
        return Err("id must be <= 128 UTF-8 bytes".into());
    }
    if id.contains('\0') {
        return Err("id must not contain NUL".into());
    }
    Ok(())
}

/// Scans raw JSON bytes for duplicate object keys and depth violations.
/// serde_json silently keeps the last value for duplicate keys, so we must
/// check at the byte level before parsing.
fn check_duplicate_keys_in_json(payload: &[u8], max_depth: usize) -> Result<(), EnvelopeError> {
    let bytes = std::str::from_utf8(payload)
        .map_err(|_| EnvelopeError::InvalidJson("invalid UTF-8".into()))?;

    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut after_colon = false; // true when we expect a value, false when we expect a key
    let mut brace_stack: Vec<std::collections::BTreeSet<String>> = Vec::new();
    let mut current_key = String::new();

    for ch in bytes.chars() {
        if escape {
            if in_string && after_colon == false {
                current_key.push(ch);
            }
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            if !in_string {
                // Just finished a string
                if let Some(keys) = brace_stack.last_mut() {
                    if !after_colon && !current_key.is_empty() {
                        // This was a key
                        if !keys.insert(current_key.clone()) {
                            return Err(EnvelopeError::DuplicateKey(current_key));
                        }
                        current_key.clear();
                    }
                }
            }
            continue;
        }
        if in_string {
            if !after_colon && brace_stack.last().is_some() {
                current_key.push(ch);
            }
            continue;
        }
        match ch {
            '{' => {
                depth += 1;
                if depth > max_depth {
                    return Err(EnvelopeError::DepthExceeded(depth, max_depth));
                }
                brace_stack.push(std::collections::BTreeSet::new());
                after_colon = false;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                brace_stack.pop();
                after_colon = false;
            }
            ':' => {
                after_colon = true;
            }
            ',' => {
                after_colon = false;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameType;

    #[test]
    fn parses_valid_request() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let result = parse_inbound_envelope(payload, FrameType::Request, 64).unwrap();
        match result {
            InboundEnvelope::Request(req) => {
                assert_eq!(req.id, "h:1");
                assert_eq!(req.method, "ping");
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn parses_valid_notification() {
        let payload = br#"{"jsonrpc":"2.0","method":"$/hello"}"#;
        let result = parse_inbound_envelope(payload, FrameType::Notification, 64).unwrap();
        match result {
            InboundEnvelope::Notification(n) => assert_eq!(n.method, "$/hello"),
            _ => panic!("expected Notification"),
        }
    }

    #[test]
    fn rejects_type_mismatch() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        // Declared as Notification but payload is a Request
        let err = parse_inbound_envelope(payload, FrameType::Notification, 64).unwrap_err();
        assert!(matches!(err, EnvelopeError::TypeMismatch { .. }));
    }

    #[test]
    fn rejects_duplicate_keys() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","id":"h:2","method":"ping"}"#;
        let _err = parse_inbound_envelope(payload, FrameType::Request, 64).unwrap_err();
    }

    #[test]
    fn rejects_invalid_request_id() {
        let payload = br#"{"jsonrpc":"2.0","id":"","method":"ping"}"#;
        let err = parse_inbound_envelope(payload, FrameType::Request, 64).unwrap_err();
        assert!(matches!(err, EnvelopeError::InvalidId(_)));
    }
}