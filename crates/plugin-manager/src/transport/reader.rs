//! Incremental frame reader — parses 5-byte binary frames from a byte stream.
//!
//! State machine: AwaitingHeader → AwaitingPayload → dispatch → AwaitingHeader.

use super::frame::{decode_header, FrameError, FrameReadStage, FrameType, HEADER_LEN, MAX_PAYLOAD_BYTES};

/// A decoded frame ready for dispatch.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// The frame type (1=Request, 2=Response, 3=Notification).
    pub frame_type: FrameType,
    /// The raw UTF-8 JSON payload.
    pub payload: String,
    /// The parsed JSON value.
    pub payload_json: serde_json::Value,
}

/// State of the incremental frame parser.
#[derive(Debug, Clone)]
enum ParseState {
    /// Waiting for at least 5 header bytes.
    AwaitingHeader,
    /// Waiting for the full payload (expected byte count, frame type, bytes accumulated).
    AwaitingPayload {
        expected: u32,
        frame_type: FrameType,
        buffer: Vec<u8>,
    },
}

/// Incremental 5-byte binary frame reader.
///
/// Call `feed()` with incoming byte chunks. Complete frames are returned immediately.
/// Partial frames are buffered internally. Call `finish()` when the stream ends.
pub struct FrameReader {
    /// Accumulated bytes not yet forming a complete header.
    buffer: Vec<u8>,
    state: ParseState,
}

