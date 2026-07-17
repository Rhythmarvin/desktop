/**
 * Business error types and factory for Agent plugin authors.
 */

/** JSON-safe value type — finite, plain, acyclic. */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

/** Business failure kinds that plugin authors can create. */
export const AUTHOR_BUSINESS_FAILURE_KINDS = [
  "agentUnavailable",
  "authenticationRequired",
  "invalidAgentConfiguration",
  "installationNotFound",
  "conversationNotFound",
  "unsupportedAgentCapability",
  "invalidState",
  "permissionDenied",
  "cursorExpired",
  "agentProcessFailed",
] as const;

export type AuthorBusinessFailureKind =
  (typeof AUTHOR_BUSINESS_FAILURE_KINDS)[number];

/** All failure kinds including bootstrap-reserved providerFailure. */
export type AgentBusinessFailureKind =
  | AuthorBusinessFailureKind
  | "providerFailure";

/** Input to the business error factory. */
export interface AgentBusinessErrorInput {
  readonly kind: AuthorBusinessFailureKind;
  readonly message: string;
  readonly retryable?: boolean;
  readonly details?: Readonly<Record<string, JsonValue>>;
}

/** Business error with private brand (recognized by bootstrap). */
export interface AgentBusinessError extends Error {
  readonly name: "AgentBusinessError";
  readonly kind: AgentBusinessFailureKind;
  readonly retryable: boolean;
  readonly details?: Readonly<Record<string, JsonValue>>;
}

/** Internal brand symbol for bootstrap recognition. */
const BUSINESS_ERROR_BRAND = Symbol.for("@ora-space/plugin-sdk/AgentBusinessError");

function isJsonValue(value: unknown): value is JsonValue {
  if (value === null || typeof value === "boolean" || typeof value === "string") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isJsonValue);
  if (typeof value === "object") {
    return Object.values(value as Record<string, unknown>).every(isJsonValue);
  }
  return false;
}

/** Create a branded AgentBusinessError. */
export function createBusinessError(
  input: AgentBusinessErrorInput,
): AgentBusinessError {
  // Validate kind
  if (!AUTHOR_BUSINESS_FAILURE_KINDS.includes(input.kind as AuthorBusinessFailureKind)) {
    throw new Error(`Invalid business error kind: ${input.kind}`);
  }

  // Validate details
  if (input.details !== undefined && !isJsonValue(input.details)) {
    throw new Error("Business error details must be plain finite JSON-safe values");
  }

  const error = new Error(input.message) as AgentBusinessError;
  error.name = "AgentBusinessError";
  error.kind = input.kind;
  error.retryable = input.retryable ?? false;
  error.details = input.details;
  (error as Record<symbol, true>)[BUSINESS_ERROR_BRAND] = true;

  return error;
}

/** Check if an error is a branded AgentBusinessError. */
export function isAgentBusinessError(error: unknown): error is AgentBusinessError {
  return (
    error instanceof Error &&
    error.name === "AgentBusinessError" &&
    (error as Record<symbol, unknown>)[BUSINESS_ERROR_BRAND] === true
  );
}
