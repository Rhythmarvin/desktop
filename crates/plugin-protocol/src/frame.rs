/// Wire v1 frame header size.
pub const FRAME_HEADER_BYTES: usize = 5;
/// Absolute wire v1 payload cap applied before any payload allocation.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Identifies the payload encoding carried by one wire frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum FrameType {
    /// JSON-RPC request (has id + method).
    Request = 1,
    /// JSON-RPC success/error response (has id, exactly one of result/error).
    Response = 2,
    /// JSON-RPC notification (has method, no id).
    Notification = 3,
}

impl TryFrom<i8> for FrameType {
    type Error = FrameError;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Request),
            2 => Ok(Self::Response),
            3 => Ok(Self::Notification),
            value => Err(FrameError::UnknownType { value }),
        }
    }
}

/// An owned, fully validated wire frame whose payload still awaits content validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub frame_type: FrameType,
    pub payload: Vec<u8>,
}

/// Classifies framing failures without attempting byte-stream resynchronization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("frame payload length must be positive, got {length}")]
    NonPositiveLength { length: i32 },
    #[error("frame payload length {length} exceeds limit {maximum}")]
    PayloadTooLarge { length: usize, maximum: usize },
    #[error("unknown signed frame type {value}")]
    UnknownType { value: i8 },
    #[error("frame decoder ended with a partial {part}")]
    PartialFrame { part: PartialFramePart },
    #[error("frame payload limit must be in 1..={hard_maximum}, got {configured}")]
    InvalidMaximum {
        configured: usize,
        hard_maximum: usize,
    },
}

/// Identifies which frame component was incomplete when the stream reached EOF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialFramePart {
    Header,
    Payload,
}

impl std::fmt::Display for PartialFramePart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header => f.write_str("header"),
            Self::Payload => f.write_str("payload"),
        }
    }
}

/// Encodes a frame into the wire representation: `[length: i32 BE][type: i8][payload]`.
pub fn encode_frame(
    frame_type: FrameType,
    payload: &[u8],
    maximum_payload_bytes: usize,
) -> Result<Vec<u8>, FrameError> {
    validate_maximum(maximum_payload_bytes)?;
    let length = validate_payload_length(payload.len(), maximum_payload_bytes)?;
    let mut encoded = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.push(frame_type as i8 as u8);
    encoded.extend_from_slice(payload);
    Ok(encoded)
}

/// Incrementally decodes arbitrary pipe chunks without buffering more than one payload.
#[derive(Debug)]
pub struct FrameDecoder {
    maximum_payload_bytes: usize,
    state: DecoderState,
}

#[derive(Debug)]
enum DecoderState {
    Header {
        bytes: [u8; FRAME_HEADER_BYTES],
        filled: usize,
    },
    Payload {
        frame_type: FrameType,
        expected: usize,
        bytes: Vec<u8>,
    },
}

impl FrameDecoder {
    /// Builds a decoder with a Host-selected limit no larger than the wire hard cap.
    pub fn new(maximum_payload_bytes: usize) -> Result<Self, FrameError> {
        validate_maximum(maximum_payload_bytes)?;
        Ok(Self {
            maximum_payload_bytes,
            state: DecoderState::Header {
                bytes: [0; FRAME_HEADER_BYTES],
                filled: 0,
            },
        })
    }

    /// Consumes one arbitrary byte chunk and returns every complete frame in order.
    pub fn decode_chunk(&mut self, chunk: &[u8]) -> Result<Vec<Frame>, FrameError> {
        let mut frames = Vec::new();
        let mut offset = 0;

        while offset < chunk.len() {
            match &mut self.state {
                DecoderState::Header { bytes, filled } => {
                    let remaining = FRAME_HEADER_BYTES - *filled;
                    let copied = remaining.min(chunk.len() - offset);
                    bytes[*filled..*filled + copied].copy_from_slice(&chunk[offset..offset + copied]);
                    *filled += copied;
                    offset += copied;

                    if *filled == FRAME_HEADER_BYTES {
                        let length = i32::from_be_bytes(
                            [bytes[0], bytes[1], bytes[2], bytes[3]],
                        );
                        if length <= 0 {
                            return Err(FrameError::NonPositiveLength { length });
                        }
                        let expected = usize::try_from(length)
                            .map_err(|_| FrameError::NonPositiveLength { length })?;
                        validate_payload_length(expected, self.maximum_payload_bytes)?;
                        let frame_type = FrameType::try_from(bytes[4] as i8)?;
                        self.state = DecoderState::Payload {
                            frame_type,
                            expected,
                            bytes: Vec::with_capacity(expected),
                        };
                    }
                }
                DecoderState::Payload {
                    frame_type,
                    expected,
                    bytes,
                } => {
                    let remaining = *expected - bytes.len();
                    let copied = remaining.min(chunk.len() - offset);
                    bytes.extend_from_slice(&chunk[offset..offset + copied]);
                    offset += copied;

                    if bytes.len() == *expected {
                        let payload = std::mem::take(bytes);
                        frames.push(Frame {
                            frame_type: *frame_type,
                            payload,
                        });
                        self.state = DecoderState::Header {
                            bytes: [0; FRAME_HEADER_BYTES],
                            filled: 0,
                        };
                    }
                }
            }
        }
        Ok(frames)
    }

