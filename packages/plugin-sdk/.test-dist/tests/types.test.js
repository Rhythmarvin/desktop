import assert from "node:assert/strict";
import test from "node:test";
test("exports JSON-RPC protocol DTOs with numeric id and generic params/result", () => {
    const request = {
        jsonrpc: "2.0",
        id: 1,
        method: "add",
        params: { a: 1, b: 2 },
    };
    const successResponse = {
        jsonrpc: "2.0",
        id: request.id,
        result: 3,
    };
    const errorResponse = {
        jsonrpc: "2.0",
        id: request.id,
        error: { code: -32601, message: "missing method" },
    };
    assert.equal(successResponse.result, 3);
    assert.equal(errorResponse.error.code, -32601);
});
test("exports ExtensionContext and related types", () => {
    // Type-level test: these imports should compile
    const ctx = {
        extensionId: "test",
        extensionPath: "/test",
        globalState: null,
        workspaceState: null,
        secrets: null,
        subscriptions: [],
    };
    assert.equal(ctx.extensionId, "test");
    assert.ok(Array.isArray(ctx.subscriptions));
});
test("exports ActivateFunction and DeactivateFunction types", () => {
    // Type-level verification: these type references should compile
    const _fn = async (_ctx) => { };
    void _fn; // suppress unused variable
    const _dfn = () => { };
    void _dfn;
    assert.ok(true);
});
test("exports Disposable interface", () => {
    const d = { dispose() { } };
    assert.equal(typeof d.dispose, "function");
});
test("OraAPI type is exported", () => {
    // Type-level verification: OraAPI reference compiles
    const _api = {};
    void _api;
    assert.ok(true);
});
