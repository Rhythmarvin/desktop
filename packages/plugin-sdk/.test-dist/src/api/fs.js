/**
 * API facade for filesystem operations.
 */
export class FsAPI {
    rpc;
    constructor(rpc) {
        this.rpc = rpc;
    }
    async readFile(path) {
        const result = await this.rpc.call("ora.fs.readFile", { path });
        return result;
    }
    async writeFile(path, content) {
        await this.rpc.call("ora.fs.writeFile", { path, content });
    }
}
