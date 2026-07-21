import { describe, it } from "node:test";
import assert from "node:assert/strict";
// Import console-guard triggers the side-effect (no manual call needed)
await import("../src/console-guard.js");
/** Create a mock function that records calls. */
function createMock() {
    const calls = [];
    const fn = (...args) => {
        calls.push(args);
    };
    return { fn, calls };
}
describe("console-guard", () => {
    it("console.log redirects to stderr with [plugin] prefix", () => {
        const mock = createMock();
        process.stderr.write = mock.fn;
        console.log("hello");
        assert.strictEqual(mock.calls.length, 1);
        const output = mock.calls[0][0];
        assert.ok(output.includes("[plugin] hello"));
        assert.ok(output.endsWith("\n"));
    });
    it("console.warn redirects to stderr with [plugin:warn] prefix", () => {
        const mock = createMock();
        process.stderr.write = mock.fn;
        console.warn("warning");
        assert.strictEqual(mock.calls.length, 1);
        const output = mock.calls[0][0];
        assert.ok(output.includes("[plugin:warn] warning"));
    });
    it("console.error redirects to stderr with [plugin:error] prefix", () => {
        const mock = createMock();
        process.stderr.write = mock.fn;
        console.error("error");
        assert.strictEqual(mock.calls.length, 1);
        const output = mock.calls[0][0];
        assert.ok(output.includes("[plugin:error] error"));
    });
    it("object arguments are JSON serialized", () => {
        const mock = createMock();
        process.stderr.write = mock.fn;
        console.log({ key: "value" });
        assert.strictEqual(mock.calls.length, 1);
        const output = mock.calls[0][0];
        assert.ok(output.includes('{"key":"value"}'));
    });
    it("multiple arguments are joined with spaces", () => {
        const mock = createMock();
        process.stderr.write = mock.fn;
        console.log("a", "b", 123);
        assert.strictEqual(mock.calls.length, 1);
        const output = mock.calls[0][0];
        assert.ok(output.includes("a b 123"));
    });
    it("stdout.write is not affected", () => {
        assert.notStrictEqual(process.stdout.write, process.stderr.write);
    });
});