    /// Validates that EOF occurred exactly on a frame boundary.
    pub fn finish(self) -> Result<(), FrameError> {
        match self.state {
            DecoderState::Header { filled: 0, .. } => Ok(()),
            DecoderState::Header { .. } => Err(FrameError::PartialFrame {
                part: PartialFramePart::Header,
            }),
            DecoderState::Payload { .. } => Err(FrameError::PartialFrame {
                part: PartialFramePart::Payload,
            }),
        }
    }
}

/// Rejects configuration values that would create a second, incompatible wire profile.
fn validate_maximum(maximum_payload_bytes: usize) -> Result<(), FrameError> {
    if !(1..=MAX_FRAME_BYTES).contains(&maximum_payload_bytes) {
        return Err(FrameError::InvalidMaximum {
            configured: maximum_payload_bytes,
            hard_maximum: MAX_FRAME_BYTES,
        });
    }
    Ok(())
}

/// Checks the signed wire length and Host policy before payload allocation or encoding.
fn validate_payload_length(length: usize, maximum: usize) -> Result<i32, FrameError> {
    if length == 0 {
        return Err(FrameError::NonPositiveLength { length: 0 });
    }
    if length > maximum {
        return Err(FrameError::PayloadTooLarge { length, maximum });
    }
    i32::try_from(length).map_err(|_| FrameError::PayloadTooLarge { length, maximum })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const PING: &[u8] = br#"{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}"#;

    /// Canonical golden vector: Request frame, `[length: i32 BE][type: i8][payload]`.
    #[test]
    fn encodes_canonical_golden_vector() {
        let encoded =
            encode_frame(FrameType::Request, PING, MAX_FRAME_BYTES)
                .unwrap_or_else(|e| panic!("encoding should succeed: {e}"));
        // length = 56 (0x38), type = 1 (Request)
        assert_eq!(&encoded[..4], &[0x00, 0x00, 0x00, 0x38]);
        assert_eq!(encoded[4], 0x01);
        assert_eq!(&encoded[5..], PING);
    }

    #[test]
    fn encodes_response_frame() {
        let result = br#"{"jsonrpc":"2.0","id":"h:1","result":{"pong":true}}"#;
        let encoded = encode_frame(FrameType::Response, result, MAX_FRAME_BYTES).unwrap();
        assert_eq!(encoded[4], 0x02); // Response type byte
        assert_eq!(&encoded[5..], result);
    }

    #[test]
    fn encodes_notification_frame() {
        let note = br#"{"jsonrpc":"2.0","method":"$/hello"}"#;
        let encoded = encode_frame(FrameType::Notification, note, MAX_FRAME_BYTES).unwrap();
        assert_eq!(encoded[4], 0x03); // Notification type byte
        assert_eq!(&encoded[5..], note);
    }

    /// Every split position + coalesced frames tested with one decoder instance.
    #[test]
    fn decodes_arbitrary_splits_and_coalesced_frames() {
        let encoded =
            encode_frame(FrameType::Request, PING, MAX_FRAME_BYTES)
                .unwrap_or_else(|e| panic!("encoding should succeed: {e}"));
        let expected = vec![Frame {
            frame_type: FrameType::Request,
            payload: PING.to_vec(),
        }];

        for cut in 0..=encoded.len() {
            let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES).unwrap();
            let mut actual = decoder
                .decode_chunk(&encoded[..cut])
                .unwrap_or_else(|e| panic!("first chunk at cut {cut}: {e}"));
            actual.extend(
                decoder
                    .decode_chunk(&encoded[cut..])
                    .unwrap_or_else(|e| panic!("second chunk at cut {cut}: {e}")),
            );
            assert_eq!(actual, expected, "failed at cut {cut}");
            assert_eq!(decoder.finish(), Ok(()), "finish at cut {cut}");
        }

        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES).unwrap();
        let mut doubled = encoded.clone();
        doubled.extend_from_slice(&encoded);
        assert_eq!(
            decoder.decode_chunk(&doubled),
            Ok([expected.clone(), expected].concat())
        );
    }

    #[test]
    fn rejects_invalid_headers_and_partial_eof() {
        let cases = [
            (
                [0x00, 0x00, 0x00, 0x00, 0x01],
                FrameError::NonPositiveLength { length: 0 },
            ),
            (
                [0xff, 0xff, 0xff, 0xff, 0x01],
                FrameError::NonPositiveLength { length: -1 },
            ),
            (
                [0x00, 0x00, 0x00, 0x02, 0x7f],
                FrameError::UnknownType { value: 127 },
            ),
        ];
        for (header, expected) in cases {
            let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES).unwrap();
            assert_eq!(decoder.decode_chunk(&header), Err(expected));
        }

        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES).unwrap();
        assert_eq!(decoder.decode_chunk(&[0, 0]), Ok(Vec::new()));
        assert_eq!(
            decoder.finish(),
            Err(FrameError::PartialFrame {
                part: PartialFramePart::Header,
            })
        );
    }
}