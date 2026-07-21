/**
 * API facade for network operations.
 */
export class NetworkAPI {
    rpc;
    constructor(rpc) {
        this.rpc = rpc;
    }
    async fetch(url, options) {
        return this.rpc.call("ora.network.fetch", { url, ...options });
    }
}
