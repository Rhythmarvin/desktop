use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use ora_plugin_protocol::frame::{FrameDecoder, FrameType, MAX_FRAME_BYTES, encode_frame};
use ora_plugin_protocol::lifecycle::InitializeParams;

/// Owns one Bun plugin child process and its stdin/stdout pipes.
pub struct PluginProcess {
    child: Child,
    stdin: Box<dyn Write + Send>,
    stdout: Box<dyn Read + Send>,
    decoder: FrameDecoder,
    activated: bool,
}

/// A handle to a running plugin process (used by invoke/stop).
pub struct PluginProcessHandle {
    inner: Option<PluginProcess>,
}

impl PluginProcessHandle {
    pub fn new(process: PluginProcess) -> Self {
        Self {
            inner: Some(process),
        }
    }

    /// Sends $/activate Request to transition plugin from initialized → running.
    /// Must be called after spawn() and before invoke().
    pub fn activate(&mut self) -> Result<(), String> {
        self.inner
            .as_mut()
            .ok_or("process already stopped")?
            .activate()
    }

    pub fn invoke(
        &mut self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, String> {
        self.inner
            .as_mut()
            .ok_or("process already stopped")?
            .invoke(request_id, method, params)
    }

    pub fn shutdown(mut self) -> Result<(), String> {
        self.inner
            .take()
            .ok_or("process already stopped")?
            .shutdown()
    }
}

pub struct InvokeResult {
    pub request_id: String,
    pub result: serde_json::Value,
}

impl PluginProcess {
    /// Spawns Bun and completes $/initialize handshake.
    /// The plugin is left in the "awaitingActivate" phase.
    pub fn spawn(
        bun_path: &Path,
        plugin_path: &Path,
        init_params: InitializeParams,
    ) -> Result<Self, String> {
        let mut child = Command::new(bun_path)
            .arg(plugin_path)
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
        stdin.write_all(&init_frame).map_err(|e| format!("write init: {e}"))?;

        // Read $/initialize Response
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES).map_err(|e| format!("decoder: {e}"))?;
        let frames = {
            let mut r: &mut dyn Read = &mut stdout;
            read_frames(&mut decoder, &mut r)?
        };

        let init_ok = frames.iter().any(|f| {
            f.frame_type == FrameType::Response
                && f.as_json().map(|v| v["id"] == "h:1").unwrap_or(false)
        });

        if !init_ok {
            return Err("handshake failed: no initialize response".into());
        }

        Ok(Self {
            child,
            stdin: Box::new(stdin),
            stdout: Box::new(stdout),
            decoder,
            activated: false,
        })
    }

