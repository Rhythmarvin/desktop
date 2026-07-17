/**
 * Incremental 5-byte binary frame reader for stdin.
 * Uses a ring buffer + cursor for O(1) amortized parsing.
 */
import { Buffer } from "node:buffer";

export const HEADER_LEN = 5;
export const MAX_PAYLOAD = 8 * 1024 * 1024; // 8 MiB

export interface ParsedFrame {
  type: 1 | 2 | 3;
  payload: string;
  payloadJson: unknown;
}

export type FrameHandler = (frame: ParsedFrame) => void;
export type ErrorHandler = (error: Error) => void;
export type CloseHandler = () => void;

function readInt32BE(buf: Buffer, offset: number): number {
  return buf.readInt32BE(offset);
}

function readInt8(buf: Buffer, offset: number): number {
  return buf.readInt8(offset);
}

export function createFrameReader(
  onFrame: FrameHandler,
  onError: ErrorHandler,
  onClose: CloseHandler,
) {
  // Ring buffer for accumulating stdin bytes
  let buffer = Buffer.alloc(0);

  return {
    /** Feed incoming bytes into the parser. */
    feed(chunk: Buffer): void {
      if (chunk.length === 0) {
        onClose();
        return;
      }
      buffer = Buffer.concat([buffer, chunk]);
      this.tryParse();
    },

    /** Signal that stdin has ended (clean EOF). */
    end(): void {
      if (buffer.length === 0) {
        onClose();
      } else {
        onError(new Error(`Unexpected EOF with ${buffer.length} unprocessed bytes`));
      }
    },

    /** Attempt to parse complete frames from the buffer. */
    tryParse(): void {
      while (buffer.length >= HEADER_LEN) {
        const len = readInt32BE(buffer, 0);
        const type = readInt8(buffer, 4);

        // Validate length
        if (len <= 0) {
          onError(new Error(`Invalid frame length: ${len}`));
          return;
        }
        if (len > MAX_PAYLOAD) {
          onError(new Error(`Payload too large: ${len} bytes (max ${MAX_PAYLOAD})`));
          return;
        }

        // Validate type
        if (type !== 1 && type !== 2 && type !== 3) {
          onError(new Error(`Unknown frame type: ${type}`));
          return;
        }

        const totalFrameLen = HEADER_LEN + len;
        if (buffer.length < totalFrameLen) {
          // Partial frame — wait for more data
          break;
        }

        // Extract payload
        const payloadBuffer = buffer.subarray(HEADER_LEN, totalFrameLen);
        let payload: string;
        try {
          payload = payloadBuffer.toString("utf-8");
        } catch {
          onError(new Error("Payload is not valid UTF-8"));
          return;
        }

        let payloadJson: unknown;
        try {
          payloadJson = JSON.parse(payload);
        } catch (e) {
          onError(new Error(`Invalid JSON payload: ${(e as Error).message}`));
          return;
        }

        // Reject non-object JSON (no batch arrays)
        if (typeof payloadJson !== "object" || payloadJson === null || Array.isArray(payloadJson)) {
          onError(new Error("Frame payload must be a single JSON object"));
          return;
        }

        // Consume the frame
        buffer = buffer.subarray(totalFrameLen);
        onFrame({
          type: type as 1 | 2 | 3,
          payload,
          payloadJson,
        });
      }
    },
  };
}
