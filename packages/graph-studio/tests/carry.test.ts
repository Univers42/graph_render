/**
 * A newly loaded document replacing another (studio/carry.ts): the nodes the two share by id move
 * from where they were drawn, the switch always cross-fades, and nothing is carried when no id is
 * shared or the drawing is past the size a match is worth.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { frameFrom } from "../../graph-render/src/frame.ts";
import { decodeSnapshot } from "../../graph-render/src/snapshot/decode.ts";
import type { FrameOptions } from "../../graph-render/src/view.ts";
import { CARRY_MAX, carriedStart, showFresh } from "../src/studio/carry.ts";
import type { Held } from "../src/studio/pipeline/clear.ts";

function table(ids: readonly string[]): Uint8Array {
  const parts = ids.map((id) => new TextEncoder().encode(id));
  const total = parts.reduce((sum, part) => sum + part.byteLength, 0);
  const out = new Uint8Array(4 * (ids.length + 1) + total + ((4 - (total % 4)) % 4));
  const offsets = new Uint32Array(out.buffer, 0, ids.length + 1);
  let at = 4 * (ids.length + 1);
  parts.forEach((part, index) => {
    out.set(part, at);
    at += part.byteLength;
    offsets[index + 1] = (offsets[index] ?? 0) + part.byteLength;
  });
  return out;
}

/** GMSN bytes for discs on a line, no edges: what a motor run of a document of these ids decodes like. */
function bytesOf(ids: readonly string[], x: readonly number[]): Uint8Array {
  const header = new Uint8Array(Uint32Array.of(0x4e534d47, 0, 0, 1, 1, ids.length, 0).buffer);
  const column = (values: readonly number[]): Uint8Array => new Uint8Array(Float32Array.from(values).buffer);
  const parts = [header, table(ids), table([]), column(x), column(x.map(() => 5)), column(x.map(() => 4))];
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.byteLength, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.byteLength;
  }
  return out;
}

function held(ids: readonly string[], x: readonly number[]): Held {
  const bytes = bytesOf(ids, x);
  return { bytes, ends: frameFrom(decodeSnapshot(bytes)) };
}

test("the nodes a new document shares with the drawing start where they were drawn", () => {
  const next = decodeSnapshot(bytesOf(["c", "x", "a"], [1, 2, 3]));
  const drawn = held(["a", "b", "c"], [0, 10, 20]);
  const start = carriedStart(drawn, next);
  // In the drawing's world, which frameFrom scaled from the motor's units.
  const { x, y } = drawn.ends;
  assert.deepEqual(Array.from(start?.x ?? []), [x[2], Number.NaN, x[0]]);
  assert.deepEqual(Array.from(start?.y ?? []), [y[2], Number.NaN, y[0]]);
  assert.notEqual(x[2], x[0], "the control: the two shared nodes were drawn apart");
});

test("nothing is carried with nothing held, no id shared, or a drawing past CARRY_MAX", () => {
  const next = decodeSnapshot(bytesOf(["a", "b"], [1, 2]));
  assert.equal(carriedStart(null, next), null);
  assert.equal(carriedStart(held(["p", "q"], [0, 1]), next), null);
  const large = held(["a", "b"], [0, 1]);
  assert.equal(carriedStart({ ...large, ends: { ...large.ends, nodeCount: CARRY_MAX + 1 } }, next), null);
  assert.notEqual(carriedStart(large, next), null, "the control: the same drawing at its own size carries");
});

test("a fresh document always fades, and moves only what it carried", () => {
  const calls: string[] = [];
  const view = {
    crossFade: () => void calls.push("crossFade"),
    setFrame: (_frame: unknown, options: FrameOptions = {}) => void calls.push(`setFrame ${options.animate === true} ${options.start !== undefined}`),
  };
  const next = decodeSnapshot(bytesOf(["a", "z"], [1, 2]));
  showFresh(view, held(["a"], [0]), next, frameFrom(next));
  showFresh(view, held(["q"], [0]), next, frameFrom(next));
  assert.deepEqual(calls, ["crossFade", "setFrame true true", "crossFade", "setFrame false false"]);
});
