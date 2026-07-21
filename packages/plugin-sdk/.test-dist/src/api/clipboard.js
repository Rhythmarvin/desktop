/**
 * API facade for clipboard operations.
 */
export class ClipboardAPI {
    rpc;
    constructor(rpc) {
        this.rpc = rpc;
    }
    async read() {
        const result = await this.rpc.call("ora.clipboard.read", {});
        return result;
    }
    async write(text) {
        await this.rpc.call("ora.clipboard.write", { text });
    }
}
