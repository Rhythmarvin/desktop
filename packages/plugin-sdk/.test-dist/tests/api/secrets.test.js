import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createSecretsStub } from "../../src/api/secrets.js";
describe("Secrets (MVP stub)", () => {
    it("get() returns undefined", async () => {
        const s = createSecretsStub();
        assert.strictEqual(await s.get("anyKey"), undefined);
    });
    it("store() does not throw", async () => {
        const s = createSecretsStub();
        await assert.doesNotReject(() => s.store("key", "value"));
    });
    it("delete() does not throw", async () => {
        const s = createSecretsStub();
        await assert.doesNotReject(() => s.delete("key"));
    });
});
