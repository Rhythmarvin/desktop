import { createMemento } from "../api/memento.js";
import { createSecretsStub } from "../api/secrets.js";
import { pathToFileURL } from "node:url";
/**
 * Load the plugin entry script, assemble an ExtensionContext, and call activate().
 *
 * @param entryPath  Absolute path to the plugin's entry script (e.g., `/path/to/index.ts`).
 * @param initResult Data received from Host during `$/init`.
 * @param rpc        RPC client for state management notifications.
 * @returns The loaded module and assembled context.
 */
export async function activatePlugin(entryPath, initResult, rpc) {
    // Step 1: dynamic import — plugin code enters memory for the first time
    // Use pathToFileURL for cross-platform compatibility (Windows requires file:// URLs)
    const module = await import(pathToFileURL(entryPath).href);
    // Step 2: assemble ExtensionContext
    const context = {
        extensionId: initResult.pluginId,
        extensionPath: initResult.pluginPath,
        globalState: createMemento(initResult.globalState, rpc),
        workspaceState: createMemento({}, rpc),
        subscriptions: [],
        secrets: createSecretsStub(),
    };
    // Step 3: call activate() if it exists (always await — compatible with sync and async)
    if (typeof module.activate === "function") {
        await module.activate(context);
    }
    return { module: module, context };
}
