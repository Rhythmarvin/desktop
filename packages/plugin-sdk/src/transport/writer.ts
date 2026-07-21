// writer.ts — ProtocolWriter: owns stdout for frame output.

import { encodeFrame, type FrameType } from "./frame.js";

export class ProtocolWriter {
  #stdoutWrite: (chunk: Uint8Array) => void;

  constructor(stdout: { write(chunk: Uint8Array): void }) {
    this.#stdoutWrite = stdout.write.bind(stdout);
  }

  /** Writes one encoded frame to stdout. */
  write(type: FrameType, payload: Uint8Array): void {
    this.#stdoutWrite(encodeFrame(type, payload));
  }

  /** Writes a JSON-encoded frame. */
  writeJson(type: FrameType, obj: unknown): void {
    const json = JSON.stringify(obj);
    const payload = new TextEncoder().encode(json);
    this.write(type, payload);
  }
}
