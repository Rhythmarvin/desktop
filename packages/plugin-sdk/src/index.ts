import "./console-guard.js";

export { definePlugin } from "./define-plugin.js";
export type { PluginConfig } from "./define-plugin.js";
export type { ExtensionContext, PluginLogger } from "./context.js";
export type { Disposable } from "./disposable.js";

// Agent plugin bootstrap (for kind: "agent" plugins)
export { runAgentBootstrap } from "./bootstrap/agent.js";
