/**
 * Built-in entry point for plugin processes.
 *
 * The Host spawns: `bun run <sdk-path>/ora-plugin-entry.ts`
 * This script bootstraps the SDK, which handles the handshake protocol
 * and then loads the actual plugin code.
 */
import "./console-guard.js";
import { bootstrap } from "./sdk.js";
bootstrap().catch((error) => {
    const message = `[plugin:error] SDK bootstrap failed: ${error instanceof Error ? error.message : String(error)}\n`;
    process.stderr.write(message);
    process.exit(1);
});
