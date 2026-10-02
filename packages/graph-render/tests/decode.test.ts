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
/** The pinned example's x and y columns, [1, -2.5, 0, 0.5] as bytes. They are the last 16. */
const XY = [...HEAD.slice(64)];
/** The pinned example without its x and y columns, so a fixture can supply its own. */
const HEAD_NO_XY = [...HEAD.slice(0, 64)];

function pinned(patch: Readonly<Record<number, number>> = {}, tail: readonly number[] = NO_NOTES): Uint8Array {
  const bytes = Uint8Array.from([...HEAD, ...tail]);
  for (const [offset, value] of Object.entries(patch)) bytes[Number(offset)] = value;
  return bytes;
}

/** One f32 as its four little-endian bytes, so a fixture states values rather than hex. */
function f32(value: number): number[] {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setFloat32(0, value, true);
  return [...bytes];
}

/** One f32 column, from its values in order. */
function column(values: readonly number[]): number[] {
  return values.flatMap(f32);
}

/**
 * A Circle snapshot with the node columns the caller gives, in wire order, and an empty note
 * section. The z column is only in the bytes when `dim` is 1 and the radius column follows
 * it — the shift this reader has to get right, so the fixture spells the order out instead
 * of patching one byte and hoping the rest lands.
 */
function circle(dim: number, nodeColumns: readonly number[]): Uint8Array {
  const bytes = Uint8Array.from([...HEAD_NO_XY, ...nodeColumns, ...NO_NOTES]);
  bytes[12] = 1;
  bytes[14] = dim;
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
  ["reserved dim", pinned({ 14: 2 }), "reserved-dim", "dim"],
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

test("a dim byte of zero is 2D and is read, byte for byte as before", () => {
  // The negative control for the dim check: byte 14 == 0 must never refuse, and the
  // decode it yields is the same one the pinned example already pinned.
  assert.equal(pinned()[14], 0);
  const snapshot = decodeSnapshot(pinned({ 14: 0 }));
  assert.deepEqual([...snapshot.x], [1, -2.5]);
  assert.deepEqual([...snapshot.y], [0, 0.5]);
  assert.deepEqual([idAt(snapshot.nodeIds, 0), idAt(snapshot.nodeIds, 1)], ["a", "bc"]);
});

test("a 3D snapshot is read, and its z column sits between y and the radius", () => {
  // The 3D counterpart of the pinned 2D read: the same nodes, the same x and y, a z column
  // spliced in after y, and the radii one column further along because of it.
  const z = [0.5, -0.5];
  const snapshot = decodeSnapshot(circle(1, [...XY, ...column(z), ...column([1, 3])]));
  assert.equal(snapshot.dim, 1);
  assert.deepEqual([...snapshot.x], [1, -2.5]);
  assert.deepEqual([...snapshot.y], [0, 0.5]);
  assert.deepEqual([...(snapshot.z ?? [])], z);
  assert.deepEqual([...(snapshot.r ?? [])], [1, 3], "the radii follow the z, not the y");
});

test("a 2D snapshot has no z column, and the same columns without one decode identically", () => {
  // The negative control for the z read, and it is the byte-identity claim: a dim of 0
  // splices nothing in, so the radius column starts exactly where it always did. If the z
  // read moved an offset, the radii would come back as z values and this fails.
  const snapshot = decodeSnapshot(circle(0, [...XY, ...column([1, 3])]));
  assert.equal(snapshot.dim, 0);
  assert.equal(snapshot.z, null);
  assert.deepEqual([...snapshot.x], [1, -2.5]);
  assert.deepEqual([...snapshot.y], [0, 0.5]);
  assert.deepEqual([...(snapshot.r ?? [])], [1, 3]);
});

test("a 3D z is a coordinate: it may be negative, and only has to be finite", () => {
  const z = [-1, 1];
  const negative = decodeSnapshot(circle(1, [...XY, ...column(z), ...column([1, 3])]));
  assert.deepEqual([...(negative.z ?? [])], z, "a negative z is a position, not a size");
  // A NaN z is refused under the column's own name, the way a NaN in any other column is.
  const nan = circle(1, [...XY, ...column([Number.NaN, 1]), ...column([1, 3])]);
  assert.deepEqual([refusal(nan).code, refusal(nan).column], ["non-finite", "node.z"]);
});

test("a 3D snapshot with a Point kind carries its z and no size column", () => {
  const bytes = circle(1, [...XY, ...column([1, 1])]);
  bytes[12] = 0;
  const snapshot = decodeSnapshot(bytes);
  assert.equal(snapshot.nodeKind, "Point");
  assert.deepEqual([...(snapshot.z ?? [])], [1, 1]);
  assert.equal(snapshot.r, null);
});

test("a dim past 1 is refused as a reserved dim", () => {
  for (const dim of [2, 3, 127, 255]) {
    const found = refusal(pinned({ 14: dim }));
    assert.deepEqual([found.code, found.column], ["reserved-dim", "dim"], `dim ${dim}`);
    assert.ok(found.message.includes(String(dim)), `dim ${dim} names its value: ${found.message}`);
  }
});

test("the dim check fires at byte 14, ahead of the geometry tags", () => {
  // Byte 12 is a garbage node tag: a reader that reached the tags first would
  // say geometry-tag, so this only passes while dim is read before them.
  const found = refusal(pinned({ 14: 9, 12: 9, 13: 9 }));
  assert.deepEqual([found.code, found.column], ["reserved-dim", "dim"]);
});

test("a 2D snapshot with a garbage tag still fails on the tag, not the dim", () => {
  assert.deepEqual(
    [refusal(pinned({ 14: 0, 12: 9 })).code, refusal(pinned({ 14: 0, 12: 9 })).column],
    ["geometry-tag", "node.kind"],
  );
});
