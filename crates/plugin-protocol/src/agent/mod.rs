mod dto;
mod leaf;
mod method;
mod validation;

pub use dto::*;
pub use leaf::*;
pub use method::*;
pub use validation::*;

// ── Wire-level compat constants and types ──────────────────────

// NOTE: PLUGIN_API_VERSION_V1 and AGENT_CONTRACT_VERSION_V1 are defined in
// manifest.rs. Do NOT redefine here — `pub use manifest::*;` in lib.rs exports them.

/// Wire protocol version — compile-time locked at v1.
pub const WIRE_VERSION_V1: u32 = 1;

/// Host-assigned request id with sequence-based generation.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct HostRequestId(String);

impl HostRequestId {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err("HostRequestId must be 1-128 bytes".into());
        }
        Ok(Self(value))
    }
    pub fn from_sequence(seq: u64) -> Result<Self, String> {
        Self::parse(format!("h:{seq}"))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Maximum frame payload bytes (re-exported from frame module).
pub const MAX_FRAME_BYTES: u32 = crate::frame::MAX_PAYLOAD_BYTES;

use crate::json_rpc::{JsonRpcNotification, JsonRpcRequest};

/// JSON-RPC envelope — discriminates request/response/notification.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonRpcEnvelope {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

/// JSON-RPC response discriminated by success/error for A-side pattern matching.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonRpcResponse {
    Success {
        id: HostRequestId,
        result: serde_json::Value,
        jsonrpc: String,
    },
    Error {
        id: HostRequestId,
        error: JsonRpcError,
        jsonrpc: String,
    },
}

/// JSON-RPC error type expected by A's code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// Method name constants
pub const METHOD_INITIALIZE: &str = "$/initialize";
pub const METHOD_ACTIVATE: &str = "$/activate";
pub const METHOD_DEACTIVATE: &str = "$/deactivate";
pub const METHOD_EXIT: &str = "$/exit";
pub const METHOD_CANCEL_REQUEST: &str = "$/cancelRequest";
pub const METHOD_STREAM: &str = "$/stream";

// Error code constants — re-exported from canonical json_rpc.rs definitions.
// Prefer AGENT_BUSINESS_ERROR / REQUEST_CANCELLED / SERVER_BUSY for new code.
pub const ERROR_AGENT_BUSINESS: i32 = crate::json_rpc::AGENT_BUSINESS_ERROR;
pub const ERROR_REQUEST_CANCELLED: i32 = crate::json_rpc::REQUEST_CANCELLED;
pub const ERROR_SERVER_BUSY: i32 = crate::json_rpc::SERVER_BUSY;

// ── Frame codec helpers ──────────────────────────────────────

#[cfg(test)]
use crate::frame::FrameType;
use crate::frame::{FrameError, FrameReadStage, HEADER_LEN as FRAME_HEADER_LEN, decode_header};

/// Encode a JSON-RPC request as frame payload bytes.
pub fn encode_json_rpc_request(
    id: &HostRequestId,
    method: &str,
    params: &impl serde::Serialize,
) -> Result<Vec<u8>, serde_json::Error> {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id.as_str(),
        "method": method,
        "params": params,
    });
    serde_json::to_vec(&req)
}

/// Encode a JSON-RPC notification as frame payload bytes.
pub fn encode_json_rpc_notification<P: serde::Serialize>(
    method: &str,
    params: Option<&P>,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
    });
    if let Some(p) = params {
        notif["params"] = serde_json::to_value(p)?;
    }
    serde_json::to_vec(&notif)
}

/// Count the maximum nesting depth of a JSON byte slice by scanning for
/// `{` / `[` (depth++) and `}` / `]` (depth--) tokens, ignoring content
/// inside strings. Returns the maximum depth observed, or 0 for non-JSON.
fn count_json_depth(bytes: &[u8]) -> usize {
    let mut max_depth = 0usize;
    let mut current_depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &b in bytes {
        if escaped {
            escaped = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                current_depth += 1;
                if current_depth > max_depth {
                    max_depth = current_depth;
                }
            }
            b'}' | b']' => {
                if current_depth > 0 {
                    current_depth -= 1;
                }
            }
            _ => {}
        }
    }
    max_depth
}

