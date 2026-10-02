/**
 * The settled picture of the GPU layer, filled over several frames and then kept. It is two
 * textures on the GPU now (picture.ts): a settled frame adds one chunk of edges to the edge
 * texture and the nodes go once into the node texture, and the picture is shown by two quads and
 * read back once, where every chunk used to leave the GPU as an ImageBitmap and be composited
 * into a kept 2D canvas, two canvas-sized copies a chunk. Drawing every edge in one settled frame
 * cost 3.9 to 4.5 s at 1M nodes on SwiftShader, 99% of it inside transferToImageBitmap
 * (target/p5-hover.py), and a hover, a leave, every fade frame and the settle after a pan each
 * paid it again on the page's only thread. The chunks are paced like the moving budget, on what
 * the whole frame cost, the readback included; a focus or a fade only changes the alpha the quads
 * are drawn at, under the lit edges the 2D painter draws.
 *
 * Caveat: the dim is one alpha over the whole picture where the moving frames and the 2D painter
 * dim each edge, so where dimmed edges cross the picture reads fainter than they would (two
 * half-alpha edges at dim 0.3: 0.225 against 0.278). While it fills, the view shows the spread
 * order's prefix, a sample of the edges, for as many frames as the chunks take. The texture is the
 * canvas's own size in device pixels, clamped to the driver's texture limit (picture.ts), so a
 * driver below the canvas size shows it stretched; any change of camera, size, positions, style or
 * theme colour starts it again from nothing.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { counted, drawChunk } from "./draw.ts";
import type { BulkLayer } from "./layer.ts";
import { nextBudget } from "./plan.ts";
import { newPicture, type Picture, pictureSize, resetPicture, showPicture } from "./picture.ts";
import { sameRefs } from "./sync.ts";

export interface Still {
  /** What the picture is of (`viewOf`): any change starts it again. */
  view: readonly unknown[];
  readonly picture: Picture;
  /** Edge pairs in the picture, a prefix of the layer's spread order. */
  drawn: number;
  /** The last picture shown, for the glide over it (glide.ts); null until a frame has shown one. */
  shown: ImageBitmap | null;
  /** Whether the nodes are in the picture: they go in once, on the first settled frame. */
  hasNodes: boolean;
  nodeCount: number;
  /** Pairs the next settled frame adds, paced by `nextBudget` on what the last chunk cost. */
  chunk: number;
}

/** Null where this driver will not give the picture's two framebuffers (then each frame paints whole). */
export function newStill(layer: BulkLayer, chunk: number): Still | null {
  const picture = newPicture(layer);
  if (picture === null) return null;
  return { view: [], picture, drawn: 0, shown: null, hasNodes: false, nodeCount: 0, chunk };
}

/** Everything the picture depends on; the focus and the dim alpha are not in it. */
export function viewOf(input: PaintInput, placed: number): readonly unknown[] {
  const { camera, viewport, theme } = input;
  return [camera.x, camera.y, camera.scale, viewport.width, viewport.height, input.dpr, placed,
    input.frame, input.x, input.y, input.extent, input.style, theme.edge, theme.rim];
}

function restart(still: Still, view: readonly unknown[], layer: BulkLayer, input: PaintInput): void {
  resetPicture(still.picture, layer, pictureSize(input, layer.maxTexture));
  still.view = view;
  still.drawn = 0;
  still.hasNodes = false;
  still.nodeCount = 0;
  still.shown?.close();
  still.shown = null;
}

/** The layer a picture is drawn with, and the view's in-place move counter (`BulkSlot`). */
export interface Source {
  readonly layer: BulkLayer;
  readonly placed: number;
}

/** Adds the nodes once and one chunk of edges to the picture's textures; false when the context is lost. */
function grow(still: Still, { layer, placed }: Source, input: PaintInput): boolean {
  if (!still.hasNodes) {
    const drawn = drawChunk(layer, still.picture.nodes, { input, placed, first: 0, count: 0, nodes: true });
    if (drawn < 0) return false;
    still.hasNodes = true;
    still.nodeCount = drawn;
  }
  const pairs = layer.uploaded.indexCount / 2;
  if (still.drawn >= pairs) return true;
  const drawn = drawChunk(layer, still.picture.edges, { input, placed, first: still.drawn, count: still.chunk, nodes: false });
  if (drawn < 0) return false;
  still.drawn += drawn;
  return true;
}

/**
 * Grows the picture by one chunk, shows it at the frame's dim and hands the bitmap over; the chunk
 * is paced on what the whole frame cost, the readback the driver waits in included. Returns the
 * edge pairs the picture still lacks, or -1 when the context is lost.
 */
export function paintStill(still: Still, source: Source, input: PaintInput, counts: PaintCounts): number {
  const { layer } = source;
  const view = viewOf(input, source.placed);
  if (!sameRefs(still.view, view)) restart(still, view, layer, input);
  const started = performance.now();
  if (!grow(still, source, input)) return -1;
  showPicture(still.picture, layer, input.focus >= 0 ? input.theme.dimAlpha : 1);
  const picture = layer.canvas.transferToImageBitmap();
  still.chunk = nextBudget(still.chunk, performance.now() - started, layer.uploaded.indexCount / 2);
  still.shown?.close();
  still.shown = picture;
  const { ctx, viewport } = input;
  ctx.drawImage(picture, 0, 0, viewport.width, viewport.height);
  counted(counts, still.drawn, still.nodeCount, input.dpr);
  return layer.uploaded.indexCount / 2 - still.drawn;
}