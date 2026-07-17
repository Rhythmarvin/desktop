/**
 * Core plugin definition types — the structural ABI.
 */

import type { ExtensionContext } from "./context.js";
import type { AgentProvider } from "./provider.js";

/** Plugin activation result: the set of registered providers. */
export interface AgentActivation {
  readonly providers: readonly AgentProvider[];
}

/** Plugin definition structure — validated by bootstrap. */
export interface AgentPluginDefinition {
  readonly kind: "agent";
  readonly pluginApi: 1;
  activate(context: ExtensionContext): AgentActivation | Promise<AgentActivation>;
  deactivate?(): void | Promise<void>;
}
