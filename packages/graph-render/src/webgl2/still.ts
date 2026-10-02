/**
 * The settled picture of the GPU layer, filled over several frames and then kept. Drawing
 * every edge in one settled frame cost 3.9 to 4.5 s at 1M nodes on SwiftShader, 99% of it
 * inside transferToImageBitmap (target/p5-hover.py), and a hover, a leave, every fade frame
 * and the settle after a pan each paid it again on the page's only thread. A settled frame
 * now adds one chunk of edges to a kept 2D picture, paced like the moving budget, and asks
 * for another frame until the picture holds them all; the nodes go once into a picture of
 * their own; a focus or a fade only blits the two at the dim alpha, under the lit edges the
 * 2D painter draws.
 *
 * Caveat: the dim is one alpha over the whole picture where the moving frames and the 2D
 * painter dim each edge, so where dimmed edges cross the picture reads fainter than they
 * would (two half-alpha edges at dim 0.3: 0.225 against 0.278). While it fills, the view
 * shows the spread order's prefix, a sample of the edges, for as many frames as the chunks
 * take. It is a copy at canvas resolution: any change of camera, size, positions, style or
 * theme colour starts it again from nothing.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { counted, deviceSize, edgePart, nodePart } from "./draw.ts";
import type { BulkLayer } from "./layer.ts";
import { nextBudget } from "./plan.ts";
import { sameRefs } from "./sync.ts";

export interface Still {
  /** What the picture is of (`viewOf`): any change starts it again. */
  view: readonly unknown[];
  readonly canvas: OffscreenCanvas;
  readonly ctx: OffscreenCanvasRenderingContext2D;
  /** Edge pairs in the picture, a prefix of the layer's spread order. */
  drawn: number;
  nodes: ImageBitmap | null;
  nodeCount: number;
  /** Pairs the next settled frame adds, paced by `nextBudget` on what the last chunk cost. */
  chunk: number;
}

/** Null where there is no 2D OffscreenCanvas to keep the picture on. */
export function newStill(chunk: number): Still | null {
  if (typeof OffscreenCanvas === "undefined") return null;
  const canvas = new OffscreenCanvas(1, 1);
  const ctx = canvas.getContext("2d");
  if (ctx === null) return null;
  return { view: [], canvas, ctx, drawn: 0, nodes: null, nodeCount: 0, chunk };
}

/** Everything the picture depends on; the focus and the dim alpha are not in it. */
export function viewOf(input: PaintInput, placed: number): readonly unknown[] {
  const { camera, viewport, theme } = input;
  return [camera.x, camera.y, camera.scale, viewport.width, viewport.height, input.dpr, placed,
    input.frame, input.x, input.y, input.extent, input.style, theme.edge, theme.rim];
}

function restart(still: Still, view: readonly unknown[], input: PaintInput): void {
  const [width, height] = deviceSize(input);
  if (still.canvas.width !== width || still.canvas.height !== height) {
    still.canvas.width = width;
    still.canvas.height = height;
  } else {
    still.ctx.clearRect(0, 0, width, height);
  }
  still.view = view;
  still.drawn = 0;
  still.nodes?.close();
  still.nodes = null;
}

/** The layer a picture is drawn with, and the view's in-place move counter (`BulkSlot`). */
export interface Source {
  readonly layer: BulkLayer;
  readonly placed: number;
}

/** Adds the nodes once and one chunk of edges to the picture; false when the context is lost. */
function grow(still: Still, { layer, placed }: Source, input: PaintInput): boolean {
  if (still.nodes === null) {
    const nodes = nodePart(layer, input, placed);
    if (nodes === null) return false;
    still.nodes = nodes.bitmap;
    still.nodeCount = nodes.drawn;
  }
  const pairs = layer.uploaded.indexCount / 2;
  if (still.drawn >= pairs) return true;
  const started = performance.now();
  const edges = edgePart(layer, input, placed, { first: still.drawn, count: still.chunk });
  if (edges === null) return false;
  still.chunk = nextBudget(still.chunk, performance.now() - started, pairs);
  still.ctx.drawImage(edges.bitmap, 0, 0);
  edges.bitmap.close();
  still.drawn += edges.drawn;
  return true;
}

/**
 * Grows the picture by one chunk and blits it onto the frame's 2D context. Returns the edge
 * pairs it still lacks, or -1 when the context is lost.
 */
export function paintStill(still: Still, source: Source, input: PaintInput, counts: PaintCounts): number {
  const view = viewOf(input, source.placed);
  if (!sameRefs(still.view, view)) restart(still, view, input);
  if (!grow(still, source, input)) return -1;
  const { ctx, viewport } = input;
  ctx.globalAlpha = input.focus >= 0 ? input.theme.dimAlpha : 1;
  ctx.drawImage(still.canvas, 0, 0, viewport.width, viewport.height);
  if (still.nodes !== null) ctx.drawImage(still.nodes, 0, 0, viewport.width, viewport.height);
  ctx.globalAlpha = 1;
  counted(counts, still.drawn, still.nodeCount, input.dpr);
  return source.layer.uploaded.indexCount / 2 - still.drawn;
}
