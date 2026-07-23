use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use ora_plugin_protocol::frame::{FrameDecoder, FrameType, MAX_FRAME_BYTES, encode_frame};
use ora_plugin_protocol::lifecycle::InitializeParams;
use tokio::sync::{mpsc, oneshot};

use crate::PluginEvent;

/// Owns one Bun plugin child process and its stdin/stdout pipes.
///
/// After the `$/initialize` handshake a background reader thread is spawned that
/// continuously reads stdout, decodes 5-byte frames, and routes them to pending
/// callers via [`sync_pending`] (oneshot for `invoke()`) or [`stream_pending`]
/// (mpsc for `invoke_streaming()`).
pub struct PluginProcess {
    child: Child,
    stdin: Arc<Mutex<Box<dyn Write + Send>>>,
    sync_pending: Arc<Mutex<std::collections::HashMap<String, SyncWaiter>>>,
    stream_pending: Arc<Mutex<std::collections::HashMap<String, mpsc::UnboundedSender<PluginEvent>>>>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
    reader_stop: Arc<AtomicBool>,
    activated: bool,
}

/// A waiting caller for a synchronous invoke().
enum SyncWaiter {
    Waiting(oneshot::Sender<Result<serde_json::Value, String>>),
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
    /// Uses the reader thread for response routing.
    pub fn activate(&mut self) -> Result<(), String> {
        self.inner
            .as_mut()
            .ok_or("process already stopped")?
            .activate()
    }

    /// Sends a JSON-RPC Request and waits synchronously for the Response.
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

    /// Sends a JSON-RPC Request and returns a streaming event receiver.
    ///
    /// The caller receives [`PluginEvent`]s via the returned unbounded channel
    /// as the plugin pushes `acp/event` Notifications.
    pub fn invoke_streaming(
        &mut self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<mpsc::UnboundedReceiver<PluginEvent>, String> {
        self.inner
            .as_mut()
            .ok_or("process already stopped")?
            .invoke_streaming(request_id, method, params)
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
    /// Spawns Bun, completes $/initialize handshake, and starts the
    /// background reader thread.
    ///
    /// The plugin is left in the "awaitingActivate" phase until a subsequent
    /// call to [`activate`](Self::activate).
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

        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;

        let stdin = Arc::new(Mutex::new(Box::new(stdin) as Box<dyn Write + Send>));

        // ── $/initialize handshake ─────────────────────────────────────
        let init_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "h:1",
            "method": "$/initialize",
            "params": init_params,
        });
        let init_bytes = serde_json::to_vec(&init_json).map_err(|e| format!("json: {e}"))?;
        let init_frame = encode_frame(FrameType::Request, &init_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;
        {
            let mut stdin_lock = stdin.lock().unwrap();
            stdin_lock.write_all(&init_frame).map_err(|e| format!("write init: {e}"))?;
        }

        // Give the plugin process a moment to boot its runtime before reading
        // the handshake response. Agent plugins spawn a child process during
        // startup, which adds latency.
        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES).map_err(|e| format!("decoder: {e}"))?;
        let mut stdout_reader = stdout;
        let frames = read_frames_direct(&mut decoder, &mut stdout_reader)?;

        let init_ok = frames.iter().any(|f| {
            f.frame_type == FrameType::Response
                && f.as_json().map(|v| v["id"] == "h:1").unwrap_or(false)
        });

        if !init_ok {
            return Err("handshake failed: no initialize response".into());
        }

        // ── Start background reader thread ─────────────────────────────
        let sync_pending: Arc<Mutex<std::collections::HashMap<String, SyncWaiter>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        let stream_pending: Arc<Mutex<std::collections::HashMap<String, mpsc::UnboundedSender<PluginEvent>>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        let reader_stop = Arc::new(AtomicBool::new(false));

        let reader_sync = sync_pending.clone();
        let reader_stream = stream_pending.clone();
        let reader_stop_clone = reader_stop.clone();

        let reader_handle = std::thread::spawn(move || {
            reader_loop(stdout_reader, decoder, reader_sync, reader_stream, reader_stop_clone);
        });

