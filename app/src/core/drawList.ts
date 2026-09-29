/**
 * Column → draw list. A run's zero-copy columns are turned ONCE into plain
 * arrays the canvas can draw every frame, because a `Column` is only valid until
 * the next call on the motor (C7) and a draw loop must never touch wasm memory.
 *
 * This is also where the six geometry kinds become shapes: a `Point` node draws at
 * one fixed radius (it carries no size column at all), a `Circle` at its `r`, a
 * `Box` at its `w`/`h`; a `Line` edge carries no points, and a `Polyline`/`Curve`
 * edge is the window `offsets` names inside the flat `pts` array. Every refusal
 * here is loud — a missing size column or a non-monotonic offset run is a
 * contract violation, and drawing it as zeroes would be a silently wrong picture.
 */

import type { EdgeGeometryKind, NodeGeometryKind } from "../../../crates/graph-sdk-js/src/index.ts";

/** Radius a `Point` node draws at, in world units. Pinned: it is the studio's
 *  only free size constant, and every Point snapshot looks the same because of it. */
export const POINT_RADIUS = 4.5;

/** A contract violation in the columns a run handed back. */
export class DrawListError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DrawListError";
  }
}

/** One node, resolved to a drawable rectangle plus a hit radius. */
export interface NodeDraw {
  readonly index: number;
  readonly x: number;
  readonly y: number;
  /** Full width; 0 for a Point, `2r` for a Circle. */
  readonly w: number;
  /** Full height; 0 for a Point, `2r` for a Circle. */
  readonly h: number;
  /** Hit/draw radius: `r` for a Circle, half the short side for a Box. */
  readonly r: number;
}

/** One edge. `pts` is empty for a `Line`, and a flat x/y run per edge otherwise. */
export interface EdgeDraw {
  readonly index: number;
  readonly source: number;
  readonly target: number;
  readonly pts: Float32Array;
  /** The snapshot-wide curve degree; 0 for every kind but `Curve`. */
  readonly degree: number;
}

/** Everything one run's geometry means to the renderer, copied out of wasm. */
export interface DrawList {
  readonly nodeKind: NodeGeometryKind;
  readonly edgeKind: EdgeGeometryKind;
  readonly nodes: readonly NodeDraw[];
  readonly edges: readonly EdgeDraw[];
}

/** The columns one run produced, as the SDK handed them over (`null` = absent). */
export interface ColumnInput {
  readonly nodeKind: NodeGeometryKind;
  readonly edgeKind: EdgeGeometryKind;
  readonly x: Float32Array | null;
  readonly y: Float32Array | null;
  readonly r: Float32Array | null;
  readonly w: Float32Array | null;
  readonly h: Float32Array | null;
  readonly source: Uint32Array | null;
  readonly target: Uint32Array | null;
  readonly offsets: Uint32Array | null;
  readonly pts: Float32Array | null;
  readonly curveDegree: Uint32Array | null;
}

const NO_POINTS = new Float32Array(0);

/** Resolve the size of node `index` under `kind` — the whole Point/Circle/Box split. */
function nodeExtent(kind: NodeGeometryKind, input: ColumnInput, index: number): { w: number; h: number; r: number } {
  if (kind === "Point") return { w: 0, h: 0, r: POINT_RADIUS };
  if (kind === "Circle") {
    if (input.r === null) throw new DrawListError("Circle nodes need the r column");
    const r = input.r[index] ?? 0;
    return { w: r * 2, h: r * 2, r };
  }
  if (input.w === null || input.h === null) throw new DrawListError("Box nodes need the w and h columns");
  const w = input.w[index] ?? 0;
  const h = input.h[index] ?? 0;
  return { w, h, r: Math.min(w, h) / 2 };
}

function buildNodes(kind: NodeGeometryKind, input: ColumnInput, count: number): NodeDraw[] {
  const x = input.x;
  const y = input.y;
  if (x === null || y === null) throw new DrawListError("a run with nodes must carry the x and y columns");
  if (x.length !== y.length) throw new DrawListError("x and y must be the same length");
  const nodes: NodeDraw[] = [];
  for (let index = 0; index < count; index += 1) {
    const size = nodeExtent(kind, input, index);
    nodes.push({ index, x: x[index], y: y[index], ...size });
  }
  return nodes;
}

