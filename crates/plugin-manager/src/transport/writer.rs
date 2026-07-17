//! Single-owner frame writer — the sole path to child stdin.
//!
//! Only one `FrameWriter` exists per connection. All concurrent tasks submit
//! complete frame commands through an mpsc channel; the writer task encodes
//! and writes them sequentially, reporting `FrameWritten` or `WriteFailed` back.

use super::frame::{encode_frame, FrameType};

/// A command to write one complete frame to the child process stdin.
#[derive(Debug)]
pub struct WriteCommand {
    /// The frame type.
    pub frame_type: FrameType,
    /// The JSON payload as a string (will be encoded as UTF-8).
    pub payload_json: String,
    /// One-shot sender for the result.
    pub result_tx: tokio::sync::oneshot::Sender<WriteResult>,
}

/// Outcome of a single frame write attempt.
#[derive(Debug, Clone)]
pub enum WriteResult {
    /// The complete `5 + N` bytes were written successfully.
    Written,
    /// The write failed.
    WriteFailed {
        /// Error description.
        message: String,
    },
}

/// The single-owner frame writer handle.
///
/// Clone this to send commands from multiple tasks. The actual I/O is
/// performed by the writer task that holds the child stdin.
#[derive(Clone)]
pub struct FrameWriter {
    command_tx: tokio::sync::mpsc::Sender<WriteCommand>,
}

impl FrameWriter {
    /// Create a new `FrameWriter` and the corresponding writer task.
    ///
    /// The returned `FrameWriter` handle can be cloned and shared. The writer
    /// task must be spawned onto a Tokio runtime; it owns the child stdin.
    pub fn new(
        mut stdin: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
        channel_capacity: usize,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WriteCommand>(channel_capacity);

        let handle = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;

            while let Some(cmd) = rx.recv().await {
                let result = match encode_frame(cmd.frame_type, &cmd.payload_json) {
                    Ok(frame_bytes) => match stdin.write_all(&frame_bytes).await {
                        Ok(()) => WriteResult::Written,
                        Err(e) => WriteResult::WriteFailed {
                            message: format!("write_all failed: {e}"),
                        },
                    },
                    Err(e) => WriteResult::WriteFailed {
                        message: format!("encode failed: {e}"),
                    },
                };

                // Ignore send error — caller may have dropped their receiver
                let _ = cmd.result_tx.send(result);
            }

            // Best-effort flush on shutdown
            let _ = stdin.flush().await;
        });

        (Self { command_tx: tx }, handle)
    }

    /// Submit a frame for writing. Returns a receiver that will yield the result.
    pub async fn write(
        &self,
        frame_type: FrameType,
        payload_json: String,
    ) -> Result<WriteResult, tokio::sync::mpsc::error::SendError<WriteCommand>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cmd = WriteCommand {
            frame_type,
            payload_json,
            result_tx: tx,
        };
        self.command_tx.send(cmd).await?;
        Ok(rx.await.unwrap_or(WriteResult::WriteFailed {
            message: "writer task terminated".to_string(),
        }))
    }

    /// Check if the writer channel is closed.
    pub fn is_closed(&self) -> bool {
        self.command_tx.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::frame::HEADER_LEN;
    use pretty_assertions::assert_eq;

    /// A minimal in-memory async writer for testing.
    struct MockStdio {
        written: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    }

    impl MockStdio {
        fn new() -> (Self, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
            let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            (Self { written: buf.clone() }, buf)
        }
    }

    impl tokio::io::AsyncWrite for MockStdio {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<Result<usize, std::io::Error>> {
            self.written.lock().unwrap().extend_from_slice(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn writer_sends_single_frame() {
        let (mock, buf) = MockStdio::new();
        let (writer, _handle) = FrameWriter::new(mock, 16);

        let result = writer
            .write(
                FrameType::Request,
                r#"{"jsonrpc":"2.0","id":"h:1","method":"ping"}"#.to_string(),
            )
            .await
            .unwrap();

        assert!(matches!(result, WriteResult::Written));

        let written = buf.lock().unwrap();
        assert!(written.len() > HEADER_LEN);

        // Verify header
        let payload_len = i32::from_be_bytes(written[0..4].try_into().unwrap());
        assert!(payload_len > 0);
        assert_eq!(written[4], FrameType::Request as u8);
    }

    #[tokio::test]
    async fn writer_rejects_empty_payload() {
        let (mock, _buf) = MockStdio::new();
        let (writer, _handle) = FrameWriter::new(mock, 16);

        let result = writer.write(FrameType::Request, String::new()).await.unwrap();
        assert!(matches!(result, WriteResult::WriteFailed { .. }));
    }

    #[tokio::test]
    async fn writer_handles_multiple_frames() {
        let (mock, buf) = MockStdio::new();
        let (writer, _handle) = FrameWriter::new(mock, 16);

        for i in 1..=3 {
            let result = writer
                .write(
                    FrameType::Request,
                    format!(r#"{{"jsonrpc":"2.0","id":"h:{i}","method":"test"}}"#),
                )
                .await
                .unwrap();
            assert!(matches!(result, WriteResult::Written));
        }

        let written = buf.lock().unwrap();
        // All three frames should be written sequentially
        let text = String::from_utf8(written.to_vec()).unwrap();
        assert!(text.contains(r#""id":"h:1""#));
        assert!(text.contains(r#""id":"h:2""#));
        assert!(text.contains(r#""id":"h:3""#));
    }

    #[tokio::test]
    async fn writer_channel_closes_cleanly() {
        let (mock, _buf) = MockStdio::new();
        let (writer, handle) = FrameWriter::new(mock, 16);

        // Drop writer → channel closes → task exits
        drop(writer);
        handle.await.unwrap();
    }
}
