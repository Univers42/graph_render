/**
 * Snapshot bytes in, typed-array views out. Follows `docs/contract/binary-layout.md`
 * section by section; the views alias the caller's buffer, so nothing is copied unless
 * the bytes start off a 4-byte boundary.
 *
 * Refused here: everything a wrong byte can make the painter misread (header, truncation,
 * trailing bytes, offsets, padding, endpoints, non-finite and negative geometry).
 * Not checked here: id uniqueness, UTF-8 validity, note codes and note order. Those are
 * the producer's construction checks, and a note this reader cannot name is still shown.
 *
 * **2D and 3D.** A `dim = 0` snapshot is read exactly as before, byte for byte: no column
 * moves and no offset shifts, so a 2D render is unchanged. A `dim = 1` snapshot carries a
 * z column, read here as `node.z` between `y` and the size column, and the painter projects
 * it (`three/orbit.ts`). A `dim` of 2 or more is still refused at byte 14, before either
 * geometry tag, under the same check order as the Rust reader: a dim this reader cannot
 * express is a dim it must not read past.
 */

export type NodeKind = "Point" | "Circle" | "Box";
export type EdgeKind = "Line" | "Polyline" | "Curve";

export type RefusalCode =
  | "truncated" | "bad-magic" | "unsupported-major" | "reserved-field" | "reserved-dim"
  | "geometry-tag" | "offsets" | "padding" | "endpoint" | "non-finite" | "negative"
  | "curve-degree" | "trailing-bytes" | "big-endian-host";

export class SnapshotRefusal extends Error {
  readonly code: RefusalCode;
  readonly column: string;

  constructor(code: RefusalCode, column: string, detail: string) {
    super(`snapshot refused (${code}) at ${column}: ${detail}`);
    this.name = "SnapshotRefusal";
    this.code = code;
    this.column = column;
  }
}

export interface StringTable {
  readonly count: number;
  readonly offsets: Uint32Array;
  readonly bytes: Uint8Array;
}

export interface Note {
  readonly code: number;
  /** `null` for a code newer than this reader. */
  readonly name: string | null;
  /** An edge position, or 4294967295 for a snapshot-wide note. */
  readonly index: number;
}

export interface Snapshot {
  readonly major: number;
  readonly minor: number;
  /** 0 for a 2D snapshot with no z column, 1 for one that carries it. Never 2 or more. */
  readonly dim: number;
  readonly nodeKind: NodeKind;
  readonly edgeKind: EdgeKind;
  readonly nodeCount: number;
  readonly edgeCount: number;
  readonly nodeIds: StringTable;
  readonly edgeIds: StringTable;
  readonly source: Uint32Array;
  readonly target: Uint32Array;
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** The third coordinate, one per node; `null` for a 2D snapshot, which has no such column. */
  readonly z: Float32Array | null;
  readonly r: Float32Array | null;
  readonly w: Float32Array | null;
  readonly h: Float32Array | null;
  /** 0 unless the edges are curves. */
  readonly curveDegree: number;
  readonly offsets: Uint32Array | null;
  /** Interior points only: an edge's endpoints are its nodes' positions. */
  readonly pts: Float32Array | null;
  readonly notes: readonly Note[];
}

const HEADER_BYTES = 28;
const KNOWN_MAJOR = 0;
const NOTES_SINCE_MINOR = 3;
const NODE_KINDS: readonly NodeKind[] = ["Point", "Circle", "Box"];
const EDGE_KINDS: readonly EdgeKind[] = ["Line", "Polyline", "Curve"];
const NOTE_NAMES: readonly (string | null)[] = [
  null, "hierarchy.cycle_edge_dropped", "hierarchy.extra_parent_dropped", "packing.approximate",
  "dag.dummy_budget_exceeded", "dag.edge_reversed", "post.route_fallback",
];
const UTF8 = new TextDecoder();

interface Cursor {
  readonly buffer: ArrayBufferLike;
  readonly base: number;
  readonly end: number;
  at: number;
}

function claim(cursor: Cursor, bytes: number, column: string): number {
  if (cursor.at + bytes > cursor.end) {
    const left = cursor.end - cursor.at;
    throw new SnapshotRefusal("truncated", column, `needs ${bytes} bytes, ${left} left`);
  }
  const start = cursor.base + cursor.at;
  cursor.at += bytes;
  return start;
}

function takeU32(cursor: Cursor, count: number, column: string): Uint32Array {
  return new Uint32Array(cursor.buffer, claim(cursor, count * 4, column), count);
}

