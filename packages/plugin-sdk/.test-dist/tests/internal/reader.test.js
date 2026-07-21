import { describe, it, afterEach } from "node:test";
import assert from "node:assert/strict";
import { onMessage, onError, onClose, _startListeningWithSource, __resetForTesting, } from "../../src/internal/reader.js";
/**
 * Creates a mock byte source that yields the given chunks then completes (EOF).
 */
function mockSource(...chunks) {
    let index = 0;
    return {
        [Symbol.asyncIterator]() {
            return {
                async next() {
                    if (index < chunks.length) {
                        return { done: false, value: chunks[index++] };
                    }
                    return { done: true, value: undefined };
                },
            };
        },
    };
}
/** Encode a string as a UTF-8 Uint8Array. */
function enc(s) {
    return new TextEncoder().encode(s);
}
/** Collect all calls to a handler into an array. */
function collect() {
    const items = [];
    return {
        items,
        handler: (item) => { items.push(item); },
    };
}
describe("reader (push model)", () => {
    afterEach(() => {
        __resetForTesting();
    });
    it("delivers a valid JSON message to all registered handlers", async () => {
        const source = mockSource(enc('{"method":"test"}\n'));
        const h1 = collect();
        const h2 = collect();
        const unsub1 = onMessage(h1.handler);
        const unsub2 = onMessage(h2.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(h1.items.length, 1);
        assert.deepStrictEqual(h1.items[0], { method: "test" });
        assert.strictEqual(h2.items.length, 1);
        assert.deepStrictEqual(h2.items[0], { method: "test" });
        unsub1();
        unsub2();
    });
    it("unsubscribe stops delivery to that handler", async () => {
        const source = mockSource(enc('{"first":true}\n'), enc('{"second":true}\n'));
        const h = collect();
        const unsub = onMessage(h.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(h.items.length, 2);
        unsub();
    });
    it("unsubscribe mid-delivery stops subsequent deliveries", async () => {
        const source = mockSource(enc('{"msg":1}\n'));
        const h = collect();
        const unsub = onMessage(h.handler);
        unsub();
        await _startListeningWithSource(source);
        assert.strictEqual(h.items.length, 0);
    });
    it("triggers error handlers on invalid JSON", async () => {
        const source = mockSource(enc("not-json\n"));
        const msgs = collect();
        const errs = collect();
        const unsubMsg = onMessage(msgs.handler);
        const unsubErr = onError(errs.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(msgs.items.length, 0);
        assert.strictEqual(errs.items.length, 1);
        assert.ok(errs.items[0] instanceof Error);
        unsubMsg();
        unsubErr();
    });
    it("triggers close handlers on EOF", async () => {
        const source = mockSource(); // empty — EOF immediately
        let closed = false;
        const unsubClose = onClose(() => { closed = true; });
        await _startListeningWithSource(source);
        assert.strictEqual(closed, true);
        unsubClose();
    });
    it("handler that throws does not block other handlers", async () => {
        const source = mockSource(enc('{"ok":true}\n'));
        const h1 = collect();
        const h2 = collect();
        const throwingHandler = (_msg) => {
            throw new Error("boom");
        };
        const unsubThrow = onMessage(throwingHandler);
        const unsub1 = onMessage(h1.handler);
        const unsub2 = onMessage(h2.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(h1.items.length, 1);
        assert.strictEqual(h2.items.length, 1);
        unsubThrow();
        unsub1();
        unsub2();
    });
    it("empty line triggers error handlers", async () => {
        const source = mockSource(enc("\n"), enc('{"after":true}\n'));
        const msgs = collect();
        const errs = collect();
        const unsubMsg = onMessage(msgs.handler);
        const unsubErr = onError(errs.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(errs.items.length, 1);
        assert.strictEqual(msgs.items.length, 1);
        assert.deepStrictEqual(msgs.items[0], { after: true });
        unsubMsg();
        unsubErr();
    });
    it("reassembles UTF-8 multi-byte character split across chunks", async () => {
        // "北" = U+5317 = 0xE5 0x8C 0x97 in UTF-8 (3 bytes)
        // Slice the full encoded message after the 10th byte, which falls
        // after the first byte of "北" (0xE5). The remainder starts with
        // 0x8C 0x97 — the continuation bytes.
        const full = enc('{"city":"北京"}\n');
        const split1 = full.slice(0, 10); // includes partial 北 (just 0xE5)
        const remainder = full.slice(10); // 0x8C 0x97 + rest
        const source = mockSource(split1, remainder);
        const msgs = collect();
        const unsub = onMessage(msgs.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(msgs.items.length, 1);
        assert.deepStrictEqual(msgs.items[0], { city: "北京" });
        unsub();
    });
    it("delivers multiple messages in a single chunk", async () => {
        const source = mockSource(enc('{"a":1}\n{"b":2}\n{"c":3}\n'));
        const msgs = collect();
        const unsub = onMessage(msgs.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(msgs.items.length, 3);
        assert.deepStrictEqual(msgs.items[0], { a: 1 });
        assert.deepStrictEqual(msgs.items[1], { b: 2 });
        assert.deepStrictEqual(msgs.items[2], { c: 3 });
        unsub();
    });
    it("delivers trailing content on EOF without final newline", async () => {
        const source = mockSource(enc('{"trailing":true}'));
        const msgs = collect();
        const unsub = onMessage(msgs.handler);
        await _startListeningWithSource(source);
        assert.strictEqual(msgs.items.length, 1);
        assert.deepStrictEqual(msgs.items[0], { trailing: true });
        unsub();
    });
});
