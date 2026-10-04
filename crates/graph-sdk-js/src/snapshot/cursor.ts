// The byte-level half of the snapshot reader (`docs/contract/binary-layout.md`): a cursor
// over one little-endian buffer and the typed takes every column is made of. Views alias
// the caller's buffer; nothing is copied unless the bytes start off a 4-byte boundary.
//
// Ported from `packages/graph-render/src/snapshot/decode.ts`, not imported: the layer table
// forbids the SDK and the renderer from depending on each other, and both read the same
// spec. A divergence between the two shows up as a red `svc-sdk` row, which compares this
// reader's columns with the wasm `Motor`'s own.

import { GraphMotorError } from "../errors.ts";

export type RefusalCode =
  | "truncated" | "bad-magic" | "unsupported-major" | "reserved-field" | "reserved-dim"
  | "geometry-tag" | "offsets" | "padding" | "endpoint" | "non-finite" | "negative"
  | "curve-degree" | "trailing-bytes" | "big-endian-host";

/** The bytes are not a snapshot this reader can read. `refusal` names the rule broken and
 *  `column` the field it was broken in, so a caller never has to parse the message. */
export class SnapshotRefusedError extends GraphMotorError {
  readonly refusal: RefusalCode;
  readonly column: string;

  constructor(refusal: RefusalCode, column: string, detail: string) {
    super(`snapshot refused (${refusal}) at ${column}: ${detail}`);
    this.refusal = refusal;
    this.column = column;
  }
}

export interface Cursor {
  readonly buffer: ArrayBufferLike;
  readonly base: number;
  readonly end: number;
  at: number;
}

export interface StringTable {
  readonly offsets: Uint32Array;
  readonly bytes: Uint8Array;
}

export function cursorOver(input: Uint8Array): Cursor {
  if (new Uint8Array(Uint32Array.of(1).buffer)[0] !== 1) {
    throw new SnapshotRefusedError("big-endian-host", "host", "the snapshot is little-endian and is viewed in place");
  }
  const bytes = input.byteOffset % 4 === 0 ? input : input.slice();
  return { buffer: bytes.buffer, base: bytes.byteOffset, end: bytes.byteLength, at: 0 };
}

export function claim(cursor: Cursor, bytes: number, column: string): number {
  if (cursor.at + bytes > cursor.end) {
    const left = cursor.end - cursor.at;
    throw new SnapshotRefusedError("truncated", column, `needs ${bytes} bytes, ${left} left`);
  }
  const start = cursor.base + cursor.at;
  cursor.at += bytes;
  return start;
}

export function takeU32(cursor: Cursor, count: number, column: string): Uint32Array {
  return new Uint32Array(cursor.buffer, claim(cursor, count * 4, column), count);
}

export function takeOne(cursor: Cursor, column: string): number {
  return takeU32(cursor, 1, column)[0] ?? 0;
}

export function takeF32(cursor: Cursor, count: number, column: string): Float32Array {
  const values = new Float32Array(cursor.buffer, claim(cursor, count * 4, column), count);
  for (let i = 0; i < count; i += 1) {
    if (!Number.isFinite(values[i])) throw new SnapshotRefusedError("non-finite", column, `element ${i}`);
  }
  return values;
}

/** A width, height or radius: finite and never negative. */
export function takeSize(cursor: Cursor, count: number, column: string): Float32Array {
  const values = takeF32(cursor, count, column);
  for (let i = 0; i < count; i += 1) {
    if ((values[i] ?? 0) < 0) throw new SnapshotRefusedError("negative", column, `element ${i}`);
  }
  return values;
}

export function takeOffsets(cursor: Cursor, count: number, column: string): Uint32Array {
  const offsets = takeU32(cursor, count + 1, column);
  let previous = 0;
  for (let i = 0; i <= count; i += 1) {
    const offset = offsets[i] ?? 0;
    if (offset < previous || (i === 0 && offset !== 0)) {
      throw new SnapshotRefusedError("offsets", column, `offset ${i} is ${offset}`);
    }
    previous = offset;
  }
  return offsets;
}

export function takeStrings(cursor: Cursor, count: number, column: string): StringTable {
  const offsets = takeOffsets(cursor, count, column);
  const length = offsets[count] ?? 0;
  const padding = (4 - (length % 4)) % 4;
  const padded = new Uint8Array(cursor.buffer, claim(cursor, length + padding, column), length + padding);
  for (let i = length; i < padded.length; i += 1) {
    if (padded[i] !== 0) throw new SnapshotRefusedError("padding", column, `padding byte ${i - length}`);
  }
  return { offsets, bytes: padded.subarray(0, length) };
}

/** An edge's source or target column: every entry names a node that exists. */
export function takeEndpoints(cursor: Cursor, edges: number, nodes: number, column: string): Uint32Array {
  const ends = takeU32(cursor, edges, column);
  for (let i = 0; i < ends.length; i += 1) {
    if ((ends[i] ?? 0) >= nodes) throw new SnapshotRefusedError("endpoint", column, `edge ${i} names node ${ends[i]}`);
  }
  return ends;
}