/** The flat `pts` window edge `index` owns, copied so no wasm view escapes. */
function edgePoints(input: ColumnInput, edgeCount: number, index: number): Float32Array {
  const offsets = requireOffsets(input, edgeCount);
  if (input.pts === null) throw new DrawListError("Polyline and Curve edges need the pts column");
  const from = offsets[index] * 2;
  const to = offsets[index + 1] * 2;
  if (to > input.pts.length) throw new DrawListError("offsets run past the end of pts");
  return input.pts.slice(from, to);
}

/** The offsets column, checked ONCE per run rather than per edge: the whole
 *  array is one non-decreasing run from 0, and a decrease in the final pair
 *  (which belongs to no edge) is as malformed as one in the middle. */
function requireOffsets(input: ColumnInput, edgeCount: number): Uint32Array {
  const offsets = input.offsets;
  if (offsets === null) throw new DrawListError("Polyline and Curve edges need the offsets column");
  if (offsets.length !== edgeCount + 1) {
    throw new DrawListError(`offsets must have one entry per edge plus one (${offsets.length} for ${edgeCount})`);
  }
  if (offsets[0] !== 0) throw new DrawListError("offsets must start at 0");
  for (let i = 1; i < offsets.length; i += 1) {
    if (offsets[i] < offsets[i - 1]) throw new DrawListError("offsets must not decrease");
  }
  return offsets;
}

function buildEdges(kind: EdgeGeometryKind, input: ColumnInput, nodeCount: number): EdgeDraw[] {
  const source = input.source;
  const target = input.target;
  if (source === null || target === null) throw new DrawListError("a run with edges must carry the source and target columns");
  if (source.length !== target.length) throw new DrawListError("edge source and target must be the same length");
  const edges: EdgeDraw[] = [];
  for (let index = 0; index < source.length; index += 1) {
    edges.push({
      index,
      source: checkedEndpoint(source[index], nodeCount, index, "starts at"),
      target: checkedEndpoint(target[index], nodeCount, index, "targets"),
      pts: kind === "Line" ? NO_POINTS : edgePoints(input, source.length, index),
      degree: curveDegreeOf(kind, input),
    });
  }
  return edges;
}

function checkedEndpoint(node: number, nodeCount: number, edge: number, verb: string): number {
  if (node >= nodeCount) throw new DrawListError(`edge ${edge} ${verb} node ${node} of ${nodeCount}`);
  return node;
}

/** `Curve` is the only kind that carries a degree, and it carries ONE for the
 *  whole snapshot — the same value on every edge, by construction. */
function curveDegreeOf(kind: EdgeGeometryKind, input: ColumnInput): number {
  if (kind !== "Curve") return 0;
  if (input.curveDegree === null) throw new DrawListError("Curve edges need the degree column");
  return input.curveDegree[0] ?? 0;
}

/** Copy one run's columns into a draw list. The only place a `Column` is read. */
export function buildDrawList(input: ColumnInput): DrawList {
  const count = input.x === null ? 0 : input.x.length;
  if (count === 0) throw new DrawListError("a run with no x column has nothing to draw");
  return {
    nodeKind: input.nodeKind,
    edgeKind: input.edgeKind,
    nodes: buildNodes(input.nodeKind, input, count),
    edges: buildEdges(input.edgeKind, input, count),
  };
}

const COLUMN_NAMES = [
  "node.x", "node.y", "node.r", "node.w", "node.h",
  "edge.source", "edge.target", "edge.offsets", "edge.pts", "edge.curveDegree",
  "note.code", "note.index",
] as const;

/** One row per contract column, in contract order, for the status panel: the
 *  length of the column a run carried, or `null` when the geometry has no such
 *  column (`ColumnId`'s reserved note ids are always null, per C3). */
export function describeColumns(input: ColumnInput): { name: string; length: number | null }[] {
  const lengths = [
    input.x?.length ?? null, input.y?.length ?? null, input.r?.length ?? null,
    input.w?.length ?? null, input.h?.length ?? null, input.source?.length ?? null,
    input.target?.length ?? null, input.offsets?.length ?? null, input.pts?.length ?? null,
    input.curveDegree?.length ?? null, null, null,
  ];
  return COLUMN_NAMES.map((name, i) => ({ name, length: lengths[i] ?? null }));
}
