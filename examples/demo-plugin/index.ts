// examples/demo-plugin/index.ts
// Entry point: `bun run examples/demo-plugin/index.ts`
//
// definePlugin() is called BEFORE bootstrap auto-runs, so handlers
// are registered before the Host sends any requests.
//
// The bootstrap is imported for its side effect (auto-runs transport).

import { definePlugin } from "../../packages/plugin-sdk/src/index.js";
import type { ExtensionContext } from "../../packages/plugin-sdk/src/index.js";

// Side-effect import: bootstrap auto-runs transport
import "../../packages/plugin-sdk/src/bootstrap/main.js";

definePlugin({
  activate(ctx: ExtensionContext) {
    ctx.logger.info(`plugin activated: id=${ctx.extensionId}`);

    ctx.registerHandler("getInfo", async () => {
      return { info: "This is a demo plugin!" };
    });
  },
});
