// envelope.ts — JSON-RPC 2.0 envelope parsing and construction.
// Matches design-v3 §12.5 and ora-plugin-protocol/src/json_rpc.rs.

import { FrameType, type Frame } from "../transport/frame.js";

export interface RpcRequest {
  readonly type: "request";
  readonly id: string;
  readonly method: string;
  readonly params?: Record<string, unknown>;
}

export interface RpcNotification {
  readonly type: "notification";
  readonly method: string;
  readonly params?: Record<string, unknown>;
}

export interface RpcResponseOk {
  readonly type: "response-ok";
  readonly id: string;
  readonly result: unknown;
}

export interface RpcResponseErr {
  readonly type: "response-err";
  readonly id: string;
  readonly error: { code: number; message: string; data?: Record<string, unknown> };
}

export type InboundEnvelope = RpcRequest | RpcNotification | RpcResponseOk | RpcResponseErr;

// ── Parsing ─────────────────────────────────────────────────────

export function parseInbound(frame: Frame): InboundEnvelope {
  const value = JSON.parse(new TextDecoder().decode(frame.payload));
  const obj = requirePlainObject(value, "JSON-RPC envelope");

  if (obj.jsonrpc !== "2.0") {
    throw new SyntaxError("JSON-RPC version must be 2.0");
  }

  const hasId = Object.hasOwn(obj, "id");
  const hasMethod = Object.hasOwn(obj, "method");
  const hasResult = Object.hasOwn(obj, "result");
  const hasError = Object.hasOwn(obj, "error");

  if (frame.type === FrameType.Request) {
    if (!hasId || !hasMethod) throw new SyntaxError("Request must have id and method");
    if (hasResult || hasError) throw new SyntaxError("Request must not have result or error");
    const id = requireBoundedString(obj.id, "request id", 128);
    const method = requireBoundedString(obj.method, "request method", 256);
    if (hasId && obj.id === null) throw new SyntaxError("Request id must not be null");
    return Object.hasOwn(obj, "params")
      ? { type: "request", id, method, params: requirePlainObject(obj.params, "params") }
      : { type: "request", id, method };
  }

  if (frame.type === FrameType.Response) {
    if (!hasId) throw new SyntaxError("Response must have id");
    if (hasMethod) throw new SyntaxError("Response must not have method");
    if (hasResult === hasError) throw new SyntaxError("Response must have exactly one of result or error");
    const id = String(obj.id);
    if (hasResult) {
      return { type: "response-ok", id, result: obj.result };
    }
    const err = obj.error as Record<string, unknown>;
    return {
      type: "response-err",
      id,
      error: {
        code: Number(err.code),
        message: String(err.message),
        data: err.data as Record<string, unknown> | undefined,
      },
    };
  }

  if (frame.type === FrameType.Notification) {
    if (!hasMethod) throw new SyntaxError("Notification must have method");
    if (hasId || hasResult || hasError) throw new SyntaxError("Notification must not have id, result, or error");
    const method = requireBoundedString(obj.method, "notification method", 256);
    return Object.hasOwn(obj, "params")
      ? { type: "notification", method, params: requirePlainObject(obj.params, "params") }
      : { type: "notification", method };
  }

  throw new Error(`Unsupported frame type: ${frame.type}`);
}

// ── Encoding ─────────────────────────────────────────────────────

export function encodeRequest(id: string, method: string, params?: Record<string, unknown>): Uint8Array {
  const obj = params
    ? { jsonrpc: "2.0", id, method, params }
    : { jsonrpc: "2.0", id, method };
  return new TextEncoder().encode(JSON.stringify(obj));
}

export function encodeSuccess(id: string, result: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify({ jsonrpc: "2.0", id, result }));
}

export function encodeError(id: string, code: number, message: string, data?: Record<string, unknown>): Uint8Array {
  const err = data ? { code, message, data } : { code, message };
  return new TextEncoder().encode(JSON.stringify({ jsonrpc: "2.0", id, error: err }));
}

export function encodeNotification(method: string, params?: Record<string, unknown>): Uint8Array {
  const obj = params
    ? { jsonrpc: "2.0", method, params }
    : { jsonrpc: "2.0", method };
  return new TextEncoder().encode(JSON.stringify(obj));
}

// ── Helpers ──────────────────────────────────────────────────────

export function requirePlainObject(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value as Record<string, unknown>;
}

export function requireBoundedString(value: unknown, label: string, maxBytes: number): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  const byteLen = new TextEncoder().encode(value).byteLength;
  if (byteLen > maxBytes || value.includes("\0")) {
    throw new TypeError(`${label} exceeds ${maxBytes} bytes or contains NUL`);
  }
  return value;
}
