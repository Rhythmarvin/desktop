const SDK_VERSION = "0.2.0";
/**
 * Handshake step 1: Notify Host that the SDK has initialized.
 * Carries the SDK version so Host can perform compatibility checks.
 */
export function sendReady(rpc) {
    rpc.notify("plugin.ready", { sdkVersion: SDK_VERSION });
}
/**
 * Handshake step 2: Wait for Host's `$/init` request.
 *
 * Returns an object with:
 * - `promise`: resolves with the init data when `$/init` arrives
 * - `handler`: a message handler to register on Transport.onMessage();
 *   returns `true` if it consumed the message (caller should send response and unsubscribe)
 */
export function createInitWaiter() {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
        resolve = res;
        reject = rej;
    });
    let resolved = false;
    const handler = (msg) => {
        if (msg.method !== "$/init")
            return false;
        if (resolved) {
            // Duplicate $/init — reject via a separate mechanism
            // The caller (bootstrap) should not call handler after resolving,
            // but guard defensively.
            return false;
        }
        const params = msg.params;
        if (!params || typeof params.pluginId !== "string" || typeof params.entry !== "string") {
            resolved = true;
            const missing = !params ? "params" :
                typeof params.pluginId !== "string" ? "pluginId" :
                    "entry";
            reject(new Error(`Invalid params: missing required field ${missing}`));
            return true;
        }
        resolved = true;
        resolve({
            pluginId: params.pluginId,
            pluginPath: params.pluginPath ?? "",
            entry: params.entry,
            capabilities: params.capabilities ?? [],
            globalState: params.globalState ?? {},
        });
        return true;
    };
    return { promise, handler };
}
/**
 * Handshake step 3: Wait for Host's `ora.extension.activate` request.
 *
 * Returns an object with:
 * - `promise`: resolves with the absolute entry path to load
 * - `handler`: a message handler to register on Transport.onMessage()
 */
export function createActivateWaiter(initResult) {
    let resolve;
    const promise = new Promise((res) => { resolve = res; });
    const handler = (msg) => {
        if (msg.method !== "ora.extension.activate")
            return false;
        const entryPath = `${initResult.pluginPath}/${initResult.entry}`;
        resolve(entryPath);
        return true;
    };
    return { promise, handler };
}