/// Parse a frame payload (JSON bytes without the 5-byte header) into a JSON-RPC envelope.
pub fn parse_json_rpc_frame(
    payload: &[u8],
    max_depth: usize,
) -> Result<JsonRpcEnvelope, String> {
    // Check depth before parsing to prevent stack overflow from malicious input.
    let actual_depth = count_json_depth(payload);
    if actual_depth > max_depth {
        return Err(format!(
            "JSON nesting depth {actual_depth} exceeds maximum {max_depth}"
        ));
    }

    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "not a JSON object".to_string())?;
    let has_id = obj.contains_key("id");
    let has_method = obj.contains_key("method");
    match (has_id, has_method) {
        (true, true) => {
            let request: JsonRpcRequest =
                serde_json::from_value(value).map_err(|e| format!("invalid request: {e}"))?;
            Ok(JsonRpcEnvelope::Request(request))
        }
        (true, false) => {
            let has_result = obj.contains_key("result");
            let has_error = obj.contains_key("error");
            let id_str = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let id =
                HostRequestId::parse(id_str).map_err(|e| format!("invalid response id: {e}"))?;
            let jsonrpc = obj
                .get("jsonrpc")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if has_result && !has_error {
                let result = obj
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                Ok(JsonRpcEnvelope::Response(JsonRpcResponse::Success {
                    id,
                    result,
                    jsonrpc,
                }))
            } else if has_error && !has_result {
                let ev = obj.get("error").cloned().unwrap_or(serde_json::Value::Null);
                let error = JsonRpcError {
                    code: ev.get("code").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    message: ev
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    data: ev.get("data").cloned(),
                };
                Ok(JsonRpcEnvelope::Response(JsonRpcResponse::Error {
                    id,
                    error,
                    jsonrpc,
                }))
            } else {
                Err("response must have exactly one of result or error".to_string())
            }
        }
        (false, true) => {
            let notification: JsonRpcNotification =
                serde_json::from_value(value).map_err(|e| format!("invalid notification: {e}"))?;
            Ok(JsonRpcEnvelope::Notification(notification))
        }
        _ => Err("invalid JSON-RPC message".to_string()),
    }
}

// ── Lifecycle compat aliases ──────────────────────────────────

pub use crate::lifecycle::{
    ActivateReason as ActivationReason, DeactivateReason as DeactivationReason,
};

// ── FrameDecoder compat wrapper ──────────────────────────────

pub struct FrameDecoder {
    buffer: Vec<u8>,
    max_frame_bytes: u32,
}