function takeF32(cursor: Cursor, count: number, column: string): Float32Array {
  const values = new Float32Array(cursor.buffer, claim(cursor, count * 4, column), count);
  for (let i = 0; i < count; i += 1) {
    if (!Number.isFinite(values[i])) throw new SnapshotRefusal("non-finite", column, `element ${i}`);
  }
  return values;
}

function takeSize(cursor: Cursor, count: number, column: string): Float32Array {
  const values = takeF32(cursor, count, column);
  for (let i = 0; i < count; i += 1) {
    if ((values[i] ?? 0) < 0) throw new SnapshotRefusal("negative", column, `element ${i}`);
  }
  return values;
}

function takeOne(cursor: Cursor, column: string): number {
  return takeU32(cursor, 1, column)[0] ?? 0;
}

function takeOffsets(cursor: Cursor, count: number, column: string): Uint32Array {
  const offsets = takeU32(cursor, count + 1, column);
  let previous = 0;
  for (let i = 0; i <= count; i += 1) {
    const offset = offsets[i] ?? 0;
    if (offset < previous || (i === 0 && offset !== 0)) {
      throw new SnapshotRefusal("offsets", column, `offset ${i} is ${offset}`);
    }
    previous = offset;
  }
  return offsets;
}

function takeStrings(cursor: Cursor, count: number, column: string): StringTable {
  const offsets = takeOffsets(cursor, count, column);
  const length = offsets[count] ?? 0;
  const padding = (4 - (length % 4)) % 4;
  const start = claim(cursor, length + padding, column);
  const padded = new Uint8Array(cursor.buffer, start, length + padding);
  for (let i = length; i < padded.length; i += 1) {
    if (padded[i] !== 0) throw new SnapshotRefusal("padding", column, `padding byte ${i - length}`);
  }
  return { count, offsets, bytes: padded.subarray(0, length) };
}

function takeEndpoints(cursor: Cursor, header: Header, column: string): Uint32Array {
  const ends = takeU32(cursor, header.edgeCount, column);
  for (let i = 0; i < ends.length; i += 1) {
    if ((ends[i] ?? 0) >= header.nodeCount) {
      throw new SnapshotRefusal("endpoint", column, `edge ${i} names node ${ends[i]}`);
    }
  }
  return ends;
}

interface Header {
  readonly major: number;
  readonly minor: number;
  /** 0 or 1. A value of 2 or more never reaches here: `takeDim` has already refused it. */
  readonly dim: number;
  readonly nodeKind: NodeKind;
  readonly edgeKind: EdgeKind;
  readonly nodeCount: number;
  readonly edgeCount: number;
}

function reserved(column: string, found: number, allowed: number): void {
  if (found !== allowed) {
    throw new SnapshotRefusal("reserved-field", column, `found ${found}, only ${allowed} is read`);
  }
}

/**
 * The dim byte, read before either geometry tag: a reader that cannot express a snapshot's
 * dimension must say so before it says anything about a column. 0 is 2D and 1 is 3D, both
 * read; anything higher is reserved and refused by name rather than read as one of the two.
 */
function takeDim(found: number): number {
  if (found === 0 || found === 1) return found;
  throw new SnapshotRefusal("reserved-dim", "dim", `found ${found}, 0 is 2D, 1 is 3D, 2 and up is reserved`);
}

function kindAt<Kind>(kinds: readonly Kind[], tag: number, column: string): Kind {
  const kind = kinds[tag];
  if (kind === undefined) throw new SnapshotRefusal("geometry-tag", column, `tag ${tag} is not read`);
  return kind;
}

/** Check order is the contract's: magic, major, dim, padding, node tag, edge tag, stages. */
function takeHeader(cursor: Cursor): Header {
  const start = claim(cursor, HEADER_BYTES, "header");
  const bytes = new Uint8Array(cursor.buffer, start, HEADER_BYTES);
  const words = new Uint32Array(cursor.buffer, start, HEADER_BYTES / 4);
  if (UTF8.decode(bytes.subarray(0, 4)) !== "GMSN") {
    throw new SnapshotRefusal("bad-magic", "magic", "the first four bytes are not GMSN");
  }
  const major = words[1] ?? 0;
  if (major > KNOWN_MAJOR) {
    throw new SnapshotRefusal("unsupported-major", "version.major", `found ${major}, reads ${KNOWN_MAJOR}`);
  }
  const dim = takeDim(bytes[14] ?? 0);
  reserved("padding", bytes[15] ?? 0, 0);
  const nodeKind = kindAt(NODE_KINDS, bytes[12] ?? 0, "node.kind");
  const edgeKind = kindAt(EDGE_KINDS, bytes[13] ?? 0, "edge.kind");
  reserved("stage.count", words[4] ?? 0, 1);
  return { major, minor: words[2] ?? 0, dim, nodeKind, edgeKind, nodeCount: words[5] ?? 0, edgeCount: words[6] ?? 0 };
}

