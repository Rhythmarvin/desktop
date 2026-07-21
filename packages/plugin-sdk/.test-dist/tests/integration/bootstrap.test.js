import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
describe("SDK Integration — protocol sequence", () => {
    let tmpDir;
    let pluginPath;
    beforeEach(() => {
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ora-integration-"));
        pluginPath = path.join(tmpDir, "test-plugin.ts");
        fs.writeFileSync(pluginPath, `
      export async function activate(context) {
        context.subscriptions.push({ dispose() { /* cleaned */ } });
        context.globalState.update("activated", true);
      }
      export async function deactivate() {
        // manual cleanup
      }
    `);
    });
    afterEach(() => {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    });
    it("full handshake works: ready → init → activate → deactivate", async () => {
        const { Transport } = await import("../../src/rpc/transport.js");
        const { RpcClient, setGrantedCapabilities } = await import("../../src/rpc/client.js");
        const { sendReady, createInitWaiter, createActivateWaiter } = await import("../../src/lifecycle/handshake.js");
        const { activatePlugin } = await import("../../src/lifecycle/activate.js");
        const { ProjectAPI } = await import("../../src/api/project.js");
        const { CommandsAPI } = await import("../../src/api/commands.js");
        const transport = new Transport();
        // Skip transport.start() in tests — it would block on real stdin.
        // Tests exercise the protocol via direct handler invocations.
        const rpc = new RpcClient(transport);
        sendReady(rpc);
        const initWaiter = createInitWaiter();
        const initPromise = initWaiter.promise;
        const consumed = initWaiter.handler({
            id: 0,
            method: "$/init",
            params: {
                pluginId: "test-plugin",
                pluginPath: tmpDir,
                entry: "test-plugin.ts",
                capabilities: ["project.read", "notification.show"],
                globalState: { lastCity: "北京" },
            },
        });
        assert.strictEqual(consumed, true);
        const initResult = await initPromise;
        assert.strictEqual(initResult.pluginId, "test-plugin");
        assert.ok(initResult.capabilities.includes("project.read"));
        setGrantedCapabilities(initResult.capabilities);
        const activateWaiter = createActivateWaiter(initResult);
        const activatePromise = activateWaiter.promise;
        const consumed2 = activateWaiter.handler({
            id: 1,
            method: "ora.extension.activate",
            params: {},
        });
        assert.strictEqual(consumed2, true);
        const entryPath = await activatePromise;
        assert.strictEqual(entryPath, `${tmpDir}/test-plugin.ts`);
        const { module, context } = await activatePlugin(entryPath, initResult, rpc);
        assert.strictEqual(context.extensionId, "test-plugin");
        assert.strictEqual(context.subscriptions.length, 1);
        assert.strictEqual(context.globalState.get("activated"), true);
        const api = new ProjectAPI(rpc);
        assert.strictEqual(typeof api.list, "function");
        const commands = new CommandsAPI(rpc);
        const disposable = commands.register("test.hello", () => { });
        assert.strictEqual(typeof disposable.dispose, "function");
        if (typeof module.deactivate === "function") {
            await module.deactivate();
        }
        for (const d of context.subscriptions) {
            d.dispose();
        }
        rpc.destroy();
    });
    it("ora API is assembled with correct capability gating", async () => {
        const { setGrantedCapabilities, getGrantedCapabilities } = await import("../../src/rpc/client.js");
        const { Transport } = await import("../../src/rpc/transport.js");
        const { RpcClient } = await import("../../src/rpc/client.js");
        const transport = new Transport();
        const rpc = new RpcClient(transport);
        setGrantedCapabilities(["project.read", "notification.show"]);
        const caps = getGrantedCapabilities();
        assert.strictEqual(caps.has("project.read"), true);
        assert.strictEqual(caps.has("network.fetch"), false);
        const { ProjectAPI } = await import("../../src/api/project.js");
        const projectApi = new ProjectAPI(rpc);
        assert.strictEqual(typeof projectApi.list, "function");
        assert.throws(() => rpc.call("ora.network.fetch", { url: "https://example.com" }), /not granted/);
    });
});