impl FrameReader {
    /// Create a new empty frame reader.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            state: ParseState::AwaitingHeader,
        }
    }

    /// Feed incoming bytes. Returns any complete frames parsed.
    ///
    /// # Errors
    /// Returns `FrameError` if the data is malformed.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<DecodedFrame>, FrameError> {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();

        loop {
            // Take ownership of state to avoid borrow issues with mutable payload buffer
            let current_state = std::mem::replace(&mut self.state, ParseState::AwaitingHeader);

            match current_state {
                ParseState::AwaitingHeader => {
                    if self.buffer.len() < HEADER_LEN {
                        self.state = ParseState::AwaitingHeader;
                        break;
                    }
                    let header: [u8; HEADER_LEN] =
                        self.buffer[..HEADER_LEN].try_into().unwrap();
                    let (length, frame_type) = decode_header(&header)?;

                    self.buffer.drain(..HEADER_LEN);
                    self.state = ParseState::AwaitingPayload {
                        expected: length as u32,
                        frame_type,
                        buffer: Vec::with_capacity(length as usize),
                    };
                }
                ParseState::AwaitingPayload {
                    expected,
                    frame_type,
                    mut buffer,
                } => {
                    let remaining = expected as usize - buffer.len();
                    let to_take = remaining.min(self.buffer.len());
                    buffer.extend_from_slice(&self.buffer[..to_take]);
                    self.buffer.drain(..to_take);

                    if buffer.len() == expected as usize {
                        // Payload complete — validate UTF-8 and JSON
                        self.state = ParseState::AwaitingHeader;

                        let payload = String::from_utf8(buffer)
                            .map_err(|_| FrameError::InvalidUtf8)?;

                        let payload_json: serde_json::Value = serde_json::from_str(&payload)
                            .map_err(|e| FrameError::EnvelopeError {
                                message: format!("invalid JSON: {e}"),
                            })?;

                        if !payload_json.is_object() {
                            return Err(FrameError::EnvelopeError {
                                message: "frame payload must be a JSON object".to_string(),
                            });
                        }

                        frames.push(DecodedFrame {
                            frame_type,
                            payload,
                            payload_json,
                        });
                    } else {
                        // Still need more data — put the partial buffer back
                        self.state = ParseState::AwaitingPayload {
                            expected,
                            frame_type,
                            buffer: buffer,
                        };
                        break;
                    }
                }
            }
        }

        Ok(frames)
    }

    /// Signal end of stream. Returns an error if partial frame data remains.
    pub fn finish(&self) -> Result<(), FrameError> {
        match &self.state {
            ParseState::AwaitingHeader if self.buffer.is_empty() => Ok(()),
            ParseState::AwaitingHeader => {
                // Partial header bytes remain
                Err(FrameError::UnexpectedEof {
                    stage: FrameReadStage::Header,
                })
            }
            ParseState::AwaitingPayload { expected, buffer, .. } => {
                Err(FrameError::UnexpectedEof {
                    stage: FrameReadStage::Payload {
                        expected: *expected,
                        read: buffer.len() as u32,
                    },
                })
            }
        }
    }

    /// Returns true if the reader is idle (no buffered data, waiting for header).
    pub fn is_idle(&self) -> bool {
        matches!(self.state, ParseState::AwaitingHeader) && self.buffer.is_empty()
    }
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_frame(frame_type: FrameType, payload: &str) -> Vec<u8> {
        let payload_bytes = payload.as_bytes();
        let mut frame = Vec::with_capacity(HEADER_LEN + payload_bytes.len());
        frame.extend_from_slice(&(payload_bytes.len() as i32).to_be_bytes());
        frame.push(frame_type as u8);
        frame.extend_from_slice(payload_bytes);
        frame
    }

    #[test]
    fn reads_single_complete_frame() {
        let frame = make_frame(FrameType::Request, r#"{"jsonrpc":"2.0","id":"h:1","method":"ping"}"#);
        let mut reader = FrameReader::new();
        let frames = reader.feed(&frame).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_type, FrameType::Request);
        assert_eq!(frames[0].payload_json["method"], "ping");
        assert!(reader.is_idle());
    }

    #[test]
    fn reads_multiple_frames_in_one_chunk() {
        let f1 = make_frame(FrameType::Request, r#"{"jsonrpc":"2.0","id":"h:1","method":"a"}"#);
        let f2 = make_frame(FrameType::Response, r#"{"jsonrpc":"2.0","id":"h:1","result":"ok"}"#);
        let combined = [f1, f2].concat();

        let mut reader = FrameReader::new();
        let frames = reader.feed(&combined).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_type, FrameType::Request);
        assert_eq!(frames[1].frame_type, FrameType::Response);
    }

    #[test]
    fn handles_byte_by_byte_feed() {
        let full = make_frame(FrameType::Notification, r#"{"jsonrpc":"2.0","method":"$/exit"}"#);
        let mut reader = FrameReader::new();

        // Feed one byte at a time
        for i in 0..full.len() {
            let result = reader.feed(&full[i..i + 1]).unwrap();
            if i < full.len() - 1 {
                assert!(result.is_empty(), "should not yield frame until complete");
            }
        }
        assert!(reader.is_idle());
    }

    #[test]
    fn handles_partial_header() {
        let frame = make_frame(FrameType::Request, r#"{"a":1}"#);
        let mut reader = FrameReader::new();

        // Feed only 3 bytes of header
        let frames = reader.feed(&frame[..3]).unwrap();
        assert!(frames.is_empty());

        // Feed the rest
        let frames = reader.feed(&frame[3..]).unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn handles_partial_payload() {
        let frame = make_frame(FrameType::Request, r#"{"jsonrpc":"2.0","id":"h:1","method":"ping"}"#);
        let split = HEADER_LEN + 10; // header + 10 bytes of payload
        let mut reader = FrameReader::new();

        let frames = reader.feed(&frame[..split]).unwrap();
        assert!(frames.is_empty(), "partial payload should not parse yet");

        let frames = reader.feed(&frame[split..]).unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn rejects_invalid_utf8_in_payload() {
        let invalid_utf8: Vec<u8> = vec![0xFF, 0xFE, 0xFD];
        let mut frame = Vec::with_capacity(HEADER_LEN + invalid_utf8.len());
        frame.extend_from_slice(&(invalid_utf8.len() as i32).to_be_bytes());
        frame.push(FrameType::Request as u8);
        frame.extend_from_slice(&invalid_utf8);

        let mut reader = FrameReader::new();
        let err = reader.feed(&frame).unwrap_err();
        assert!(matches!(err, FrameError::InvalidUtf8));
    }

    #[test]
    fn rejects_non_object_json() {
        // JSON array as top-level
        let payload = "[1,2,3]";
        let frame = make_frame(FrameType::Request, payload);
        let mut reader = FrameReader::new();
        let err = reader.feed(&frame).unwrap_err();
        assert!(matches!(err, FrameError::EnvelopeError { .. }));
    }

    #[test]
    fn finish_with_clean_state_ok() {
        let frame = make_frame(FrameType::Request, r#"{"a":1}"#);
        let mut reader = FrameReader::new();
        reader.feed(&frame).unwrap();
        assert!(reader.finish().is_ok());
    }

    #[test]
    fn finish_with_partial_data_err() {
        let frame = make_frame(FrameType::Request, r#"{"a":1}"#);
        let mut reader = FrameReader::new();
        reader.feed(&frame[..HEADER_LEN - 1]).unwrap();
        let err = reader.finish().unwrap_err();
        assert!(matches!(err, FrameError::UnexpectedEof { .. }));
    }

    #[test]
    fn fuzz_arbitrary_bytes_no_panic() {
        // Feed random byte sequences — must never panic, overflow, or allocate beyond budget
        let test_inputs: &[&[u8]] = &[
            &[],                                                           // empty
            &[0x00],                                                       // partial header
            &[0x00, 0x00, 0x00, 0x00, 0x01],                              // zero length
            &[0xff, 0xff, 0xff, 0xff, 0x01],                              // negative
            &[0x00, 0x80, 0x00, 0x01, 0x01],                              // over limit
            &[0x00, 0x00, 0x00, 0x02, 0x7f],                              // unknown type
            &[0x00, 0x00, 0x00, 0x02, 0x7f, 0x7b, 0x7d],                  // unknown type + payload
            &[0xFF, 0xFE, 0xFD, 0xFC, 0x01],                              // garbled header
            &[0x00, 0x00, 0x00, 0x0a, 0x01, 0xFF, 0xFE, 0xFD],            // invalid UTF-8 payload
            &[0x00, 0x00, 0x00, 0x01, 0x01, 0x00],                        // NUL byte in payload
            &[0x7f, 0xff, 0xff, 0xff, 0x01],                              // max i32
            &[0x00, 0x00, 0x00, 0x64, 0x01],                              // declared 100 bytes but stream ends
        ];

        for (i, input) in test_inputs.iter().enumerate() {
            let mut reader = FrameReader::new();
            // Must not panic
            let result = reader.feed(input);
            // Result is either Ok or Err — both are fine, just no panic
            if let Err(e) = &result {
                // Errors must be descriptive, not generic panics
                let msg = e.to_string();
                assert!(!msg.is_empty(), "input [{i}]: error message must not be empty");
            }
            // finish() must also not panic
            let _ = reader.finish();
        }
    }
}
