/**
 * 5-byte binary frame writer for stdout.
 * Captures process.stdout.write before plugin code loads.
 */
import { Buffer } from "node:buffer";
import { HEADER_LEN, MAX_PAYLOAD } from "./reader.js";

/** Encode a complete frame: 5-byte header + payload. */
export function encodeFrame(type: 1 | 2 | 3, payloadJson: string): Buffer {
  const payload = Buffer.from(payloadJson, "utf-8");

  if (payload.length === 0) {
    throw new Error("Payload must not be empty");
  }
  if (payload.length > MAX_PAYLOAD) {
    throw new Error(`Payload too large: ${payload.length} bytes (max ${MAX_PAYLOAD})`);
  }

  const header = Buffer.alloc(HEADER_LEN);
  header.writeInt32BE(payload.length, 0);
  header.writeInt8(type, 4);

  return Buffer.concat([header, payload]);
}

/** Create a frame writer using the captured stdout write reference. */
export function createFrameWriter(writeFn?: (buf: Buffer) => boolean) {
  const write = writeFn ?? process.stdout.write.bind(process.stdout);

  return {
    /** Write a complete frame to stdout. Returns true on success. */
    writeFrame(type: 1 | 2 | 3, payloadJson: string): boolean {
      const frame = encodeFrame(type, payloadJson);
      return write(frame);
    },

    /** Write a complete frame asynchronously. */
    async writeFrameAsync(type: 1 | 2 | 3, payloadJson: string): Promise<void> {
      const frame = encodeFrame(type, payloadJson);
      return new Promise<void>((resolve, reject) => {
        const success = write(frame);
        if (success) {
          resolve();
        } else {
          // Wait for drain
          process.stdout.once("drain", () => {
            resolve();
          });
        }
      });
    },
  };
}
