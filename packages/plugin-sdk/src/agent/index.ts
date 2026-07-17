/**
 * Public Agent SDK — defineAgentPlugin entry point.
 *
 * Usage:
 *   import { defineAgentPlugin } from "@ora-space/plugin-sdk/agent";
 *   export default defineAgentPlugin({ ... });
 */

import type { AgentActivation, AgentPluginDefinition } from "./types.js";

export type { ExtensionContext, Disposable, PluginLogger, SubscriptionStore } from "./context.js";
export type { AgentProvider, AgentCallContext } from "./provider.js";
export type {
  AgentPluginDefinition,
  AgentActivation,
} from "./types.js";
export type {
  AgentBusinessError,
  AgentBusinessErrorInput,
  AgentBusinessFailureKind,
  AuthorBusinessFailureKind,
} from "./errors.js";

/**
 * Zero-I/O identity helper. Returns the same plain object.
 *
 * Authors may also write an equivalent object by hand — bootstrap
 * performs structural validation only (no `instanceof` dependency).
 */
export function defineAgentPlugin<T extends AgentPluginDefinition>(
  definition: T,
): T {
  return definition;
}