        Ok(Self {
            child,
            stdin,
            sync_pending,
            stream_pending,
            reader_handle: Some(reader_handle),
            reader_stop,
            activated: false,
        })
    }

    /// Sends $/activate Request. The response is routed via sync_pending
    /// by the background reader thread.
    pub fn activate(&mut self) -> Result<(), String> {
        if self.activated {
            return Err("plugin already activated".into());
        }

        let request_id = "h:2".to_string();

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.sync_pending.lock().unwrap();
            pending.insert(request_id.clone(), SyncWaiter::Waiting(tx));
        }

        let act_json = serde_json::json!({
            "jsonrpc": "2.0", "id": &request_id, "method": "$/activate",
            "params": { "reason": "manualStart" }
        });
        let act_bytes = serde_json::to_vec(&act_json).map_err(|e| format!("json: {e}"))?;
        let act_frame = encode_frame(FrameType::Request, &act_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;

        {
            let mut stdin_lock = self.stdin.lock().unwrap();
            stdin_lock.write_all(&act_frame).map_err(|e| format!("write activate: {e}"))?;
        }

        // Block until reader thread sends the response
        match rx.blocking_recv() {
            Ok(Ok(_result)) => {
                self.activated = true;
                Ok(())
            }
            Ok(Err(e)) => {
                self.sync_pending.lock().unwrap().remove(&request_id);
                Err(format!("activate: {e}"))
            }
            Err(_) => {
                self.sync_pending.lock().unwrap().remove(&request_id);
                Err("activate: reader thread dropped".into())
            }
        }
    }

    /// Sends a JSON-RPC Request and blocks until the matched Response arrives.
    pub fn invoke(
        &mut self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, String> {
        eprintln!("[host] → invoke: method={method} requestId={request_id}");

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.sync_pending.lock().unwrap();
            pending.insert(request_id.to_string(), SyncWaiter::Waiting(tx));
        }

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

        {
            let mut stdin_lock = self.stdin.lock().unwrap();
            stdin_lock
                .write_all(&request_frame)
                .map_err(|e| format!("write request: {e}"))?;
        }

        // Block until reader thread delivers the matched response or channel closes
        match rx.blocking_recv() {
            Ok(Ok(value)) => {
                eprintln!("[host] ← response: method={method} result={value}");
                Ok(InvokeResult {
                    request_id: request_id.to_string(),
                    result: value,
                })
            }
            Ok(Err(e)) => {
                self.sync_pending.lock().unwrap().remove(request_id);
                Err(e)
            }
            Err(_) => {
                self.sync_pending.lock().unwrap().remove(request_id);
                Err("invoke: reader thread dropped".into())
            }
        }
    }

    /// Sends a JSON-RPC Request and immediately returns an event stream.
    ///
    /// The returned unbounded receiver yields [`PluginEvent`]s as the plugin
    /// sends `acp/event` Notifications. The channel closes when the operation
    /// completes (via `completed` or `error` event) or when the process exits.
    pub fn invoke_streaming(
        &mut self,
        request_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<mpsc::UnboundedReceiver<PluginEvent>, String> {
        eprintln!("[host] → streaming invoke: method={method} requestId={request_id}");

        let (tx, rx) = mpsc::unbounded_channel();
        {
            let mut pending = self.stream_pending.lock().unwrap();
            pending.insert(request_id.to_string(), tx);
        }

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

        {
            let mut stdin_lock = self.stdin.lock().unwrap();
            stdin_lock
                .write_all(&request_frame)
                .map_err(|e| format!("write streaming request: {e}"))?;
        }

        Ok(rx)
    }

    /// Sends $/deactivate → $/exit → stops the reader thread → waits for child exit.
    pub fn shutdown(mut self) -> Result<(), String> {
        // $/deactivate — use sync_pending to wait for ack
        let deact_id = "h:deact".to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.sync_pending.lock().unwrap();
            pending.insert(deact_id.clone(), SyncWaiter::Waiting(tx));
        }

        let deact_json = serde_json::json!({
            "jsonrpc": "2.0", "id": &deact_id, "method": "$/deactivate",
            "params": { "reason": "shutdown" }
        });
        let deact_bytes = serde_json::to_vec(&deact_json).map_err(|e| format!("json: {e}"))?;
        let deact_frame = encode_frame(FrameType::Request, &deact_bytes, MAX_FRAME_BYTES)
            .map_err(|e| format!("encode: {e}"))?;
        {
            let mut stdin_lock = self.stdin.lock().unwrap();
            stdin_lock.write_all(&deact_frame).map_err(|e| format!("write deactivate: {e}"))?;
        }

        // Wait for deactivate response (with timeout via try)
        let _ = rx.blocking_recv();
        eprintln!("[host] $/deactivate acknowledged");

        // $/exit notification
        let exit_json = serde_json::json!({ "jsonrpc": "2.0", "method": "$/exit" });
        let exit_bytes = serde_json::to_vec(&exit_json).map_err(|e| format!("json: {e}"))?;
        let exit_frame =
            encode_frame(FrameType::Notification, &exit_bytes, MAX_FRAME_BYTES)
                .map_err(|e| format!("encode: {e}"))?;
        {
            let mut stdin_lock = self.stdin.lock().unwrap();
            stdin_lock
                .write_all(&exit_frame)
                .map_err(|e| format!("write exit: {e}"))?;
        }

        // Signal reader thread to stop, then join it
        self.reader_stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }

        let status = self.child.wait().map_err(|e| format!("wait: {e}"))?;
        eprintln!("[host] plugin exited: {status}");
        Ok(())
    }
}

// ── Background reader thread ─────────────────────────────────────────────

