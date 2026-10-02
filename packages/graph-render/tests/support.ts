/** Shared by the tests: a seeded stream, a frame built from plain arrays, a context recorder. */
import type { Surface2D } from "../src/canvas2d/surface.ts";
import type { Frame } from "../src/frame.ts";

/** One f32 as its four little-endian bytes, so a fixture states values rather than hex. */
export function f32(value: number): number[] {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setFloat32(0, value, true);
  return [...bytes];
}

/** One f32 column, from its values in node order. */
export function column(values: readonly number[]): number[] {
  return values.flatMap(f32);
}

/** One u32 as its four little-endian bytes: an edge endpoint is an index, not a coordinate. */
function u32(value: number): number[] {
  return [...new Uint8Array(Uint32Array.of(value).buffer)];
}

/** The string table a header's node or edge count asks for: offsets, then the bytes, padded. */
export function stringTable(ids: readonly string[]): number[] {
  const encoder = new TextEncoder();
  const parts = ids.map((id) => encoder.encode(id));
  const total = parts.reduce((sum, part) => sum + part.byteLength, 0);
  const offsets = [0];
  for (const part of parts) offsets.push((offsets.at(-1) ?? 0) + part.byteLength);
  const out = new Uint8Array(4 * (ids.length + 1) + total + ((4 - (total % 4)) % 4));
  new Uint32Array(out.buffer).set(offsets);
  let at = 4 * (ids.length + 1);
  for (const part of parts) {
    out.set(part, at);
    at += part.byteLength;
  }
  return [...out];
}

export interface SpaceSpec {
  readonly x: readonly number[];
  readonly y: readonly number[];
  readonly z: readonly number[];
  readonly r?: readonly number[];
  readonly edges?: readonly (readonly [number, number])[];
}

/**
 * Real snapshot bytes for a `dim = 1` drawing: the z column spliced in after y and header
 * byte 14 set to 1, which is what the motor writes for a 3D layout.
 *
 * This exists so a test can take the whole path — bytes, reader, frame, painter — rather
 * than building a `Frame` by hand and testing the painter against its own idea of a column
 * order. A reader that shifted the size column would still pass a hand-built frame's test
 * and fail this one.
 */
export function spaceBytes(spec: SpaceSpec): Uint8Array {
  const nodes = spec.x.length;
  const edges = spec.edges ?? [];
  // magic "GMSN", major 0, minor 0, bytes 12-15 = Circle kind, stage 1, nodes, edges
  const header = new Uint8Array(Uint32Array.of(0x4e534d47, 0, 0, 1, 1, nodes, edges.length).buffer);
  header[14] = 1;
  const ids = Array.from({ length: nodes }, (_, at) => `n${at}`);
  const parts = [
    [...header],
    stringTable(ids),
    stringTable(edges.map((_, at) => `e${at}`)),
    // The endpoints are dense indices, one u32 each, in edge order: the two columns are
    // separate and in this order, which is what `takeEndpoints` reads.
    edges.flatMap((edge) => u32(edge[0])),
    edges.flatMap((edge) => u32(edge[1])),
    // The node columns, coordinates first and contiguous, then the sizes: `x, y, z, r`.
    column(spec.x),
    column(spec.y),
    column(spec.z),
    column(spec.r ?? Array.from({ length: nodes }, () => 4)),
  ];
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(a ^ (a >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface FrameSpec {
  readonly x: readonly number[];
  readonly y: readonly number[];
  readonly edges?: readonly (readonly [number, number])[];
  /** A z per node makes it a 3D frame: the same drawing with the third column the painter projects. */
  readonly z?: readonly number[];
}

export function lineFrame(spec: FrameSpec): Frame {
  const edges = spec.edges ?? [];
  const x = Float32Array.from(spec.x);
  const y = Float32Array.from(spec.y);
  return {
    nodeKind: "Point", edgeKind: "Line", nodeCount: x.length, edgeCount: edges.length, x, y,
    z: spec.z === undefined ? null : Float32Array.from(spec.z),
    r: null, w: null, h: null,
    source: Uint32Array.from(edges, (edge) => edge[0]),
    target: Uint32Array.from(edges, (edge) => edge[1]),
    curveDegree: 0, offsets: null, pts: null,
    bounds: x.length === 0 ? null : {
      minX: Math.min(...spec.x), minY: Math.min(...spec.y), maxX: Math.max(...spec.x), maxY: Math.max(...spec.y),
    },
    factor: 1,
  };
}

export function randomFrame(nodes: number, edges: number, seed: number): Frame {
  const next = mulberry32(seed);
  const x = Array.from({ length: nodes }, () => next() * 1000);
  const y = Array.from({ length: nodes }, () => next() * 1000);
  const pairs = Array.from({ length: edges }, (): [number, number] => [Math.floor(next() * nodes), Math.floor(next() * nodes)]);
  return lineFrame({ x, y, edges: pairs });
}

export interface Recorder {
  readonly ctx: Surface2D;
  readonly calls: Map<string, number>;
  readonly fills: string[];
  /** The endpoints of every linear gradient asked for, and the stops added to them. */
  readonly gradients: number[][];
  readonly stops: string[];
}

export function recorder(): Recorder {
  const calls = new Map<string, number>();
  const fills: string[] = [];
  const gradients: number[][] = [];
  const stops: string[] = [];
  const count = (name: string) => (): void => {
    calls.set(name, (calls.get(name) ?? 0) + 1);
  };
  const ctx: Surface2D = {
    setTransform: count("setTransform"),
    fillRect: count("fillRect"),
    beginPath: count("beginPath"),
    moveTo: count("moveTo"),
    lineTo: count("lineTo"),
    quadraticCurveTo: count("quadraticCurveTo"),
    bezierCurveTo: count("bezierCurveTo"),
    arc: count("arc"),
    rect: count("rect"),
    stroke: count("stroke"),
    drawImage: count("drawImage"),
    createLinearGradient: (x0, y0, x1, y1) => {
      gradients.push([x0, y0, x1, y1]);
      return { addColorStop: (offset, colour) => void stops.push(`${offset}:${colour}`) };
    },
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    globalAlpha: 1,
    fill: (): void => {
      count("fill")();
      fills.push(typeof ctx.fillStyle === "string" ? ctx.fillStyle : "(not a colour)");
    },
  };
  return { ctx, calls, fills, gradients, stops };
}
