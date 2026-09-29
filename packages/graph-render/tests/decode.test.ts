import assert from "node:assert/strict";
import { test } from "node:test";

import { SnapshotRefusal, decodeSnapshot, idAt } from "../src/snapshot/decode.ts";

// docs/contract/binary-layout.md, "The pinned 84-byte example", bytes 0..79.
const HEAD = [
  0x47, 0x4d, 0x53, 0x4e, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0,
  0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 0x61, 0x62, 0x63, 0,
  0, 0, 0, 0, 1, 0, 0, 0, 0x65, 0, 0, 0,
  0, 0, 0, 0, 1, 0, 0, 0,
  0, 0, 0x80, 0x3f, 0, 0, 0x20, 0xc0, 0, 0, 0, 0, 0, 0, 0, 0x3f,
];
const NO_NOTES = [0, 0, 0, 0];
const CURVE_TAIL = [2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0x80, 0x3f, 0, 0, 0, 0x3f];

function pinned(patch: Readonly<Record<number, number>> = {}, tail: readonly number[] = NO_NOTES): Uint8Array {
  const bytes = Uint8Array.from([...HEAD, ...tail]);
  for (const [offset, value] of Object.entries(patch)) bytes[Number(offset)] = value;
  return bytes;
}

function refusal(bytes: Uint8Array): SnapshotRefusal {
  try {
    decodeSnapshot(bytes);
  } catch (error) {
    if (error instanceof SnapshotRefusal) return error;
    throw error;
  }
  throw new assert.AssertionError({ message: "the snapshot was accepted" });
}

test("the pinned 84-byte snapshot decodes column by column", () => {
  const snapshot = decodeSnapshot(pinned());
  assert.deepEqual([snapshot.major, snapshot.minor], [0, 3]);
  assert.deepEqual([snapshot.nodeKind, snapshot.edgeKind], ["Point", "Line"]);
  assert.deepEqual([snapshot.nodeCount, snapshot.edgeCount], [2, 1]);
  assert.deepEqual([idAt(snapshot.nodeIds, 0), idAt(snapshot.nodeIds, 1)], ["a", "bc"]);
  assert.equal(idAt(snapshot.edgeIds, 0), "e");
  assert.deepEqual([...snapshot.source, ...snapshot.target], [0, 1]);
  assert.deepEqual([...snapshot.x], [1, -2.5]);
  assert.deepEqual([...snapshot.y], [0, 0.5]);
  assert.equal(snapshot.r, null);
  assert.equal(snapshot.pts, null);
  assert.deepEqual(snapshot.notes, []);
});

test("the pinned curve variant carries its degree and one control point", () => {
  const snapshot = decodeSnapshot(pinned({ 13: 2 }, [...CURVE_TAIL, ...NO_NOTES]));
  assert.equal(snapshot.edgeKind, "Curve");
  assert.equal(snapshot.curveDegree, 2);
  assert.deepEqual([...(snapshot.offsets ?? [])], [0, 1]);
  assert.deepEqual([...(snapshot.pts ?? [])], [1, 0.5]);
});

test("the pinned one-note snapshot names its note", () => {
  const snapshot = decodeSnapshot(pinned({}, [1, 0, 0, 0, 3, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]));
  assert.deepEqual(snapshot.notes, [{ code: 3, name: "packing.approximate", index: 4294967295 }]);
});

test("a note this reader cannot name is kept, unnamed", () => {
  const snapshot = decodeSnapshot(pinned({}, [1, 0, 0, 0, 99, 0, 0, 0, 0, 0, 0, 0]));
  assert.deepEqual(snapshot.notes, [{ code: 99, name: null, index: 0 }]);
});

test("a 0.2 snapshot has no notes section", () => {
  const snapshot = decodeSnapshot(pinned({ 8: 2 }, []));
  assert.deepEqual(snapshot.notes, []);
  assert.equal(refusal(pinned({}, [])).column, "note.count");
});

test("a view at an odd byte offset is decoded from a copy", () => {
  const padded = new Uint8Array(85);
  padded.set(pinned(), 1);
  assert.deepEqual([...decodeSnapshot(padded.subarray(1)).x], [1, -2.5]);
});

test("every truncation length is refused", () => {
  const whole = pinned({ 13: 2 }, [...CURVE_TAIL, 1, 0, 0, 0, 3, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
  decodeSnapshot(whole);
  for (let length = 0; length < whole.length; length += 1) {
    assert.equal(refusal(whole.slice(0, length)).code, "truncated", `length ${length}`);
  }
});

test("a header claiming more nodes than the payload holds is refused", () => {
  assert.equal(refusal(pinned({ 20: 0xff, 21: 0xff, 22: 0xff, 23: 0xff })).code, "truncated");
});

const REFUSALS: readonly (readonly [string, Uint8Array, string, string])[] = [
  ["corrupted magic", pinned({ 0: 0x48 }), "bad-magic", "magic"],
  ["newer major", pinned({ 4: 1 }), "unsupported-major", "version.major"],
  ["z channel", pinned({ 14: 1 }), "reserved-field", "z"],
  ["header padding", pinned({ 15: 1 }), "reserved-field", "padding"],
  ["unknown node tag", pinned({ 12: 3 }), "geometry-tag", "node.kind"],
  ["reserved edge tag", pinned({ 13: 3 }), "geometry-tag", "edge.kind"],
  ["stage count", pinned({ 16: 2 }), "reserved-field", "stage.count"],
  ["first offset not zero", pinned({ 28: 1 }), "offsets", "node.id"],
  ["decreasing offset", pinned({ 32: 4 }), "offsets", "node.id"],
  ["nonzero string padding", pinned({ 43: 7 }), "padding", "node.id"],
  ["endpoint past the node count", pinned({ 60: 2 }), "endpoint", "edge.target"],
  ["NaN coordinate", pinned({ 78: 0xc0, 79: 0x7f }), "non-finite", "node.y"],
  ["trailing bytes", pinned({}, [...NO_NOTES, 0, 0, 0, 0]), "trailing-bytes", "snapshot"],
  ["curve degree zero", pinned({ 13: 2 }, [0, ...CURVE_TAIL.slice(1), ...NO_NOTES]), "curve-degree", "edge.degree"],
];

for (const [name, bytes, code, column] of REFUSALS) {
  test(`refused: ${name}`, () => {
    const found = refusal(bytes);
    assert.deepEqual([found.code, found.column], [code, column]);
  });
}

test("a negative circle radius is refused", () => {
  const circle = pinned({ 12: 1 }, [0, 0, 0x80, 0x3f, 0, 0, 0x80, 0xbf, ...NO_NOTES]);
  assert.deepEqual([refusal(circle).code, refusal(circle).column], ["negative", "node.r"]);
});
