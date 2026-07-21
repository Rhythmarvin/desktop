/**
 * API facade for project operations.
 * Each method delegates to a corresponding `ora.project.*` RPC call.
 */
export class ProjectAPI {
    rpc;
    constructor(rpc) {
        this.rpc = rpc;
    }
    async list() {
        return this.rpc.call("ora.project.list", {});
    }
    async get(projectId) {
        return this.rpc.call("ora.project.get", { projectId });
    }
    async create(params) {
        return this.rpc.call("ora.project.create", params);
    }
    async update(projectId, params) {
        return this.rpc.call("ora.project.update", { projectId, ...params });
    }
    async delete(projectId) {
        return this.rpc.call("ora.project.delete", { projectId });
    }
}
