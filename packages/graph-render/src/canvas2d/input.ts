/** Everything one painted frame reads. The view owns it and reuses it across frames. */
import type { Adjacency } from "../adjacency.ts";
import type { Camera, Viewport } from "../camera.ts";
import type { Frame } from "../frame.ts";
import type { LabelPlan } from "../labels.ts";
import type { Style } from "../style.ts";
import type { Theme } from "../theme.ts";
import type { SpriteCache } from "./sprites.ts";
import type { Surface2D } from "./surface.ts";

export interface PaintInput {
  readonly ctx: Surface2D;
  readonly viewport: Viewport;
  readonly dpr: number;
  readonly camera: Camera;
  readonly theme: Theme;
  readonly frame: Frame;
  readonly style: Style;
  readonly adjacency: Adjacency;
  /** Where the nodes are now: the frame's own columns, or a blend mid-transition. */
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** World distance from a node's centre to its edge. */
  readonly extent: Float32Array;
  /** False mid-transition: routed edges are drawn straight until the nodes arrive. */
  readonly settled: boolean;
  /** True while the camera or the nodes move: the painter may draw less. */
  readonly moving: boolean;
  /** The node whose neighbourhood is lit, or -1. */
  readonly focus: number;
  /** 1 for the focus and its neighbours. Read only when `focus >= 0`. */
  readonly lit: Uint8Array;
  readonly selected: number;
  readonly labels: LabelPlan;
  readonly sprites: SpriteCache;
}

export interface PaintCounts {
  nodes: number;
  edges: number;
  labels: number;
  /** Path fills and strokes issued: the number batching keeps small. */
  draws: number;
  /** Heads drawn, and the length in CSS pixels of the last one (0 when there are none). */
  arrows: number;
  arrowSize: number;
  /** Edges traced with a control point. */
  curves: number;
  /** `stroke()` calls on edges, and the edge styles (base, lit) that drew at least one edge. */
  strokes: number;
  edgeStyles: number;
  /** Edges whose ends wore two colours, and the strokes each of them took of its own. */
  mixedEdges: number;
  gradientStrokes: number;
  /** Fills issued for arrow heads and for glow discs: each a named budget of its own. */
  arrowFills: number;
  glowFills: number;
  /** The stroke width of an edge in CSS pixels in the last frame. */
  stroke: number;
}

export function newCounts(): PaintCounts {
  return { nodes: 0, edges: 0, labels: 0, draws: 0, arrows: 0, arrowSize: 0, curves: 0, strokes: 0, edgeStyles: 0, mixedEdges: 0, gradientStrokes: 0, arrowFills: 0, glowFills: 0, stroke: 0 };
}
