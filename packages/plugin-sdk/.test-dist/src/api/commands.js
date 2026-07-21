/**
 * API facade for command registration.
 *
 * Plugin developers call `ora.commands.register(id, handler)` to register
 * a command handler. The Host is notified of the registration via
 * `ora.commands.register` notification. The returned Disposable can be
 * used to unregister the command.
 */
export class CommandsAPI {
    rpc;
    handlers = new Map();
    constructor(rpc) {
        this.rpc = rpc;
    }
    /**
     * Register a command handler.
     *
     * @param id      The command identifier (e.g., "myPlugin.hello").
     * @param handler The function to invoke when this command is triggered.
     * @returns A Disposable that unregisters the command when disposed.
     */
    register(id, handler) {
        this.handlers.set(id, handler);
        this.rpc.notify("ora.commands.register", { id });
        return {
            dispose: () => {
                this.handlers.delete(id);
                this.rpc.notify("ora.commands.unregister", { id });
            },
        };
    }
}
