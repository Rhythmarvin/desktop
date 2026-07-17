//! Runtime-aware 5-byte frame encode/decode.
//!
//! Delegates to `ora_plugin_protocol::frame` for the core encoding logic
//! and adds runtime-specific error types and frame-read-stage tracking.

use ora_plugin_protocol::frame as proto;

pub use proto::{FrameType, HEADER_LEN, MAX_PAYLOAD_BYTES};

/// Errors from runtime frame encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Payload length is zero or negative.
    InvalidLength { length: i32 },
    /// Payload exceeds MAX_PAYLOAD_BYTES.
    PayloadTooLarge { length: u32, max: u32 },
    /// Unknown frame type value.
    UnknownFrameType { value: i8 },
    /// Unexpected EOF during header or payload read.
    UnexpectedEof { stage: FrameReadStage },
    /// Payload is not valid UTF-8.
    InvalidUtf8,
    /// Encoder: payload is empty.
    EmptyPayload,
    /// JSON-RPC envelope validation failed.
    EnvelopeError { message: String },
}

/// Which part of a frame was being read when EOF occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReadStage {
    /// Reading the 5-byte header.
    Header,
    /// Reading the type byte.
    Type,
    /// Reading the payload (expected bytes, actual bytes read).
    Payload { expected: u32, read: u32 },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLength { length } => write!(f, "invalid frame length: {length}"),
            Self::PayloadTooLarge { length, max } => {
                write!(f, "payload too large: {length} bytes (max {max})")
            }
            Self::UnknownFrameType { value } => write!(f, "unknown frame type: {value}"),
            Self::UnexpectedEof { stage } => match stage {
                FrameReadStage::Header => write!(f, "unexpected EOF in frame header"),
                FrameReadStage::Type => write!(f, "unexpected EOF at frame type byte"),
                FrameReadStage::Payload { expected, read } => write!(
                    f,
                    "unexpected EOF in payload: expected {expected} bytes, got {read}"
                ),
            },
            Self::InvalidUtf8 => write!(f, "frame payload is not valid UTF-8"),
            Self::EmptyPayload => write!(f, "frame payload must not be empty"),
            Self::EnvelopeError { message } => write!(f, "JSON-RPC envelope error: {message}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<proto::FrameError> for FrameError {
    fn from(e: proto::FrameError) -> Self {
        match e {
            proto::FrameError::InvalidLength { length } => Self::InvalidLength { length },
            proto::FrameError::PayloadTooLarge { length, max } => {
                Self::PayloadTooLarge { length, max }
            }
            proto::FrameError::UnknownFrameType { value } => Self::UnknownFrameType { value },
            proto::FrameError::UnexpectedEof { stage } => {
                let stage = match stage {
                    proto::FrameReadStage::Header => FrameReadStage::Header,
                    proto::FrameReadStage::Type => FrameReadStage::Type,
                    proto::FrameReadStage::Payload { expected, read } => {
                        FrameReadStage::Payload { expected, read }
                    }
                };
                Self::UnexpectedEof { stage }
            }
            proto::FrameError::InvalidUtf8 => Self::InvalidUtf8,
            proto::FrameError::EmptyPayload => Self::EmptyPayload,
        }
    }
}

/// Encode a complete frame: 5-byte header + payload bytes.
///
/// Validates: payload non-empty, ≤ MAX_PAYLOAD_BYTES, valid frame type.
pub fn encode_frame(frame_type: FrameType, payload_str: &str) -> Result<Vec<u8>, FrameError> {
    Ok(proto::encode_frame(
        frame_type,
        payload_str.as_bytes(),
        MAX_PAYLOAD_BYTES,
    )?)
}

/// Decode a frame header from 5 bytes. Returns (payload_length, frame_type).
///
/// Validates: length > 0, ≤ MAX_PAYLOAD_BYTES, valid frame type.
pub fn decode_header(header: &[u8; HEADER_LEN]) -> Result<(i32, FrameType), FrameError> {
    Ok(proto::decode_header(header)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn encode_request_round_trip() {
        let payload = r#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let encoded = encode_frame(FrameType::Request, payload).unwrap();
        assert_eq!(encoded.len(), HEADER_LEN + payload.as_bytes().len());

        let (len, ft) = decode_header(encoded[..HEADER_LEN].try_into().unwrap()).unwrap();
        assert_eq!(len, payload.as_bytes().len() as i32);
        assert_eq!(ft, FrameType::Request);

        let decoded_payload =
            std::str::from_utf8(&encoded[HEADER_LEN..HEADER_LEN + len as usize]).unwrap();
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn encode_rejects_empty_payload() {
        let err = encode_frame(FrameType::Request, "").unwrap_err();
        assert!(matches!(err, FrameError::EmptyPayload));
    }

    #[test]
    fn decode_rejects_zero_length() {
        let header: [u8; HEADER_LEN] = [0, 0, 0, 0, 1];
        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { .. }));
    }

    #[test]
    fn decode_rejects_negative_length() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&(-1i32).to_be_bytes());
        header[4] = 1;
        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { .. }));
    }

    #[test]
    fn decode_rejects_over_limit() {
        let mut header = [0u8; HEADER_LEN];
        let over = (MAX_PAYLOAD_BYTES + 1) as i32;
        header[..4].copy_from_slice(&over.to_be_bytes());
        header[4] = 1;
        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::PayloadTooLarge { .. }));
    }

    #[test]
    fn decode_rejects_unknown_type() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&2i32.to_be_bytes());
        header[4] = 127;
        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::UnknownFrameType { .. }));
    }

    #[test]
    fn golden_vector_request_ping() {
        let payload = r#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let encoded = encode_frame(FrameType::Request, payload).unwrap();
        let header_hex = hex::encode(&encoded[..HEADER_LEN]);
        assert_eq!(header_hex, "0000003801");
    }

    #[test]
    fn max_valid_frame_accepted() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&(MAX_PAYLOAD_BYTES as i32).to_be_bytes());
        header[4] = 1;
        let (len, ft) = decode_header(&header).unwrap();
        assert_eq!(len, MAX_PAYLOAD_BYTES as i32);
        assert_eq!(ft, FrameType::Request);
    }

    #[test]
    fn frame_error_display_is_descriptive() {
        let err = FrameError::UnexpectedEof {
            stage: FrameReadStage::Header,
        };
        assert!(err.to_string().contains("EOF"));
        assert!(err.to_string().contains("header"));
    }
}