/// Runs in a dedicated thread: reads stdout, decodes 5-byte frames, and
/// routes each frame to the appropriate pending caller.
fn reader_loop(
    mut stdout: impl Read + Send + 'static,
    mut decoder: FrameDecoder,
    sync_pending: Arc<Mutex<std::collections::HashMap<String, SyncWaiter>>>,
    stream_pending: Arc<Mutex<std::collections::HashMap<String, mpsc::UnboundedSender<PluginEvent>>>>,
    stop: Arc<AtomicBool>,
) {
    let mut buf = [0u8; 65536];
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }

        let n = match stdout.read(&mut buf) {
            Ok(0) => {
                // EOF — plugin process exited
                eprintln!("[host:reader] stdout EOF, exiting reader loop");
                return;
            }
            Ok(n) => n,
            Err(e) => {
                eprintln!("[host:reader] read error: {e}");
                return;
            }
        };

        let frames = match decoder.decode_chunk(&buf[..n]) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[host:reader] decode error: {e}");
                return;
            }
        };

        for frame in frames {
            let frame_type = frame.frame_type;
            let json: Option<serde_json::Value> = serde_json::from_slice(&frame.payload).ok();

            match frame_type {
                FrameType::Response => {
                    let Some(ref value) = json else { continue };
                    let id = value["id"].as_str().unwrap_or("");
                    if let Some(mut pending_lock) = try_lock_sync(&sync_pending) {
                        if let Some(SyncWaiter::Waiting(sender)) = pending_lock.remove(id) {
                            if let Some(result) = value.get("result") {
                                let _ = sender.send(Ok(result.clone()));
                            } else if let Some(error) = value.get("error") {
                                let msg = error["message"].as_str().unwrap_or("unknown");
                                let code = error["code"].as_i64().unwrap_or(0);
                                let _ = sender.send(Err(format!("plugin error: {msg} (code: {code})")));
                            }
                        }
                    }
                    eprintln!("[host:reader] ← response id={id}");
                }
                FrameType::Notification => {
                    let Some(ref value) = json else { continue };
                    let method = value["method"].as_str().unwrap_or("");

                    match method {
                        "acp/event" => {
                            let params = &value["params"];
                            let request_id = params["requestId"].as_str().unwrap_or("");

                            if let Some(mut pending_lock) = try_lock_stream(&stream_pending) {
                                if let Some(sender) = pending_lock.get(request_id) {
                                    let event = parse_acp_event(request_id, params);
                                    if sender.send(event).is_err() {
                                        // Receiver dropped — clean up
                                        pending_lock.remove(request_id);
                                    }
                                }
                            }
                        }
                        _ => {
                            let msg = value["params"]["message"].as_str().unwrap_or("");
                            eprintln!("[host:reader] <<< notification: {method} message=\"{msg}\"");
                        }
                    }
                }
                FrameType::Request => {
                    // Plugin-originated requests are not expected in current MVP.
                    eprintln!("[host:reader] unexpected Request frame from plugin");
                }
            }
        }
    }
}

/// Attempts to lock the sync_pending map, logging on contention.
fn try_lock_sync(
    map: &Arc<Mutex<std::collections::HashMap<String, SyncWaiter>>>,
) -> Option<std::sync::MutexGuard<'_, std::collections::HashMap<String, SyncWaiter>>> {
    match map.lock() {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("[host:reader] sync_pending lock poisoned: {e}");
            None
        }
    }
}

/// Attempts to lock the stream_pending map.
fn try_lock_stream(
    map: &Arc<Mutex<std::collections::HashMap<String, mpsc::UnboundedSender<PluginEvent>>>>,
) -> Option<std::sync::MutexGuard<'_, std::collections::HashMap<String, mpsc::UnboundedSender<PluginEvent>>>> {
    match map.lock() {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("[host:reader] stream_pending lock poisoned: {e}");
            None
        }
    }
}

/// Parses an `acp/event` notification params into a [`PluginEvent`].
fn parse_acp_event(request_id: &str, params: &serde_json::Value) -> PluginEvent {
    let event_type = params["event"]["type"].as_str().unwrap_or("error");

    match event_type {
        "session_update" => PluginEvent::SessionUpdate {
            request_id: request_id.to_string(),
            update: params["event"]["update"].clone(),
        },
        "permission_request" => PluginEvent::PermissionRequest {
            request_id: request_id.to_string(),
            permission: params["event"]["permission"].clone(),
        },
        "completed" => PluginEvent::Completed {
            request_id: request_id.to_string(),
            result: params["event"].clone(),
        },
        _ => PluginEvent::Error {
            request_id: request_id.to_string(),
            code: params["event"]["code"].as_i64().unwrap_or(-32603) as i32,
            message: params["event"]["message"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string(),
        },
    }
}

// ── Shared helpers ──────────────────────────────────────────────────────

/// Reads available frames from stdout using the incremental decoder.
/// Used during the initial handshake before the reader thread starts.
fn read_frames_direct(decoder: &mut FrameDecoder, stdout: &mut dyn Read) -> Result<Vec<ParsedFrame>, String> {
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
