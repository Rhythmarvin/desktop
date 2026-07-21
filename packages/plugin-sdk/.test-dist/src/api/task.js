/**
 * API facade for task operations.
 * Each method delegates to a corresponding `ora.task.*` RPC call.
 */
export class TaskAPI {
    rpc;
    constructor(rpc) {
        this.rpc = rpc;
    }
    async list() {
        return this.rpc.call("ora.task.list", {});
    }
    async get(taskId) {
        return this.rpc.call("ora.task.get", { taskId });
    }
    async create(params) {
        return this.rpc.call("ora.task.create", params);
    }
    async update(taskId, params) {
        return this.rpc.call("ora.task.update", { taskId, ...params });
    }
    async delete(taskId) {
        return this.rpc.call("ora.task.delete", { taskId });
    }
}
