// examples/demo-plugin/index.ts
// Minimal plugin using @ora-space/plugin-sdk to demonstrate the SDK API.
//
// The bootstrap handles $/initialize handshake and built-in handlers
// (ping/pong, $/hello notification, $/exit). This file is loaded by
// the bootstrap after handshake and its activate() is called.
//
// Usage: The server points to this directory via the REST API, and the
// bootstrap runs bun on this file as the plugin entry.

import { definePlugin } from "@ora-space/plugin-sdk";
import type { ExtensionContext } from "@ora-space/plugin-sdk";

export default definePlugin({
  activate(ctx: ExtensionContext) {
    ctx.logger.info(`plugin activated: id=${ctx.extensionId} session=${ctx.sessionId}`);
  },

  deactivate() {
    // Cleanup goes here. ctx.subscriptions are auto-disposed by the bootstrap.
  },
});
