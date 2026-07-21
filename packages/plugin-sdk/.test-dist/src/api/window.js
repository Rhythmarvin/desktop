/**
 * API facade for window / notification operations.
 */
export class WindowAPI {
    rpc;
    constructor(rpc) {
        this.rpc = rpc;
    }
    async showNotification(message, level = "info") {
        await this.rpc.call("ora.window.showNotification", { message, level });
    }
    async showQuickPick(items) {
        const result = await this.rpc.call("ora.window.showQuickPick", { items });
        return result;
    }
    async showInputBox(prompt) {
        const result = await this.rpc.call("ora.window.showInputBox", { prompt });
        return result;
    }
}
