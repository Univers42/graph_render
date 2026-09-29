/** Shared by the tests: a seeded stream, a frame built from plain arrays, a context recorder. */
import type { Surface2D } from "../src/canvas2d/surface.ts";
import type { Frame } from "../src/frame.ts";

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
}

export function lineFrame(spec: FrameSpec): Frame {
  const edges = spec.edges ?? [];
  const x = Float32Array.from(spec.x);
  const y = Float32Array.from(spec.y);
  return {
    nodeKind: "Point", edgeKind: "Line", nodeCount: x.length, edgeCount: edges.length, x, y,
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
}

export function recorder(): Recorder {
  const calls = new Map<string, number>();
  const fills: string[] = [];
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
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    globalAlpha: 1,
    fill: (): void => {
      count("fill")();
      fills.push(typeof ctx.fillStyle === "string" ? ctx.fillStyle : "(not a colour)");
    },
  };
  return { ctx, calls, fills };
}
