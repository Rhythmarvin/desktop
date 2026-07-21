import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { activatePlugin } from "../../src/lifecycle/activate.js";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
/** Fake RpcClient for activate tests. */
class FakeRpc {
    notifications = [];
    notify(method, params) {
        this.notifications.push({ method, params });
    }
    call(_method, _params) {
        return Promise.resolve(null);
    }
    destroy() { }
}
describe("activatePlugin", () => {
    let tmpDir;
    let initResult;
    let rpc;
    beforeEach(() => {
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ora-activate-test-"));
        rpc = new FakeRpc();
        initResult = {
            pluginId: "test-plugin",
            pluginPath: tmpDir,
            entry: "plugin.ts",
            capabilities: [],
            globalState: {},
        };
    });
    afterEach(() => {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    });
    it("loads valid module and calls activate", async () => {
        const entryPath = path.join(tmpDir, "plugin.mjs");
        fs.writeFileSync(entryPath, `
      export async function activate(context) {
        context.subscriptions.push({ dispose() {} });
      }
    `);
        initResult.entry = "plugin.mjs";
        const { context } = await activatePlugin(entryPath, initResult, rpc);
        assert.strictEqual(context.extensionId, "test-plugin");
        assert.strictEqual(context.extensionPath, tmpDir);
        assert.strictEqual(context.subscriptions.length, 1);
    });
    it("plugin without activate succeeds silently", async () => {
        const entryPath = path.join(tmpDir, "noact.mjs");
        fs.writeFileSync(entryPath, `
      export const name = "no-activate-plugin";
    `);
        initResult.entry = "noact.mjs";
        const { context } = await activatePlugin(entryPath, initResult, rpc);
        assert.strictEqual(context.extensionId, "test-plugin");
    });
    it("file not found throws", async () => {
        const entryPath = path.join(tmpDir, "nonexistent.mjs");
        await assert.rejects(activatePlugin(entryPath, initResult, rpc));
    });
    it("syntax error in plugin code throws", async () => {
        const entryPath = path.join(tmpDir, "broken.mjs");
        fs.writeFileSync(entryPath, `
      export default { this is broken syntax !!!
    `);
        initResult.entry = "broken.mjs";
        await assert.rejects(activatePlugin(entryPath, initResult, rpc));
    });
    it("module.activate() that throws propagates the error", async () => {
        const entryPath = path.join(tmpDir, "thrower.mjs");
        fs.writeFileSync(entryPath, `
      export async function activate(context) {
        throw new Error("intentional failure");
      }
    `);
        initResult.entry = "thrower.mjs";
        await assert.rejects(activatePlugin(entryPath, initResult, rpc), { message: /intentional failure/ });
    });
    it("empty module (no exports) succeeds", async () => {
        const entryPath = path.join(tmpDir, "empty.mjs");
        fs.writeFileSync(entryPath, `
      // empty module — no exports at all
    `);
        initResult.entry = "empty.mjs";
        const { context } = await activatePlugin(entryPath, initResult, rpc);
        assert.strictEqual(context.extensionId, "test-plugin");
    });
});
