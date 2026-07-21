import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { sendReady, createInitWaiter, createActivateWaiter } from "../../src/lifecycle/handshake.js";
/** Fake RpcClient that captures notify() calls. */
class FakeRpc {
    notifications = [];
    notify(method, params) {
        this.notifications.push({ method, params });
    }
}
describe("handshake", () => {
    describe("sendReady", () => {
        it("sends plugin.ready notification with sdkVersion", () => {
            const rpc = new FakeRpc();
            sendReady(rpc);
            assert.strictEqual(rpc.notifications.length, 1);
            assert.strictEqual(rpc.notifications[0].method, "plugin.ready");
            assert.ok("sdkVersion" in rpc.notifications[0].params);
            assert.strictEqual(rpc.notifications[0].params.sdkVersion, "0.2.0");
        });
    });
    describe("createInitWaiter", () => {
        let initResult;
        beforeEach(() => {
            initResult = {
                pluginId: "test-plugin",
                pluginPath: "/plugins/test",
                entry: "index.ts",
                capabilities: ["project.read"],
                globalState: { lastCity: "北京" },
            };
        });
        it("resolves on matching $/init message", async () => {
            const { promise, handler } = createInitWaiter();
            const consumed = handler({
                id: 0,
                method: "$/init",
                params: initResult,
            });
            assert.strictEqual(consumed, true);
            const result = await promise;
            assert.strictEqual(result.pluginId, "test-plugin");
            assert.strictEqual(result.pluginPath, "/plugins/test");
            assert.strictEqual(result.entry, "index.ts");
            assert.deepStrictEqual(result.capabilities, ["project.read"]);
            assert.deepStrictEqual(result.globalState, { lastCity: "北京" });
        });
        it("ignores non-matching messages", () => {
            const { handler } = createInitWaiter();
            const consumed = handler({
                id: 1,
                method: "some.other.method",
                params: {},
            });
            assert.strictEqual(consumed, false);
        });
        it("rejects when params is missing", async () => {
            const { promise, handler } = createInitWaiter();
            const consumed = handler({
                id: 0,
                method: "$/init",
            });
            assert.strictEqual(consumed, true);
            await assert.rejects(promise, { message: /missing required field/ });
        });
        it("rejects when pluginId is missing", async () => {
            const { promise, handler } = createInitWaiter();
            const consumed = handler({
                id: 0,
                method: "$/init",
                params: { entry: "index.ts" },
            });
            assert.strictEqual(consumed, true);
            await assert.rejects(promise, { message: /missing required field pluginId/ });
        });
        it("rejects when entry is missing", async () => {
            const { promise, handler } = createInitWaiter();
            const consumed = handler({
                id: 0,
                method: "$/init",
                params: { pluginId: "test" },
            });
            assert.strictEqual(consumed, true);
            await assert.rejects(promise, { message: /missing required field entry/ });
        });
    });
    describe("createActivateWaiter", () => {
        it("resolves with correct entry path", async () => {
            const initResult = {
                pluginId: "test",
                pluginPath: "/plugins/test",
                entry: "main.ts",
                capabilities: [],
                globalState: {},
            };
            const { promise, handler } = createActivateWaiter(initResult);
            const consumed = handler({
                id: 1,
                method: "ora.extension.activate",
                params: {},
            });
            assert.strictEqual(consumed, true);
            const entryPath = await promise;
            assert.strictEqual(entryPath, "/plugins/test/main.ts");
        });
        it("ignores non-matching messages", () => {
            const initResult = {
                pluginId: "test",
                pluginPath: "/p",
                entry: "e.ts",
                capabilities: [],
                globalState: {},
            };
            const { handler } = createActivateWaiter(initResult);
            const consumed = handler({
                id: 2,
                method: "ora.project.list",
                params: {},
            });
            assert.strictEqual(consumed, false);
        });
    });
});
