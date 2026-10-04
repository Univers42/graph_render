// The SDK's reader for the snapshot's binary face (`docs/contract/binary-layout.md`):
// bytes in, the same columns `Motor#column` serves out, keyed by the same `ColumnId`s.
// `./remote` decodes the service's response with it; any caller holding snapshot bytes
// (`Motor#toBytes`, a file) can use it too.
//
// Refused: everything a wrong byte can make a reader misread (header, truncation, trailing
// bytes, offsets, padding, endpoints, non-finite and negative geometry). Not checked: id
// uniqueness, UTF-8 validity, note codes and note order. Those are the producer's checks.

import { ColumnId, type Column, type Dim, type EdgeGeometryKind, type NodeGeometryKind } from "./types.ts";
import {
  SnapshotRefusedError, claim, cursorOver, takeEndpoints, takeF32, takeOffsets, takeOne, takeSize,
  takeStrings, takeU32, type Cursor, type StringTable,
} from "./snapshot/cursor.ts";

export { SnapshotRefusedError, type RefusalCode } from "./snapshot/cursor.ts";

/** One note (format 0.3): a code from `docs/contract/binary-layout.md`, and an edge
 *  position or 4294967295 for a snapshot-wide note. */
export interface Note {
  readonly code: number;
  readonly index: number;
}

/** A decoded snapshot. `column` answers exactly as `Motor#column` does for the same run:
 *  `null` for a column the geometry does not carry, and for the reserved note ids (the notes
 *  are in `notes`). The views alias the decoded buffer. */
export interface Snapshot {
  readonly major: number;
  readonly minor: number;
  readonly nodeKind: NodeGeometryKind;
  readonly edgeKind: EdgeGeometryKind;
  readonly dim: Dim;
  readonly nodeCount: number;
  readonly edgeCount: number;
  /** Node ids in column order: `nodeIds[i]` is the node at row `i` of every node column. */
  readonly nodeIds: readonly string[];
  readonly edgeIds: readonly string[];
  readonly notes: readonly Note[];
  column(columnId: ColumnId): Column;
}

interface Header {
  readonly major: number;
  readonly minor: number;
  readonly dim: Dim;
  readonly nodeKind: NodeGeometryKind;
  readonly edgeKind: EdgeGeometryKind;
  readonly nodeCount: number;
  readonly edgeCount: number;
}

type Columns = Map<ColumnId, Float32Array | Uint32Array>;

const HEADER_BYTES = 28;
const KNOWN_MAJOR = 0;
const NOTES_SINCE_MINOR = 3;
const NODE_KINDS: readonly NodeGeometryKind[] = ["Point", "Circle", "Box"];
const EDGE_KINDS: readonly EdgeGeometryKind[] = ["Line", "Polyline", "Curve"];
const UTF8 = new TextDecoder();

function kindAt<Kind>(kinds: readonly Kind[], tag: number, column: string): Kind {
  const kind = kinds[tag];
  if (kind === undefined) throw new SnapshotRefusedError("geometry-tag", column, `tag ${tag} is not read`);
  return kind;
}

function reserved(column: string, found: number, allowed: number): void {
  if (found !== allowed) throw new SnapshotRefusedError("reserved-field", column, `found ${found}, only ${allowed} is read`);
}

/** Check order is the contract's: magic, major, dim, padding, node tag, edge tag, stages.
 *  The dim byte goes before either tag: a dimension this reader cannot express must be named
 *  before anything is said about a column. */
function takeHeader(cursor: Cursor): Header {
  const start = claim(cursor, HEADER_BYTES, "header");
  const bytes = new Uint8Array(cursor.buffer, start, HEADER_BYTES);
  const words = new Uint32Array(cursor.buffer, start, HEADER_BYTES / 4);
  if (UTF8.decode(bytes.subarray(0, 4)) !== "GMSN") {
    throw new SnapshotRefusedError("bad-magic", "magic", "the first four bytes are not GMSN");
  }
  const major = words[1] ?? 0;
  if (major > KNOWN_MAJOR) {
    throw new SnapshotRefusedError("unsupported-major", "version.major", `found ${major}, reads ${KNOWN_MAJOR}`);
  }
  const dim = bytes[14] ?? 0;
  if (dim !== 0 && dim !== 1) {
    throw new SnapshotRefusedError("reserved-dim", "dim", `found ${dim}, 0 is 2D, 1 is 3D, 2 and up is reserved`);
  }
  reserved("padding", bytes[15] ?? 0, 0);
  const nodeKind = kindAt(NODE_KINDS, bytes[12] ?? 0, "node.kind");
  const edgeKind = kindAt(EDGE_KINDS, bytes[13] ?? 0, "edge.kind");
  reserved("stage.count", words[4] ?? 0, 1);
  return { major, minor: words[2] ?? 0, dim, nodeKind, edgeKind, nodeCount: words[5] ?? 0, edgeCount: words[6] ?? 0 };
}

