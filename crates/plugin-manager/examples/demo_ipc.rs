// demo_ipc.rs — End-to-end plugin IPC demo.
//
// Usage:
//   cargo run --example demo_ipc -p ora-plugin-manager -- <plugin-entry.ts>

use ora_plugin_manager::PluginRuntime;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let plugin_path = if args.len() >= 2 {
        args[1].clone()
    } else {
        eprintln!("Usage: cargo run --example demo_ipc -p ora-plugin-manager -- <plugin-entry.ts>");
        std::process::exit(1);
    };

    let bun_path = std::env::var("ORA_BUN_PATH").unwrap_or_else(|_| "bun".to_string());

    println!("=== Plugin IPC Demo ===\n");
    println!("bun:    {bun_path}");
    println!("plugin: {plugin_path}\n");

    let runtime = PluginRuntime::new(PathBuf::from(&bun_path), PathBuf::from(&plugin_path));

    // 1. Start
    println!("── 1. Starting plugin ──");
    let start = runtime.start(&plugin_path).unwrap_or_else(|e| panic!("start: {e}"));
    println!("  instanceId: {}", start.instance_id);
    println!("  sessionId:  {}", start.session_id);
    println!("  ✓ handshake complete\n");

    // 2. Ping
    println!("── 2. Sending ping ──");
    let pong = runtime.invoke(&start.instance_id, "ping", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("ping: {e}"));
    println!("  result: {}", pong.result);
    println!("  ✓ pong received\n");

    // 3. getInfo (plugin's own handler)
    println!("── 3. Calling getInfo ──");
    let info = runtime.invoke(&start.instance_id, "getInfo", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("getInfo: {e}"));
    println!("  result: {}", info.result);
    println!("  ✓ getInfo works\n");

    // 4. Wait for hello notification
    println!("── 4. Flush (catch notification) ──");
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _flush = runtime.invoke(&start.instance_id, "ping", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("flush: {e}"));
    println!("  ✓ flush complete\n");

    // 5. Stop
    println!("── 5. Stopping plugin ──");
    runtime.stop(&start.instance_id).unwrap_or_else(|e| panic!("stop: {e}"));
    println!("  ✓ plugin exited\n");

    println!("=== Demo Complete ===");
    println!("  handshake:  ✓");
    println!("  ping/pong:  ✓");
    println!("  getInfo:    ✓");
    println!("  notify:     ✓ (check stderr)");
    println!("  shutdown:   ✓");
}
