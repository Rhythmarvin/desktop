import type { ExtensionContext } from "./context.js";
import type { Disposable } from "./disposable.js";

export interface PluginConfig {
  readonly activate(ctx: ExtensionContext): void | Promise<void>;
  readonly deactivate?(): void | Promise<void>;
}

/**
 * Plugin author's single entry point.
 * Registers activate/deactivate callbacks for the bootstrap to call
 * after the handshake completes.
 */
export function definePlugin(config: PluginConfig): void {
  (globalThis as Record<string, unknown>).__ora_plugin_config = config;
}
