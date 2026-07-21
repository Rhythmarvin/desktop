/**
 * Create an MVP SecretStorage stub.
 *
 * In the MVP, secrets are not backed by a real secure store (e.g., Tauri's
 * secure store or macOS Keychain). All methods are no-ops.
 *
 * Plugin developers can call these APIs; they won't throw, but data will
 * not be persisted.
 */
export function createSecretsStub() {
    return {
        async get(_key) {
            return undefined;
        },
        async store(_key, _value) {
            // no-op
        },
        async delete(_key) {
            // no-op
        },
    };
}
