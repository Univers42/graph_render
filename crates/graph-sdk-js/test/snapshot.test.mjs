// The SDK's snapshot reader on the contract's own pinned bytes (`docs/contract/binary-layout.md`
// "The pinned 84-byte example", its Curve and one-note variants, "A 0.2 snapshot") and on one
// patch per refusal. The hex is the doc's table, row by row.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/snapshot.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";
import { ColumnId } from "../src/types.ts";
import { decodeSnapshot, SnapshotRefusedError } from "../src/snapshot.ts";

const hex = (text) => Uint8Array.from(text.trim().split(/\s+/), (byte) => Number.parseInt(byte, 16));

const FIRST_80 = hex(`
  47 4D 53 4E  00 00 00 00  03 00 00 00  00 00 00 00  01 00 00 00  02 00 00 00  01 00 00 00
  00 00 00 00  01 00 00 00  03 00 00 00  61 62 63 00
  00 00 00 00  01 00 00 00  65 00 00 00
  00 00 00 00  01 00 00 00
  00 00 80 3F  00 00 20 C0  00 00 00 00  00 00 00 3F`);
const NO_NOTES = hex("00 00 00 00");
const PINNED = Uint8Array.of(...FIRST_80, ...NO_NOTES);
const CURVE_TAIL = hex("02 00 00 00  00 00 00 00  01 00 00 00  00 00 80 3F  00 00 00 3F");
const ONE_NOTE = hex("01 00 00 00  03 00 00 00  FF FF FF FF");

const patched = (bytes, offset, ...values) => {
  const copy = bytes.slice();
  copy.set(values, offset);
  return copy;
};

function refusal(bytes, expected, column) {
  assert.throws(() => decodeSnapshot(bytes), (error) => {
    assert.ok(error instanceof SnapshotRefusedError, String(error));
    assert.equal(error.refusal, expected, error.message);
    if (column !== undefined) assert.equal(error.column, column, error.message);
    return true;
  });
}

test("the pinned 84-byte example reads back field for field", () => {
  assert.equal(PINNED.length, 84);
  const snapshot = decodeSnapshot(PINNED);
  assert.deepEqual([snapshot.major, snapshot.minor, snapshot.nodeKind, snapshot.edgeKind, snapshot.dim], [0, 3, "Point", "Line", 0]);
  assert.deepEqual([snapshot.nodeIds, snapshot.edgeIds, snapshot.notes], [["a", "bc"], ["e"], []]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.EdgeSource)), [0]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.EdgeTarget)), [1]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.NodeX)), [1, -2.5]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.NodeY)), [0, 0.5]);
  for (const absent of [ColumnId.NodeZ, ColumnId.NodeR, ColumnId.NodeW, ColumnId.EdgeOffsets, ColumnId.EdgeCurveDegree, ColumnId.NoteCode]) {
    assert.equal(snapshot.column(absent), null);
  }
});

test("the Curve variant: byte 13 is 02, then degree, offsets and points", () => {
  const snapshot = decodeSnapshot(Uint8Array.of(...patched(FIRST_80, 13, 2), ...CURVE_TAIL, ...NO_NOTES));
  assert.equal(snapshot.edgeKind, "Curve");
  assert.deepEqual(Array.from(snapshot.column(ColumnId.EdgeCurveDegree)), [2]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.EdgeOffsets)), [0, 1]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.EdgePts)), [1, 0.5]);
});

test("the one-note example and a 0.2 snapshot", () => {
  const noted = Uint8Array.of(...FIRST_80, ...ONE_NOTE);
  assert.equal(noted.length, 92);
  assert.deepEqual(decodeSnapshot(noted).notes, [{ code: 3, index: 4294967295 }]);
  const older = decodeSnapshot(patched(FIRST_80, 8, 2));
  assert.equal(older.minor, 2);
  assert.deepEqual(older.notes, []);
});

test("bytes off a 4-byte boundary read the same", () => {
  const shifted = new Uint8Array(PINNED.length + 1);
  shifted.set(PINNED, 1);
  assert.deepEqual(Array.from(decodeSnapshot(shifted.subarray(1)).column(ColumnId.NodeX)), [1, -2.5]);
});

test("every truncation is refused, and the first 80 bytes labelled 0.3 stop at note.count", () => {
  for (let length = 0; length < PINNED.length; length += 1) refusal(PINNED.subarray(0, length), "truncated");
  refusal(FIRST_80, "truncated", "note.count");
  refusal(Uint8Array.of(...PINNED, 0, 0, 0, 0), "trailing-bytes");
});

test("one patch per refusal names the field", () => {
  refusal(patched(PINNED, 0, 0x47, 0x4d, 0x53, 0x4f), "bad-magic", "magic");
  refusal(patched(PINNED, 4, 1), "unsupported-major", "version.major");
  refusal(patched(PINNED, 12, 9, 9, 2), "reserved-dim", "dim");
  refusal(patched(PINNED, 15, 1), "reserved-field", "padding");
  refusal(patched(PINNED, 12, 9), "geometry-tag", "node.kind");
  refusal(patched(PINNED, 16, 2), "reserved-field", "stage.count");
  refusal(patched(PINNED, 32, 4), "offsets", "node.id");
  refusal(patched(PINNED, 43, 1), "padding", "node.id");
  refusal(patched(PINNED, 60, 2), "endpoint", "edge.target");
  refusal(patched(PINNED, 76, 0x00, 0x00, 0xc0, 0x7f), "non-finite", "node.y");
  refusal(Uint8Array.of(...patched(FIRST_80, 13, 2), ...patched(CURVE_TAIL, 0, 0), ...NO_NOTES), "curve-degree", "edge.degree");
});
