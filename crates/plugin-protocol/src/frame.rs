//! 5-byte binary frame format for the Ora wire protocol.
//!
//! ```text
//! offset  size  field
//! 0       4     length: signed i32, big-endian (payload bytes only, excluding header)
//! 4       1     type: signed i8 (1=Request, 2=Response, 3=Notification)
//! 5       N     payload: exactly length bytes of UTF-8 JSON
//! ```

/// Total header length in bytes.
pub const HEADER_LEN: usize = 5;

/// Maximum payload bytes (8 MiB).
pub const MAX_PAYLOAD_BYTES: u32 = 8 * 1024 * 1024;

/// Frame type constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum FrameType {
    /// JSON-RPC request: `{jsonrpc, id, method, params?}`
    Request = 1,
    /// JSON-RPC success/error response: `{jsonrpc, id, result}` or `{jsonrpc, id, error}`
    Response = 2,
    /// JSON-RPC notification: `{jsonrpc, method, params?}`
    Notification = 3,
}

impl FrameType {
    /// Convert from an i8 wire value. Returns None for reserved/unknown types.
    pub fn from_i8(value: i8) -> Option<Self> {
        match value {
            1 => Some(Self::Request),
            2 => Some(Self::Response),
            3 => Some(Self::Notification),
            _ => None,
        }
    }

    /// Convert to i8 wire value.
    pub fn to_i8(self) -> i8 {
        self as i8
    }
}

