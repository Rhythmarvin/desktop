/**
 * ExtensionContext and related types for Agent plugin authors.
 */

export interface Disposable {
  dispose(): void | Promise<void>;
}

/** Plugin logger — writes sanitized messages to stderr. */
export interface PluginLogger {
  debug(message: string): void;
  info(message: string): void;
  warn(message: string): void;
  error(message: string): void;
}

/** LIFO subscription store — disposed on deactivate. */
export interface SubscriptionStore {
  add<T extends Disposable>(disposable: T): T;
}

import type { AgentBusinessError, AgentBusinessErrorInput } from "./errors.js";

/**
 * ExtensionContext v1 — available during plugin activation.
 * Contains plugin identity, paths, lifecycle signals, and utilities.
 */
export interface ExtensionContext {
  readonly plugin: Readonly<{
    id: string;
    version: string;
  }>;
  readonly sessionId: string;
  readonly extensionPath: string;
  readonly storagePath: string;
  readonly logger: PluginLogger;
  readonly shutdownSignal: AbortSignal;
  readonly subscriptions: SubscriptionStore;
  readonly errors: Readonly<{
    business(input: AgentBusinessErrorInput): AgentBusinessError;
  }>;
}
