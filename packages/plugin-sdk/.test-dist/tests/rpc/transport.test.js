import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { Transport } from "../../src/rpc/transport.js";
describe("Transport", () => {
    it("onMessage() returns an unsubscribe function", () => {
        const transport = new Transport();
        const handler = (_msg) => { };
        const unsub = transport.onMessage(handler);
        assert.strictEqual(typeof unsub, "function");
        unsub();
    });
    it("onClose() returns an unsubscribe function", () => {
        const transport = new Transport();
        const handler = () => { };
        const unsub = transport.onClose(handler);
        assert.strictEqual(typeof unsub, "function");
        unsub();
    });
    it("onError() returns an unsubscribe function", () => {
        const transport = new Transport();
        const handler = (_err) => { };
        const unsub = transport.onError(handler);
        assert.strictEqual(typeof unsub, "function");
        unsub();
    });
});
