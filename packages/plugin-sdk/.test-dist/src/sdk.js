import "./console-guard.js";
import { Transport } from "./rpc/transport.js";
import { RpcClient, setGrantedCapabilities, getGrantedCapabilities } from "./rpc/client.js";
import { sendReady, createInitWaiter, createActivateWaiter, } from "./lifecycle/handshake.js";
import { activatePlugin } from "./lifecycle/activate.js";
import { ProjectAPI } from "./api/project.js";
import { TaskAPI } from "./api/task.js";
import { WindowAPI } from "./api/window.js";
import { CommandsAPI } from "./api/commands.js";
import { FsAPI } from "./api/fs.js";
import { NetworkAPI } from "./api/network.js";
import { ClipboardAPI } from "./api/clipboard.js";
// ── Module-level state ──────────────────────────────────────────────
/** The assembled ora API object, populated by bootstrap(). */
let ora;
/** The plugin module loaded during activation (for deactivate access). */
let pluginModule = null;
/** The assembled extension context (for subscription cleanup). */
let pluginContext = null;
// ── Public API ──────────────────────────────────────────────────────
/**
 * Return the populated ora API object.
 * Plugins access this via `import { ora } from "@ora-space/plugin-sdk"`.
 */
export function getOra() {
    return ora;
}
/**
 * Bootstrap the SDK: handshake → activate → ready.
 *
 * This is called once by the entry script (`ora-plugin-entry.ts`) when
 * the Bun process starts. It does not return until stdin closes (EOF).
 */
export async function bootstrap() {
    // 1. Initialize Transport + RPC
    const transport = new Transport();
    transport.start();
    const rpc = new RpcClient(transport);
    // 2. Handshake step 1: notify Host we're ready
    sendReady(rpc);
    // 3. Handshake step 2: wait for $/init
    let initResult;
    const initWaiter = createInitWaiter();
    const unsubInit = transport.onMessage((msg) => {
        if (initWaiter.handler(msg)) {
            unsubInit();
        }
    });
    try {
        initResult = await initWaiter.promise;
    }
    catch (error) {
        // $/init validation failed — respond with error to the request that failed
        transport.send({
            jsonrpc: "2.0",
            id: 0,
            error: { code: -32602, message: error.message },
        });
        return;
    }
    // Save capabilities for pre-check
    setGrantedCapabilities(initResult.capabilities);
    // Respond to $/init
    transport.send({ jsonrpc: "2.0", id: 0, result: null });
    // 4. Handshake step 3: wait for ora.extension.activate
    const activateWaiter = createActivateWaiter(initResult);
    const unsubActivate = transport.onMessage((msg) => {
        if (activateWaiter.handler(msg)) {
            unsubActivate();
        }
    });
    const entryPath = await activateWaiter.promise;
    // 5. Load and activate plugin code
    try {
        const result = await activatePlugin(entryPath, initResult, rpc);
        pluginModule = result.module;
        pluginContext = result.context;
        // Respond success to activate request
        transport.send({ jsonrpc: "2.0", id: 1, result: null });
    }
    catch (error) {
        // Activation failed
        transport.send({
            jsonrpc: "2.0",
            id: 1,
            error: { code: -32603, message: error.message },
        });
        return;
    }
    // 6. Build ora API and register deactivate listener
    ora = buildOraAPI(rpc);
    transport.onMessage(async (msg) => {
        const m = msg;
        if (m.method === "ora.extension.deactivate" && typeof m.id === "number") {
            const deactivateId = m.id;
            // Step 1: call module.deactivate() if it exists
            if (pluginModule && typeof pluginModule.deactivate === "function") {
                try {
                    await pluginModule.deactivate();
                }
                catch (err) {
                    // Log but continue with cleanup
                    const message = `[plugin:error] deactivate threw: ${err.message}\n`;
                    process.stderr.write(message);
                }
            }
            // Step 2: dispose all subscriptions
            if (pluginContext) {
                for (const d of pluginContext.subscriptions) {
                    try {
                        d.dispose();
                    }
                    catch (err) {
                        const message = `[plugin:error] dispose threw: ${err.message}\n`;
                        process.stderr.write(message);
                    }
                }
            }
            // Step 3: destroy RPC (rejects pending requests)
            rpc.destroy();
            // Step 4: respond to Host
            transport.send({ jsonrpc: "2.0", id: deactivateId, result: null });
        }
    });
}
// ── Internal helpers ────────────────────────────────────────────────
/**
 * Build the OraAPI object, conditionally including API facades based on
 * the granted capabilities.
 */
function buildOraAPI(rpc) {
    const caps = getGrantedCapabilities();
    const api = {};
    // commands — always available
    api.commands = new CommandsAPI(rpc);
    // project — requires project.read or project.write
    if (caps.has("project.read") || caps.has("project.write")) {
        api.project = new ProjectAPI(rpc);
    }
    // task — requires task.read or task.write
    if (caps.has("task.read") || caps.has("task.write")) {
        api.task = new TaskAPI(rpc);
    }
    // window — requires any window-related capability
    if (caps.has("notification.show") ||
        caps.has("window.showQuickPick") ||
        caps.has("window.showInputBox")) {
        api.window = new WindowAPI(rpc);
    }
    // fs — requires fs.read or fs.write
    if (caps.has("fs.read") || caps.has("fs.write")) {
        api.fs = new FsAPI(rpc);
    }
    // network — requires network.fetch
    if (caps.has("network.fetch")) {
        api.network = new NetworkAPI(rpc);
    }
    // clipboard — requires clipboard.read or clipboard.write
    if (caps.has("clipboard.read") || caps.has("clipboard.write")) {
        api.clipboard = new ClipboardAPI(rpc);
    }
    return api;
}
