// demo-plugin.ts — Minimal plugin that demonstrates bidirectional JSON-RPC communication.
//
// This plugin:
//   1. Handles `ping` → returns `pong`
//   2. After receiving `ping`, sends a `$/hello` notification to the Host
//   3. Gracefully exits on `$/exit`
//
// The bootstrap handles $/initialize handshake automatically.
// This file is the "plugin entry" that gets loaded after handshake.

// The console guard is installed by bootstrap before this runs,
// so console.log/warn/error go to stderr, not stdout.

export function activate() {
  // Nothing to initialize for the demo
}

export function deactivate() {
  // Nothing to clean up
}

// Note: the bootstrap auto-runs when executed as `bun run bootstrap/main.ts`.
// The bootstrap loads this file as the plugin entry and calls activate().
// For MVP, handlers are registered by the bootstrap based on exports.