type NodeColumns = Pick<Snapshot, "x" | "y" | "z" | "r" | "w" | "h">;

function takeNodes(cursor: Cursor, header: Header): NodeColumns {
  const n = header.nodeCount;
  const x = takeF32(cursor, n, "node.x");
  const y = takeF32(cursor, n, "node.y");
  // Coordinates first, so `x, y, z` stay contiguous; a z is a coordinate, so it is signed and
  // only has to be finite. The size column below shifts by one when this is present, which
  // is why the reader computes every offset from `dim` rather than from a fixed one.
  const z = header.dim === 1 ? takeF32(cursor, n, "node.z") : null;
  if (header.nodeKind === "Circle") return { x, y, z, r: takeSize(cursor, n, "node.r"), w: null, h: null };
  if (header.nodeKind === "Point") return { x, y, z, r: null, w: null, h: null };
  return { x, y, z, r: null, w: takeSize(cursor, n, "node.w"), h: takeSize(cursor, n, "node.h") };
}

type EdgeColumns = Pick<Snapshot, "curveDegree" | "offsets" | "pts">;

function takeEdges(cursor: Cursor, header: Header): EdgeColumns {
  if (header.edgeKind === "Line") return { curveDegree: 0, offsets: null, pts: null };
  const curveDegree = header.edgeKind === "Curve" ? takeOne(cursor, "edge.degree") : 0;
  if (header.edgeKind === "Curve" && curveDegree === 0) {
    throw new SnapshotRefusal("curve-degree", "edge.degree", "a curve's degree is at least 1");
  }
  const offsets = takeOffsets(cursor, header.edgeCount, "edge.offsets");
  const points = offsets[header.edgeCount] ?? 0;
  return { curveDegree, offsets, pts: takeF32(cursor, points * 2, "edge.pts") };
}

function takeNotes(cursor: Cursor, header: Header): readonly Note[] {
  if (header.major === 0 && header.minor < NOTES_SINCE_MINOR) return [];
  const count = takeOne(cursor, "note.count");
  const codes = takeU32(cursor, count, "note.code");
  const indices = takeU32(cursor, count, "note.index");
  return Array.from(codes, (code, i) => ({ code, name: NOTE_NAMES[code] ?? null, index: indices[i] ?? 0 }));
}

function aligned(bytes: Uint8Array): Uint8Array {
  return bytes.byteOffset % 4 === 0 ? bytes : bytes.slice();
}

function requireLittleEndian(): void {
  if (new Uint8Array(Uint32Array.of(1).buffer)[0] !== 1) {
    throw new SnapshotRefusal("big-endian-host", "host", "the snapshot is little-endian and is viewed in place");
  }
}

export function decodeSnapshot(input: Uint8Array): Snapshot {
  requireLittleEndian();
  const bytes = aligned(input);
  const cursor: Cursor = { buffer: bytes.buffer, base: bytes.byteOffset, end: bytes.byteLength, at: 0 };
  const header = takeHeader(cursor);
  const nodeIds = takeStrings(cursor, header.nodeCount, "node.id");
  const edgeIds = takeStrings(cursor, header.edgeCount, "edge.id");
  const source = takeEndpoints(cursor, header, "edge.source");
  const target = takeEndpoints(cursor, header, "edge.target");
  const nodes = takeNodes(cursor, header);
  const edges = takeEdges(cursor, header);
  const notes = takeNotes(cursor, header);
  if (cursor.at !== cursor.end) {
    throw new SnapshotRefusal("trailing-bytes", "snapshot", `${cursor.end - cursor.at} bytes after the last column`);
  }
  return { ...header, nodeIds, edgeIds, source, target, ...nodes, ...edges, notes };
}

export function idAt(table: StringTable, index: number): string {
  const start = table.offsets[index];
  const end = table.offsets[index + 1];
  if (start === undefined || end === undefined) return "";
  return UTF8.decode(table.bytes.subarray(start, end));
}
