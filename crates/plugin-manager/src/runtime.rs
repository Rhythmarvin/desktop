use std::path::Path;
use std::process::Stdio;

use ora_plugin_protocol::frame::{FrameDecoder, FrameType, MAX_FRAME_BYTES, encode_frame};
use ora_plugin_protocol::lifecycle::InitializeParams;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Owns one Bun plugin child process and its stdin/stdout pipes.
pub struct PluginProcess {
    child: Mutex<Child>,
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<tokio::process::ChildStdout>,
    decoder: Mutex<FrameDecoder>,
}

/// A handle to a running plugin process (used by invoke/stop).
pub struct PluginProcessHandle {
    inner: PluginProcess,
}

impl PluginProcessHandle {
    pub fn new(process: PluginProcess) -> Self {
        Self { inner: process }
    }

    pub async fn invoke(
        &self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, String> {
        self.inner.invoke(request_id, method, params).await
    }

    pub async fn shutdown(self) -> Result<(), String> {
        self.inner.shutdown().await
    }
}

pub struct InvokeResult {
    pub request_id: String,
    pub result: serde_json::Value,
}

impl PluginProcess {
    /// Spawns Bun with the bootstrap script and completes the $/initialize handshake.
    pub async fn spawn(
        bun_path: &Path,
        bootstrap_path: &Path,
        init_params: InitializeParams,
    ) -> Result<Self, String> {
        let mut child = Command::new(bun_path)
            .arg(bootstrap_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("spawn bun: {e}"))?;

        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        let mut stdout = child.stdout.take().ok_or("no stdout")?;

        // Send $/initialize Request
        let init_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:1",
            "method": "$/initialize",
            "params": init_params,
        });
        let init_bytes = serde_json::to_vec(&init_json).map_err(|e| format!("json: {e}"))?;
        let init_frame = encode_frame(FrameType::Request, &init_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;
        stdin
            .write_all(&init_frame)
            .await
            .map_err(|e| format!("write init: {e}"))?;

        // Read $/initialize Response
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES)
            .map_err(|e| format!("decoder: {e}"))?;
        let mut buf = [0u8; 65536];
        let n = stdout
            .read(&mut buf)
            .await
            .map_err(|e| format!("read init response: {e}"))?;

        let frames = decoder
            .decode_chunk(&buf[..n])
            .map_err(|e| format!("decode init response: {e}"))?;

        let mut init_ok = false;
        for frame in &frames {
            if frame.frame_type == FrameType::Response {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&frame.payload) {
                    if value["id"] == "h:1" && value.get("result").is_some() {
                        init_ok = true;
                        tracing::info!(
                            session_id = %value["result"]["sessionId"],
                            "plugin handshake complete"
                        );
                    }
                }
            }
        }

        if !init_ok {
            return Err("handshake failed: no initialize response".into());
        }

        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(stdout),
            decoder: Mutex::new(FrameDecoder::new(MAX_FRAME_BYTES).map_err(|e| format!("{e}"))?),
        })
    }

    /// Sends a JSON-RPC Request to the plugin and reads the Response.
    pub async fn invoke(
        &self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, String> {
        // Send request
        let request_json = if params.is_null() {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
            })
        } else {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params,
            })
        };
        let request_bytes =
            serde_json::to_vec(&request_json).map_err(|e| format!("json: {e}"))?;
        let request_frame = encode_frame(FrameType::Request, &request_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(&request_frame)
            .await
            .map_err(|e| format!("write request: {e}"))?;
        drop(stdin);

        // Read responses until we get a matching Response or Notification
        let mut stdout = self.stdout.lock().await;
        let mut decoder = self.decoder.lock().await;
        let mut buf = [0u8; 65536];

        // Read up to 3 chunks to collect all responses
        for _ in 0..3 {
            let n = stdout
                .read(&mut buf)
                .await
                .map_err(|e| format!("read response: {e}"))?;
            if n == 0 {
                break;
            }

            let frames = decoder
                .decode_chunk(&buf[..n])
                .map_err(|e| format!("decode: {e}"))?;

            for frame in &frames {
                let value: serde_json::Value =
                    serde_json::from_slice(&frame.payload).map_err(|e| format!("json: {e}"))?;

                // Check for notification
                if frame.frame_type == FrameType::Notification {
                    let method = value["method"].as_str().unwrap_or("");
                    tracing::info!(%method, "plugin notification");
                    continue;
                }

                // Check for matching response
                if value["id"].as_str() == Some(request_id) {
                    if let Some(result) = value.get("result") {
                        tracing::info!(request_id, "plugin response received");
                        return Ok(InvokeResult {
                            request_id: request_id.to_string(),
                            result: result.clone(),
                        });
                    }
                    if let Some(error) = value.get("error") {
                        return Err(format!(
                            "plugin error: {} (code: {})",
                            error["message"].as_str().unwrap_or("unknown"),
                            error["code"].as_i64().unwrap_or(0)
                        ));
                    }
                }
            }
        }

        Err("no response received".into())
    }

    /// Sends $/exit Notification and waits for child exit.
    pub async fn shutdown(self) -> Result<(), String> {
        let exit_json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "$/exit"
        });
        let exit_bytes =
            serde_json::to_vec(&exit_json).map_err(|e| format!("json: {e}"))?;
        let exit_frame = encode_frame(FrameType::Notification, &exit_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(&exit_frame)
            .await
            .map_err(|e| format!("write exit: {e}"))?;
        drop(stdin);

        let mut child = self.child.lock().await;
        let status = child.wait().await.map_err(|e| format!("wait: {e}"))?;
        tracing::info!(%status, "plugin exited");
        Ok(())
    }
}
