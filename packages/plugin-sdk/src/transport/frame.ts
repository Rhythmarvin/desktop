// frame.ts — 5-byte binary frame encode/decode.
// Wire format: [length: i32 BE][type: i8][payload: UTF-8 JSON]
// Matches design-v3 §12 and ora-plugin-protocol/src/frame.rs.

export const FRAME_HEADER_BYTES = 5;
export const MAX_FRAME_BYTES = 8 * 1024 * 1024; // 8 MiB

export const FrameType = {
  Request: 1,
  Response: 2,
  Notification: 3,
} as const;

export type FrameType = (typeof FrameType)[keyof typeof FrameType];

export interface Frame {
  readonly type: FrameType;
  readonly payload: Uint8Array;
}

/** Encodes `[length: i32 BE][type: i8][payload]`. */
export function encodeFrame(type: FrameType, payload: Uint8Array): Uint8Array {
  if (payload.byteLength <= 0 || payload.byteLength > MAX_FRAME_BYTES) {
    throw new RangeError(`Invalid payload length: ${payload.byteLength}`);
  }
  const buf = new Uint8Array(FRAME_HEADER_BYTES + payload.byteLength);
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  view.setInt32(0, payload.byteLength, false); // offset 0: length i32 BE
  view.setInt8(4, type);                        // offset 4: type i8
  buf.set(payload, FRAME_HEADER_BYTES);
  return buf;
}

/** Incremental frame decoder for stream reassembly. */
export class FrameDecoder {
  #header = new Uint8Array(FRAME_HEADER_BYTES);
  #headerFilled = 0;
  #frameType = 0;
  #payload: Uint8Array | null = null;
  #payloadFilled = 0;
  #payloadExpected = 0;

  /** Consumes one arbitrary chunk and returns every complete frame. */
  decodeChunk(chunk: Uint8Array): Frame[] {
    const frames: Frame[] = [];
    let offset = 0;

    while (offset < chunk.byteLength) {
      if (this.#payload === null) {
        const copied = Math.min(FRAME_HEADER_BYTES - this.#headerFilled, chunk.byteLength - offset);
        this.#header.set(chunk.subarray(offset, offset + copied), this.#headerFilled);
        this.#headerFilled += copied;
        offset += copied;

        if (this.#headerFilled === FRAME_HEADER_BYTES) {
          const view = new DataView(this.#header.buffer, this.#header.byteOffset, this.#header.byteLength);
          const length = view.getInt32(0, false); // offset 0: length i32 BE
          const type = view.getInt8(4);            // offset 4: type i8

          if (length <= 0 || length > MAX_FRAME_BYTES) {
            throw new RangeError(`Invalid payload length: ${length}`);
          }
          if (type < 1 || type > 3) {
            throw new RangeError(`Unknown frame type: ${type}`);
          }
          this.#frameType = type as FrameType;
          this.#payloadExpected = length;
          this.#payload = new Uint8Array(length);
          this.#payloadFilled = 0;
        }
      } else {
        const copied = Math.min(this.#payloadExpected - this.#payloadFilled, chunk.byteLength - offset);
        this.#payload.set(chunk.subarray(offset, offset + copied), this.#payloadFilled);
        this.#payloadFilled += copied;
        offset += copied;

        if (this.#payloadFilled === this.#payloadExpected) {
          frames.push({ type: this.#frameType, payload: this.#payload! });
          this.#header = new Uint8Array(FRAME_HEADER_BYTES);
          this.#headerFilled = 0;
          this.#frameType = 0;
          this.#payload = null;
          this.#payloadFilled = 0;
          this.#payloadExpected = 0;
        }
      }
    }
    return frames;
  }

  /** Validates that EOF occurred exactly on a frame boundary. */
  finish(): void {
    if (this.#payload !== null) {
      throw new Error("partial frame payload at EOF");
    }
    if (this.#headerFilled !== 0) {
      throw new Error("partial frame header at EOF");
    }
  }
}