/** x, y, then z when `dim` is 1 (a coordinate: signed, only finite), then the size columns. */
function takeNodes(cursor: Cursor, header: Header, columns: Columns): void {
  const n = header.nodeCount;
  columns.set(ColumnId.NodeX, takeF32(cursor, n, "node.x"));
  columns.set(ColumnId.NodeY, takeF32(cursor, n, "node.y"));
  if (header.dim === 1) columns.set(ColumnId.NodeZ, takeF32(cursor, n, "node.z"));
  if (header.nodeKind === "Circle") columns.set(ColumnId.NodeR, takeSize(cursor, n, "node.r"));
  if (header.nodeKind === "Box") {
    columns.set(ColumnId.NodeW, takeSize(cursor, n, "node.w"));
    columns.set(ColumnId.NodeH, takeSize(cursor, n, "node.h"));
  }
}

/** Interior points only: an edge's endpoints are its nodes' positions. */
function takeEdges(cursor: Cursor, header: Header, columns: Columns): void {
  if (header.edgeKind === "Line") return;
  if (header.edgeKind === "Curve") {
    const degree = takeU32(cursor, 1, "edge.degree");
    if (degree[0] === 0) throw new SnapshotRefusedError("curve-degree", "edge.degree", "a curve's degree is at least 1");
    columns.set(ColumnId.EdgeCurveDegree, degree);
  }
  const offsets = takeOffsets(cursor, header.edgeCount, "edge.offsets");
  columns.set(ColumnId.EdgeOffsets, offsets);
  columns.set(ColumnId.EdgePts, takeF32(cursor, (offsets[header.edgeCount] ?? 0) * 2, "edge.pts"));
}

function takeNotes(cursor: Cursor, header: Header): readonly Note[] {
  if (header.major === 0 && header.minor < NOTES_SINCE_MINOR) return [];
  const count = takeOne(cursor, "note.count");
  const codes = takeU32(cursor, count, "note.code");
  const indices = takeU32(cursor, count, "note.index");
  return Array.from(codes, (code, i) => ({ code, index: indices[i] ?? 0 }));
}

function idsOf(table: StringTable, count: number): string[] {
  const ids = new Array<string>(count);
  for (let i = 0; i < count; i += 1) {
    ids[i] = UTF8.decode(table.bytes.subarray(table.offsets[i] ?? 0, table.offsets[i + 1] ?? 0));
  }
  return ids;
}

/** Reads one snapshot, or throws {@link SnapshotRefusedError} naming the field it refused. */
export function decodeSnapshot(input: Uint8Array): Snapshot {
  const cursor = cursorOver(input);
  const header = takeHeader(cursor);
  const nodeIds = idsOf(takeStrings(cursor, header.nodeCount, "node.id"), header.nodeCount);
  const edgeIds = idsOf(takeStrings(cursor, header.edgeCount, "edge.id"), header.edgeCount);
  const columns: Columns = new Map();
  columns.set(ColumnId.EdgeSource, takeEndpoints(cursor, header.edgeCount, header.nodeCount, "edge.source"));
  columns.set(ColumnId.EdgeTarget, takeEndpoints(cursor, header.edgeCount, header.nodeCount, "edge.target"));
  takeNodes(cursor, header, columns);
  takeEdges(cursor, header, columns);
  const notes = takeNotes(cursor, header);
  if (cursor.at !== cursor.end) {
    throw new SnapshotRefusedError("trailing-bytes", "snapshot", `${cursor.end - cursor.at} bytes after the last column`);
  }
  return { ...header, nodeIds, edgeIds, notes, column: (columnId) => columns.get(columnId) ?? null };
}
