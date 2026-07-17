import assert from "node:assert/strict";
import test from "node:test";

import {
  FrameDecoder,
  FrameType,
  MAX_FRAME_BYTES,
  encodeFrame,
} from "../../src/transport/frame.js";

const encoder = new TextEncoder();
const request = encoder.encode('{"jsonrpc":"2.0","id":"h:1","method":"ping","params":{}}');

test("encodes the canonical five-byte big-endian header", () => {
  const encoded = encodeFrame(FrameType.Request, request);
  assert.deepEqual([...encoded.subarray(0, 5)], [0, 0, 0, 0x38, 1]);
  assert.deepEqual(encoded.subarray(5), request);
});

test("decodes every split position and coalesced frames", () => {
  const encoded = encodeFrame(FrameType.Request, request);
  for (let cut = 0; cut <= encoded.byteLength; cut += 1) {
    const decoder = new FrameDecoder();
    const frames = [
      ...decoder.decodeChunk(encoded.subarray(0, cut)),
      ...decoder.decodeChunk(encoded.subarray(cut)),
    ];
    decoder.finish();
    assert.deepEqual(frames, [{ type: FrameType.Request, payload: request }]);
  }

  const decoder = new FrameDecoder();
  assert.equal(decoder.decodeChunk(new Uint8Array([...encoded, ...encoded])).length, 2);
});

test("rejects invalid signed lengths, types, caps, and partial EOF", () => {
  assert.throws(() => new FrameDecoder(MAX_FRAME_BYTES + 1), RangeError);
  assert.throws(() => new FrameDecoder().decodeChunk(Uint8Array.of(0, 0, 0, 0, 1)), RangeError);
  assert.throws(
    () => new FrameDecoder().decodeChunk(Uint8Array.of(0xff, 0xff, 0xff, 0xff, 1)),
    RangeError,
  );
  assert.throws(() => new FrameDecoder().decodeChunk(Uint8Array.of(0, 0, 0, 2, 0x7f)), RangeError);

  const decoder = new FrameDecoder();
  decoder.decodeChunk(Uint8Array.of(0, 0));
  assert.throws(() => decoder.finish(), /partial frame header/u);
});
