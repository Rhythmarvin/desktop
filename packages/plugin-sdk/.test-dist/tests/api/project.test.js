import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { ProjectAPI } from "../../src/api/project.js";
/** Fake RpcClient that captures call() invocations. */
class FakeRpc {
    calls = [];
    nextResult = null;
    call(method, params) {
        this.calls.push({ method, params });
        return Promise.resolve(this.nextResult);
    }
    notify(_method, _params) { }
    destroy() { }
}
describe("ProjectAPI", () => {
    let rpc;
    let api;
    beforeEach(() => {
        rpc = new FakeRpc();
        api = new ProjectAPI(rpc);
    });
    it("list() delegates to ora.project.list", async () => {
        rpc.nextResult = { projects: [] };
        const result = await api.list();
        assert.strictEqual(rpc.calls.length, 1);
        assert.strictEqual(rpc.calls[0].method, "ora.project.list");
        assert.deepStrictEqual(rpc.calls[0].params, {});
        assert.deepStrictEqual(result, { projects: [] });
    });
    it("get() delegates with projectId", async () => {
        await api.get("proj-1");
        assert.strictEqual(rpc.calls.length, 1);
        assert.strictEqual(rpc.calls[0].method, "ora.project.get");
        assert.deepStrictEqual(rpc.calls[0].params, { projectId: "proj-1" });
    });
    it("create() delegates with params", async () => {
        await api.create({ name: "test" });
        assert.strictEqual(rpc.calls.length, 1);
        assert.strictEqual(rpc.calls[0].method, "ora.project.create");
        assert.deepStrictEqual(rpc.calls[0].params, { name: "test" });
    });
    it("update() delegates with projectId merged", async () => {
        await api.update("proj-1", { name: "updated" });
        assert.strictEqual(rpc.calls.length, 1);
        assert.strictEqual(rpc.calls[0].method, "ora.project.update");
        assert.deepStrictEqual(rpc.calls[0].params, { projectId: "proj-1", name: "updated" });
    });
    it("delete() delegates with projectId", async () => {
        await api.delete("proj-1");
        assert.strictEqual(rpc.calls.length, 1);
        assert.strictEqual(rpc.calls[0].method, "ora.project.delete");
        assert.deepStrictEqual(rpc.calls[0].params, { projectId: "proj-1" });
    });
});
