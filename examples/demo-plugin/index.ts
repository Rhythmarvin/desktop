// examples/demo-plugin/index.ts
// Entry point: `bun run examples/demo-plugin/index.ts`
//
// IMPORTANT: definePlugin() must run BEFORE the bootstrap starts.
// We import runBootstrap explicitly and call it after definePlugin().

import { definePlugin } from "../../packages/plugin-sdk/src/index.js";
import type { ExtensionContext } from "../../packages/plugin-sdk/src/index.js";
import { runBootstrap } from "../../packages/plugin-sdk/src/bootstrap/main.js";

definePlugin({
  activate(ctx: ExtensionContext) {
    ctx.logger.info(`plugin activated: id=${ctx.extensionId}`);

    ctx.registerHandler("getInfo", async () => {
      return { info: "This is a demo plugin!" };
    });
  },
});

// Start transport AFTER handlers are registered
runBootstrap();