impl FrameDecoder {
    pub fn new(max_frame_bytes: u32) -> Self {
        Self {
            buffer: Vec::new(),
            max_frame_bytes,
        }
    }
    pub fn decode_chunk(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        self.buffer.extend_from_slice(chunk);
        let effective_max = self.max_frame_bytes.min(crate::frame::MAX_PAYLOAD_BYTES);
        let mut frames = Vec::new();
        loop {
            if self.buffer.len() < FRAME_HEADER_LEN {
                break;
            }
            let mut header = [0u8; FRAME_HEADER_LEN];
            header.copy_from_slice(&self.buffer[..FRAME_HEADER_LEN]);
            let (length, _) = decode_header(&header)?;
            if length as u32 > effective_max {
                return Err(FrameError::PayloadTooLarge {
                    length: length as u32,
                    max: effective_max,
                });
            }
            let total = FRAME_HEADER_LEN + length as usize;
            if self.buffer.len() < total {
                break;
            }
            let payload = self.buffer[FRAME_HEADER_LEN..total].to_vec();
            self.buffer.drain(..total);
            frames.push(payload);
        }
        Ok(frames)
    }
    pub fn finish(&self) -> Result<(), FrameError> {
        if self.buffer.is_empty() {
            Ok(())
        } else {
            Err(FrameError::UnexpectedEof {
                stage: FrameReadStage::Header,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameType;
    use pretty_assertions::assert_eq;

    // ── HostRequestId ─────────────────────────────────────────────

    #[test]
    fn host_request_id_from_sequence() {
        let id = HostRequestId::from_sequence(1).unwrap();
        assert_eq!(id.as_str(), "h:1");
        let id = HostRequestId::from_sequence(42).unwrap();
        assert_eq!(id.as_str(), "h:42");
    }

    #[test]
    fn host_request_id_rejects_empty() {
        assert!(HostRequestId::parse("").is_err());
    }

    #[test]
    fn host_request_id_rejects_too_long() {
        let long = "x".repeat(129);
        assert!(HostRequestId::parse(long).is_err());
    }

    #[test]
    fn host_request_id_max_length_ok() {
        let id = "x".repeat(128);
        assert!(HostRequestId::parse(id).is_ok());
    }

    // ── encode_json_rpc_request ──────────────────────────────────

    #[test]
    fn encode_request_roundtrip() {
        let id = HostRequestId::from_sequence(1).unwrap();
        let payload =
            encode_json_rpc_request(&id, "test", &serde_json::json!({"a": 1})).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Request(req) => {
                assert_eq!(req.id, "h:1");
                assert_eq!(req.method, "test");
                assert!(req.params.is_some());
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn encode_request_no_params() {
        let id = HostRequestId::from_sequence(1).unwrap();
        let payload =
            encode_json_rpc_request(&id, "ping", &serde_json::Value::Null).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Request(req) => {
                assert_eq!(req.method, "ping");
            }
            _ => panic!("expected request"),
        }
    }

    // ── encode_json_rpc_notification ─────────────────────────────

    #[test]
    fn encode_notification_without_params() {
        let payload =
            encode_json_rpc_notification::<serde_json::Value>("$/exit", None).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        assert!(matches!(envelope, JsonRpcEnvelope::Notification(_)));
    }

    #[test]
    fn encode_notification_with_params() {
        let params = serde_json::json!({"id": "h:1", "seq": 1});
        let payload =
            encode_json_rpc_notification("$/stream", Some(&params)).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Notification(n) => {
                assert_eq!(n.method, "$/stream");
                assert!(n.params.is_some());
            }
            _ => panic!("expected notification"),
        }
    }

    // ── parse_json_rpc_frame ─────────────────────────────────────

    #[test]
    fn parse_success_response() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:1",
            "result": {"ok": true}
        });
        let payload = serde_json::to_vec(&json).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Response(JsonRpcResponse::Success { id, result, .. }) => {
                assert_eq!(id.as_str(), "h:1");
                assert_eq!(result["ok"], true);
            }
            other => panic!("expected success response, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_response() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:5",
            "error": {"code": -32601, "message": "Method not found"}
        });
        let payload = serde_json::to_vec(&json).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Response(JsonRpcResponse::Error { id, error, .. }) => {
                assert_eq!(id.as_str(), "h:5");
                assert_eq!(error.code, -32601);
                assert_eq!(error.message, "Method not found");
            }
            other => panic!("expected error response, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_response_with_data() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:3",
            "error": {"code": -32000, "message": "business error", "data": {"kind": "agentUnavailable"}}
        });
        let payload = serde_json::to_vec(&json).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Response(JsonRpcResponse::Error { error, .. }) => {
                assert_eq!(error.code, -32000);
                assert!(error.data.is_some());
            }
            other => panic!("expected error response with data, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_both_result_and_error_rejected() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:1",
            "result": "ok",
            "error": {"code": -1, "message": "x"}
        });
        let payload = serde_json::to_vec(&json).unwrap();
        assert!(parse_json_rpc_frame(&payload, 64).is_err());
    }

    #[test]
    fn parse_response_neither_result_nor_error_rejected() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:1"
        });
        let payload = serde_json::to_vec(&json).unwrap();
        assert!(parse_json_rpc_frame(&payload, 64).is_err());
    }

    #[test]
    fn parse_not_json_rejected() {
        assert!(parse_json_rpc_frame(b"not json", 64).is_err());
    }

    #[test]
    fn parse_array_rejected() {
        let payload = serde_json::to_vec(&serde_json::json!([1, 2, 3])).unwrap();
        assert!(parse_json_rpc_frame(&payload, 64).is_err());
    }

    #[test]
    fn parse_request() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:1",
            "method": "test.method",
            "params": {"key": "value"}
        });
        let payload = serde_json::to_vec(&json).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        match envelope {
            JsonRpcEnvelope::Request(req) => {
                assert_eq!(req.id, "h:1");
                assert_eq!(req.method, "test.method");
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn parse_notification() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "$/stream",
            "params": {"id": "h:1", "seq": 1}
        });
        let payload = serde_json::to_vec(&json).unwrap();
        let envelope = parse_json_rpc_frame(&payload, 64).unwrap();
        assert!(matches!(envelope, JsonRpcEnvelope::Notification(_)));
    }

    // ── FrameDecoder ──────────────────────────────────────────────

    #[test]
    fn frame_decoder_single_frame() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","result":"ok"}"#;
        let mut frame = Vec::new();
        frame.extend_from_slice(&(payload.len() as i32).to_be_bytes());
        frame.push(FrameType::Response as u8);
        frame.extend_from_slice(payload);

        let mut decoder = FrameDecoder::new(8 * 1024 * 1024);
        let frames = decoder.decode_chunk(&frame).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], payload);
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn frame_decoder_partial_reads() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let mut frame = Vec::new();
        frame.extend_from_slice(&(payload.len() as i32).to_be_bytes());
        frame.push(FrameType::Request as u8);
        frame.extend_from_slice(payload);

        let mut decoder = FrameDecoder::new(8 * 1024 * 1024);
        let frames = decoder.decode_chunk(&frame[..3]).unwrap();
        assert!(frames.is_empty());
        let frames = decoder.decode_chunk(&frame[3..]).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(&frames[0], payload);
    }

    #[test]
    fn frame_decoder_multiple_frames() {
        let p1 = br#"{"jsonrpc":"2.0","id":"h:1","result":"a"}"#;
        let p2 = br#"{"jsonrpc":"2.0","id":"h:2","result":"b"}"#;
        let mut combined = Vec::new();
        for p in [p1, p2] {
            combined.extend_from_slice(&(p.len() as i32).to_be_bytes());
            combined.push(FrameType::Response as u8);
            combined.extend_from_slice(p);
        }
        let mut decoder = FrameDecoder::new(8 * 1024 * 1024);
        let frames = decoder.decode_chunk(&combined).unwrap();
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn frame_decoder_finish_with_partial_data_err() {
        let mut decoder = FrameDecoder::new(8 * 1024 * 1024);
        decoder.decode_chunk(&[0x00, 0x00, 0x00]).unwrap();
        assert!(decoder.finish().is_err());
    }
}
