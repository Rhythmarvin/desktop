import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { createMemento } from "../../src/api/memento.js";
/** Fake RpcClient that captures notify() calls. */
class FakeRpc {
    notifications = [];
    notify(method, params) {
        this.notifications.push({ method, params });
    }
}
describe("Memento", () => {
    let rpc;
    beforeEach(() => {
        rpc = new FakeRpc();
    });
    it("get() returns cached value", () => {
        const m = createMemento({ lastCity: "北京" }, rpc);
        assert.strictEqual(m.get("lastCity"), "北京");
    });
    it("get() returns defaultValue for missing key", () => {
        const m = createMemento({}, rpc);
        assert.strictEqual(m.get("nonexistent", "default"), "default");
    });
    it("get() without defaultValue returns undefined for missing key", () => {
        const m = createMemento({}, rpc);
        assert.strictEqual(m.get("nonexistent"), undefined);
    });
    it("update() modifies cache then notifies Host", async () => {
        const m = createMemento({}, rpc);
        await m.update("key", "value");
        assert.strictEqual(m.get("key"), "value");
        assert.strictEqual(rpc.notifications.length, 1);
        assert.strictEqual(rpc.notifications[0].method, "ora.storage.set");
        assert.deepStrictEqual(rpc.notifications[0].params, { key: "key", value: "value" });
    });
    it("get() after update() (same tick) sees new value", () => {
        const m = createMemento({}, rpc);
        m.update("key", "new");
        assert.strictEqual(m.get("key"), "new");
    });
    it("update() with undefined throws", async () => {
        const m = createMemento({}, rpc);
        await assert.rejects(() => m.update("key", undefined), { message: /Cannot store undefined/ });
    });
    it("concurrent updates to same key", async () => {
        const m = createMemento({ count: 0 }, rpc);
        await Promise.all([
            m.update("count", 1),
            m.update("count", 2),
        ]);
        assert.strictEqual(rpc.notifications.length, 2);
    });
    it("keys() returns all cached keys", () => {
        const m = createMemento({ a: 1, b: 2 }, rpc);
        const keys = m.keys();
        assert.ok(keys.includes("a"));
        assert.ok(keys.includes("b"));
        assert.strictEqual(keys.length, 2);
    });
    it("keys() on empty cache returns empty array", () => {
        const m = createMemento({}, rpc);
        assert.deepStrictEqual(m.keys(), []);
    });
});
