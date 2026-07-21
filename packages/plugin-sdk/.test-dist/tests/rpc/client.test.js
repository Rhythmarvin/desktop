import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { RpcClient, setGrantedCapabilities } from "../../src/rpc/client.js";
import { Transport } from "../../src/rpc/transport.js";
/**
 * A fake Transport that captures sent messages and allows
 * simulating incoming messages via a stored callback.
 */
class FakeTransport {
    sent = [];
    messageHandler = null;
    start() { }
    send(message) {
        this.sent.push(message);
    }
    onMessage(handler) {
        this.messageHandler = handler;
        return () => { this.messageHandler = null; };
    }
    onClose(_handler) {
        return () => { };
    }
    onError(_handler) {
        return () => { };
    }
    /** Simulate an incoming message from Host. */
    receive(msg) {
        if (this.messageHandler) {
            this.messageHandler(msg);
        }
    }
}
describe("RpcClient", () => {
    let transport;
    let client;
    beforeEach(() => {
        setGrantedCapabilities(["project.read", "project.write", "task.read", "notification.show"]);
        transport = new FakeTransport();
        client = new RpcClient(transport);
    });
    it("call() sends a JSON-RPC request and resolves with result", async () => {
        const promise = client.call("ora.project.list", {});
        assert.strictEqual(transport.sent.length, 1);
        assert.strictEqual(transport.sent[0].jsonrpc, "2.0");
        assert.strictEqual(transport.sent[0].method, "ora.project.list");
        assert.strictEqual(transport.sent[0].id, 1);
        transport.receive({ jsonrpc: "2.0", id: 1, result: { projects: [] } });
        const result = await promise;
        assert.deepStrictEqual(result, { projects: [] });
    });
    it("call() rejects on error response", async () => {
        const promise = client.call("ora.project.list", {});
        transport.receive({
            jsonrpc: "2.0",
            id: 1,
            error: { code: -32601, message: "Method not found" },
        });
        await assert.rejects(promise, { message: /RPC error -32601: Method not found/ });
    });
    it("generates monotonically increasing ids", () => {
        client.call("ora.project.list", {});
        client.call("ora.project.get", { projectId: "x" });
        client.call("ora.task.list", {});
        assert.strictEqual(transport.sent.length, 3);
        assert.strictEqual(transport.sent[0].id, 1);
        assert.strictEqual(transport.sent[1].id, 2);
        assert.strictEqual(transport.sent[2].id, 3);
    });
    it("capability pre-check: authorized method proceeds", () => {
        assert.doesNotThrow(() => client.call("ora.project.list", {}));
    });
    it("capability pre-check: unauthorized method throws synchronously", () => {
        assert.throws(() => client.call("ora.network.fetch", {}), /Capability "network.fetch" is not granted/);
        assert.strictEqual(transport.sent.length, 0);
    });
    it("notify() sends a message without an id field", () => {
        client.notify("ora.storage.set", { key: "x", value: 1 });
        assert.strictEqual(transport.sent.length, 1);
        assert.strictEqual(transport.sent[0].jsonrpc, "2.0");
        assert.strictEqual(transport.sent[0].method, "ora.storage.set");
        assert.deepStrictEqual(transport.sent[0].params, { key: "x", value: 1 });
        assert.ok(!("id" in transport.sent[0]));
    });
    it("notify() with empty params works", () => {
        client.notify("plugin.ready", {});
        assert.strictEqual(transport.sent.length, 1);
        assert.deepStrictEqual(transport.sent[0].params, {});
    });
    it("destroy() rejects all pending requests", async () => {
        const p1 = client.call("ora.project.list", {});
        const p2 = client.call("ora.task.list", {});
        client.destroy();
        await assert.rejects(p1, { message: /Transport closed/ });
        await assert.rejects(p2, { message: /Transport closed/ });
    });
    it("call() after destroy() throws synchronously", () => {
        client.destroy();
        assert.throws(() => client.call("ora.project.list", {}), /RpcClient is destroyed/);
    });
    it("ignores unmatched response ids", async () => {
        const promise = client.call("ora.project.list", {});
        transport.receive({ jsonrpc: "2.0", id: 999, result: "wrong" });
        transport.receive({ jsonrpc: "2.0", id: 1, result: { projects: [] } });
        const result = await promise;
        assert.deepStrictEqual(result, { projects: [] });
    });
    it("ignores messages with a method field (Host-initiated requests)", () => {
        transport.receive({
            jsonrpc: "2.0",
            id: 1,
            method: "$/init",
            params: {},
        });
    });
    it("null result resolves as null (not undefined)", async () => {
        const promise = client.call("ora.project.list", {});
        transport.receive({ jsonrpc: "2.0", id: 1, result: null });
        const result = await promise;
        assert.strictEqual(result, null);
    });
    it("handles 100 concurrent pending requests correctly", async () => {
        const promises = [];
        for (let i = 0; i < 100; i++) {
            promises.push(client.call("ora.project.list", {}));
        }
        assert.strictEqual(transport.sent.length, 100);
        const ids = transport.sent.map((m) => m.id);
        const uniqueIds = new Set(ids);
        assert.strictEqual(uniqueIds.size, 100);
        for (let i = 0; i < 100; i++) {
            transport.receive({ jsonrpc: "2.0", id: ids[i], result: i });
        }
        const results = await Promise.all(promises);
        assert.strictEqual(results.length, 100);
    });
    it("unknown method (no capability mapping) proceeds without check", () => {
        assert.doesNotThrow(() => client.call("some.unknown.method", {}));
        assert.strictEqual(transport.sent.length, 1);
    });
});