/// Errors from frame encoding/decoding.
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
    /// Encoder: payload is empty (zero bytes).
    EmptyPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReadStage {
    Header,
    Type,
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
                FrameReadStage::Header => write!(f, "unexpected EOF while reading frame header"),
                FrameReadStage::Type => write!(f, "unexpected EOF while reading frame type"),
                FrameReadStage::Payload { expected, read } => write!(
                    f,
                    "unexpected EOF while reading payload: expected {expected} bytes, got {read}"
                ),
            },
            Self::InvalidUtf8 => write!(f, "payload is not valid UTF-8"),
            Self::EmptyPayload => write!(f, "payload must not be empty"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Encode a complete frame: 5-byte header + payload bytes.
///
/// Validates: payload non-empty, payload ≤ max_bytes, valid frame type.
pub fn encode_frame(
    frame_type: FrameType,
    payload_bytes: &[u8],
    max_bytes: u32,
) -> Result<Vec<u8>, FrameError> {
    let payload_len = payload_bytes.len();

    if payload_len == 0 {
        return Err(FrameError::EmptyPayload);
    }
    if payload_len > max_bytes as usize {
        return Err(FrameError::PayloadTooLarge {
            length: payload_len as u32,
            max: max_bytes,
        });
    }

    let length_i32: i32 = payload_len
        .try_into()
        .map_err(|_| FrameError::InvalidLength { length: -1 })?;

    let mut frame = Vec::with_capacity(HEADER_LEN + payload_len);
    frame.extend_from_slice(&length_i32.to_be_bytes());
    frame.push(frame_type.to_i8() as u8);
    frame.extend_from_slice(payload_bytes);

    Ok(frame)
}

/// Decode a frame header from 5 bytes. Returns (payload_length, frame_type).
///
/// Validates: length > 0, length ≤ MAX_PAYLOAD_BYTES, valid frame type.
pub fn decode_header(header: &[u8; HEADER_LEN]) -> Result<(i32, FrameType), FrameError> {
    let length = i32::from_be_bytes(header[0..4].try_into().unwrap());

    if length <= 0 {
        return Err(FrameError::InvalidLength { length });
    }
    if length as u32 > MAX_PAYLOAD_BYTES {
        return Err(FrameError::PayloadTooLarge {
            length: length as u32,
            max: MAX_PAYLOAD_BYTES,
        });
    }

    let type_byte = header[4] as i8;
    let frame_type =
        FrameType::from_i8(type_byte).ok_or(FrameError::UnknownFrameType { value: type_byte })?;

    Ok((length, frame_type))
}

/// Generate the 5-byte header hex string (for golden vector comparison).
pub fn header_hex(frame_type: FrameType, payload_len: i32) -> String {
    let mut header = [0u8; HEADER_LEN];
    header[..4].copy_from_slice(&payload_len.to_be_bytes());
    header[4] = frame_type.to_i8() as u8;
    hex::encode(header)
}

/// Generate the full frame hex string (header + payload, for golden vector comparison).
pub fn frame_hex(frame_type: FrameType, payload_utf8: &str) -> String {
    let payload_bytes = payload_utf8.as_bytes();
    let payload_len = payload_bytes.len() as i32;
    let mut frame = Vec::with_capacity(HEADER_LEN + payload_bytes.len());
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.push(frame_type.to_i8() as u8);
    frame.extend_from_slice(payload_bytes);
    hex::encode(&frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ── encode / decode round-trip ────────────────────────────────

    #[test]
    fn encode_request_frame() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let frame = encode_frame(FrameType::Request, payload, MAX_PAYLOAD_BYTES).unwrap();

        assert_eq!(frame.len(), HEADER_LEN + payload.len());
        // Verify header
        assert_eq!(&frame[0..4], &56i32.to_be_bytes());
        assert_eq!(frame[4], 1u8);
        // Verify payload
        assert_eq!(&frame[5..], payload);
    }

    #[test]
    fn encode_response_frame() {
        let payload = br#"{"jsonrpc":"2.0","id":"h:1","result":"ok"}"#;
        let frame = encode_frame(FrameType::Response, payload, MAX_PAYLOAD_BYTES).unwrap();

        assert_eq!(frame.len(), HEADER_LEN + payload.len());
        assert_eq!(&frame[0..4], &42i32.to_be_bytes());
        assert_eq!(frame[4], 2u8);
    }

    #[test]
    fn encode_notification_frame() {
        let payload = br#"{"jsonrpc":"2.0","method":"$/exit"}"#;
        let frame = encode_frame(FrameType::Notification, payload, MAX_PAYLOAD_BYTES).unwrap();

        assert_eq!(frame.len(), HEADER_LEN + payload.len());
        assert_eq!(&frame[0..4], &35i32.to_be_bytes());
        assert_eq!(frame[4], 3u8);
    }

    #[test]
    fn decode_header_valid() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&42i32.to_be_bytes());
        header[4] = 2; // Response

        let (len, ft) = decode_header(&header).unwrap();
        assert_eq!(len, 42);
        assert_eq!(ft, FrameType::Response);
    }

    #[test]
    fn decode_header_zero_length() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&0i32.to_be_bytes());
        header[4] = 1;

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { length: 0 }));
    }

    #[test]
    fn decode_header_negative_length() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&(-1i32).to_be_bytes());
        header[4] = 1;

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { .. }));
    }

    #[test]
    fn decode_header_over_limit() {
        let mut header = [0u8; HEADER_LEN];
        let over = (MAX_PAYLOAD_BYTES + 1) as i32;
        header[..4].copy_from_slice(&over.to_be_bytes());
        header[4] = 1;

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::PayloadTooLarge { .. }));
    }

    #[test]
    fn decode_header_unknown_type() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&2i32.to_be_bytes());
        header[4] = 127; // reserved

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::UnknownFrameType { value: 127 }));
    }

    #[test]
    fn encode_rejects_empty_payload() {
        let err = encode_frame(FrameType::Request, b"", MAX_PAYLOAD_BYTES).unwrap_err();
        assert!(matches!(err, FrameError::EmptyPayload));
    }

    // ── Golden Vector verification ────────────────────────────────

    #[test]
    fn golden_vector_request_ping() {
        let payload = r#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let payload_bytes = payload.as_bytes();
        assert_eq!(payload_bytes.len(), 56);

        let frame = encode_frame(FrameType::Request, payload_bytes, MAX_PAYLOAD_BYTES).unwrap();
        let header_hex_str = hex::encode(&frame[..HEADER_LEN]);
        assert_eq!(header_hex_str, "0000003801");

        let full_hex = hex::encode(&frame);
        let payload_hex = hex::encode(payload_bytes);
        assert!(full_hex.starts_with("0000003801"));
        assert!(full_hex.ends_with(&payload_hex));
    }

    #[test]
    fn golden_vector_response_ok() {
        let payload = r#"{"jsonrpc":"2.0","id":"h:1","result":"ok"}"#;
        let payload_bytes = payload.as_bytes();
        assert_eq!(payload_bytes.len(), 42);

        let frame = encode_frame(FrameType::Response, payload_bytes, MAX_PAYLOAD_BYTES).unwrap();
        let header_hex_str = hex::encode(&frame[..HEADER_LEN]);
        assert_eq!(header_hex_str, "0000002a02");
    }

    #[test]
    fn golden_vector_notification_exit() {
        let payload = r#"{"jsonrpc":"2.0","method":"$/exit"}"#;
        let payload_bytes = payload.as_bytes();
        assert_eq!(payload_bytes.len(), 35);

        let frame =
            encode_frame(FrameType::Notification, payload_bytes, MAX_PAYLOAD_BYTES).unwrap();
        let header_hex_str = hex::encode(&frame[..HEADER_LEN]);
        assert_eq!(header_hex_str, "0000002303");
    }

    #[test]
    fn golden_vector_stream_with_chinese() {
        let payload = r#"{"jsonrpc":"2.0","method":"$/stream","params":{"id":"h:1","seq":1,"value":{"kind":"textDelta","text":"你好"}}}"#;
        let payload_bytes = payload.as_bytes();
        assert_eq!(payload_bytes.len(), 112);

        let frame =
            encode_frame(FrameType::Notification, payload_bytes, MAX_PAYLOAD_BYTES).unwrap();
        let header_hex_str = hex::encode(&frame[..HEADER_LEN]);
        assert_eq!(header_hex_str, "0000007003");
    }

    #[test]
    fn invalid_vector_zero_length_header() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&0i32.to_be_bytes());
        header[4] = 1;

        let hh = hex::encode(&header);
        assert_eq!(hh, "0000000001");

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { .. }));
    }

    #[test]
    fn invalid_vector_negative_length_header() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&(-1i32).to_be_bytes());
        header[4] = 1;

        let hh = hex::encode(&header);
        assert_eq!(hh, "ffffffff01");

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { .. }));
    }

    #[test]
    fn invalid_vector_over_limit_header() {
        let mut header = [0u8; HEADER_LEN];
        // 8 MiB + 1 = 0x00800001
        let over: i32 = 8 * 1024 * 1024 + 1;
        header[..4].copy_from_slice(&over.to_be_bytes());
        header[4] = 1;

        let hh = hex::encode(&header);
        assert_eq!(hh, "0080000101");

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::PayloadTooLarge { .. }));
    }

    #[test]
    fn invalid_vector_unknown_type() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&2i32.to_be_bytes());
        header[4] = 127;

        let hh = hex::encode(&header);
        assert_eq!(hh, "000000027f");

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::UnknownFrameType { value: 127 }));
    }

    #[test]
    fn i32_min_length_rejected() {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&i32::MIN.to_be_bytes());
        header[4] = 1;

        let err = decode_header(&header).unwrap_err();
        assert!(matches!(err, FrameError::InvalidLength { .. }));
    }

    #[test]
    fn max_valid_frame_accepted() {
        // A frame at exactly MAX_PAYLOAD_BYTES should be accepted by the header check
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&(MAX_PAYLOAD_BYTES as i32).to_be_bytes());
        header[4] = 1;

        let (len, ft) = decode_header(&header).unwrap();
        assert_eq!(len, MAX_PAYLOAD_BYTES as i32);
        assert_eq!(ft, FrameType::Request);
    }

    // ── Header hex utility ────────────────────────────────────────

    #[test]
    fn header_hex_matches_expected() {
        let hex_str = header_hex(FrameType::Request, 56);
        assert_eq!(hex_str, "0000003801");
    }

    #[test]
    fn frame_hex_bidirectional() {
        let payload = r#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;
        let hex_full = frame_hex(FrameType::Request, payload);

        // Decode the hex back
        let bytes = hex::decode(&hex_full).unwrap();
        let (len, ft) = decode_header((&bytes[..HEADER_LEN]).try_into().unwrap()).unwrap();
        assert_eq!(len, 56);
        assert_eq!(ft, FrameType::Request);

        let decoded_payload = std::str::from_utf8(&bytes[HEADER_LEN..]).unwrap();
        assert_eq!(decoded_payload, payload);
    }
}