    /// Sends $/activate Request to transition from initialized → running.
    pub fn activate(&mut self) -> Result<(), String> {
        if self.activated {
            return Err("plugin already activated".into());
        }

        let act_json = serde_json::json!({
            "jsonrpc": "2.0", "id": "h:2", "method": "$/activate",
            "params": { "reason": "manualStart" }
        });
        let act_bytes = serde_json::to_vec(&act_json).map_err(|e| format!("json: {e}"))?;
        let act_frame = encode_frame(FrameType::Request, &act_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;
        self.stdin.write_all(&act_frame).map_err(|e| format!("write activate: {e}"))?;

        let stdout: &mut dyn Read = &mut *self.stdout;
        let act_frames = read_frames(&mut self.decoder, stdout)?;
        let act_ok = act_frames.iter().any(|f| {
            f.frame_type == FrameType::Response
                && f.as_json().map(|v| v["id"] == "h:2").unwrap_or(false)
        });
        if !act_ok {
            return Err("activate failed".into());
        }

        self.activated = true;
        Ok(())
    }

    /// Sends a JSON-RPC Request to the plugin and reads the Response.
    pub fn invoke(
        &mut self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, String> {
        eprintln!("[host] → invoke: method={method} requestId={request_id}");
        let request_json = if params.is_null() {
            serde_json::json!({ "jsonrpc": "2.0", "id": request_id, "method": method })
        } else {
            serde_json::json!({ "jsonrpc": "2.0", "id": request_id, "method": method, "params": params })
        };
        let request_bytes =
            serde_json::to_vec(&request_json).map_err(|e| format!("json: {e}"))?;
        let request_frame =
            encode_frame(FrameType::Request, &request_bytes, MAX_FRAME_BYTES)
                .map_err(|e| format!("encode: {e}"))?;
        self.stdin
            .write_all(&request_frame)
            .map_err(|e| format!("write request: {e}"))?;

        // Read responses
        for _ in 0..3 {
            let stdout: &mut dyn Read = &mut *self.stdout;
            let frames = read_frames(&mut self.decoder, stdout)?;
            for frame in &frames {
                let Some(value) = frame.as_json() else { continue };

                if frame.frame_type == FrameType::Notification {
                    let method = value["method"].as_str().unwrap_or("");
                    let msg = value["params"]["message"].as_str().unwrap_or("");
                    eprintln!("[host] <<< notification: {method} message=\"{msg}\"");
                    continue;
                }

                if value["id"].as_str() == Some(request_id) {
                    if let Some(result) = value.get("result") {
                        eprintln!("[host] ← response: method={method} result={result}");
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

    /// Sends $/deactivate Request → $/exit Notification → waits for child exit.
    pub fn shutdown(mut self) -> Result<(), String> {
        // Send $/deactivate
        let deact_json = serde_json::json!({ "jsonrpc": "2.0", "id": "h:deact", "method": "$/deactivate", "params": { "reason": "shutdown" } });
        let deact_bytes = serde_json::to_vec(&deact_json).map_err(|e| format!("json: {e}"))?;
        let deact_frame = encode_frame(FrameType::Request, &deact_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;
        self.stdin.write_all(&deact_frame).map_err(|e| format!("write deactivate: {e}"))?;

        // Read deactivate response
        let frames = read_frames(&mut self.decoder, &mut self.stdout)?;
        let deact_ok = frames.iter().any(|f| f.as_json().map(|v| v["id"] == "h:deact").unwrap_or(false));
        if deact_ok {
            eprintln!("[host] $/deactivate acknowledged");
        }

        // Send $/exit
        let exit_json = serde_json::json!({ "jsonrpc": "2.0", "method": "$/exit" });
        let exit_bytes = serde_json::to_vec(&exit_json).map_err(|e| format!("json: {e}"))?;
        let exit_frame =
            encode_frame(FrameType::Notification, &exit_bytes, MAX_FRAME_BYTES)
                .map_err(|e| format!("encode: {e}"))?;
        self.stdin
            .write_all(&exit_frame)
            .map_err(|e| format!("write exit: {e}"))?;

        let status = self.child.wait().map_err(|e| format!("wait: {e}"))?;
        eprintln!("[host] plugin exited: {status}");
        Ok(())
    }
}

/// Reads available frames from stdout using the incremental decoder.
fn read_frames(decoder: &mut FrameDecoder, stdout: &mut dyn Read) -> Result<Vec<ParsedFrame>, String> {
    let mut buf = [0u8; 65536];
    let n = stdout.read(&mut buf).map_err(|e| format!("read: {e}"))?;
    if n == 0 {
        return Ok(Vec::new());
    }
    decoder
        .decode_chunk(&buf[..n])
        .map_err(|e| format!("decode: {e}"))
        .map(|frames| frames.into_iter().map(ParsedFrame::from).collect())
}

/// Wrapper that caches JSON parsing for a frame.
struct ParsedFrame {
    frame_type: FrameType,
    json: Option<serde_json::Value>,
}

impl ParsedFrame {
    fn as_json(&self) -> Option<&serde_json::Value> {
        self.json.as_ref()
    }
}

impl From<ora_plugin_protocol::frame::Frame> for ParsedFrame {
    fn from(f: ora_plugin_protocol::frame::Frame) -> Self {
        let json = serde_json::from_slice(&f.payload).ok();
        Self {
            frame_type: f.frame_type,
            json,
        }
    }
}
