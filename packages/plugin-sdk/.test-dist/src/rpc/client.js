import { Transport } from "./transport.js";
/**
 * Maps each known `ora.*` RPC method to its required capability.
 * Methods not in this map require no capability.
 */
const METHOD_CAPABILITY_MAP = {
    "ora.project.list": "project.read",
    "ora.project.get": "project.read",
    "ora.project.create": "project.write",
    "ora.project.update": "project.write",
    "ora.project.delete": "project.write",
    "ora.task.list": "task.read",
    "ora.task.get": "task.read",
    "ora.task.create": "task.write",
    "ora.task.update": "task.write",
    "ora.task.delete": "task.write",
    "ora.window.showNotification": "notification.show",
    "ora.window.showQuickPick": "window.showQuickPick",
    "ora.window.showInputBox": "window.showInputBox",
    "ora.fs.readFile": "fs.read",
    "ora.fs.writeFile": "fs.write",
    "ora.network.fetch": "network.fetch",
    "ora.clipboard.read": "clipboard.read",
    "ora.clipboard.write": "clipboard.write",
};
/**
 * Module-level set of capabilities granted to the current plugin.
 * Populated when `$/init` is received during the handshake.
 */
let grantedCapabilities = new Set();
/** Read the current granted capabilities (for use in capability pre-check). */
export function getGrantedCapabilities() {
    return grantedCapabilities;
}
/** Set the granted capabilities (called when $/init is received). */
export function setGrantedCapabilities(caps) {
    grantedCapabilities = new Set(caps);
}
/**
 * JSON-RPC 2.0 client for SDK → Host communication.
 *
 * - `call(method, params)` sends a request with a unique integer id and
 *   returns a Promise that resolves with the response result or rejects on error.
 * - `notify(method, params)` sends a notification without an id (fire-and-forget).
 * - `destroy()` rejects all pending requests and unsubscribes from the transport.
 */
export class RpcClient {
    nextId = 1;
    pending = new Map();
    transport;
    unsubscribe = null;
    destroyed = false;
    constructor(transport) {
        this.transport = transport;
        this.unsubscribe = transport.onMessage((msg) => this.handleMessage(msg));
    }
    /**
     * Send a JSON-RPC request and return a Promise for the response.
     *
     * @throws {Error} If the client is destroyed or the required capability is not granted.
     */
    call(method, params = {}) {
        if (this.destroyed) {
            throw new Error("RpcClient is destroyed");
        }
        // Capability pre-check
        const required = METHOD_CAPABILITY_MAP[method];
        if (required && !grantedCapabilities.has(required)) {
            const granted = [...grantedCapabilities].join(", ") || "(none)";
            throw new Error(`Capability "${required}" is not granted. ` +
                `Granted: [${granted}]. ` +
                `Check plugin.json capabilities declaration or user authorization settings.`);
        }
        const id = this.nextId++;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject, method });
            this.transport.send({
                jsonrpc: "2.0",
                id,
                method,
                params: params,
            });
        });
    }
    /**
     * Send a JSON-RPC notification (no id, no response expected).
     */
    notify(method, params = {}) {
        this.transport.send({
            jsonrpc: "2.0",
            method,
            params: params,
        });
    }
    /**
     * Reject all pending requests and unsubscribe from transport.
     * Safe to call multiple times.
     */
    destroy() {
        if (this.destroyed)
            return;
        this.destroyed = true;
        this.unsubscribe?.();
        this.unsubscribe = null;
        for (const [, pending] of this.pending) {
            pending.reject(new Error("Transport closed"));
        }
        this.pending.clear();
    }
    /**
     * Handle an incoming message from the transport.
     * Routes by id: ignores messages with a `method` field (Host-initiated requests)
     * and unmatched ids.
     */
    handleMessage(msg) {
        // Ignore notifications and requests from Host (they have a `method` field)
        if ("method" in msg)
            return;
        const id = msg.id;
        if (typeof id !== "number")
            return;
        const pending = this.pending.get(id);
        if (!pending)
            return; // unmatched id, silently ignore
        this.pending.delete(id);
        if ("error" in msg) {
            const err = msg.error;
            pending.reject(new Error(`RPC error ${err.code}: ${err.message}`));
        }
        else {
            // result may be present or absent; null is a valid JSON result
            pending.resolve("result" in msg ? msg.result : null);
        }
    }
}
