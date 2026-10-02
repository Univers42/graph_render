/**
 * Moving frames over a kept picture. A moving GPU frame at 1M nodes cost about 33 ms on
 * SwiftShader, 52% of the main thread inside transferToImageBitmap (target/p5-zoom-follow.log),
 * which held a continuous zoom under 30 fps. A moving frame now redraws the last whole picture
 * under the camera's change, one blit whatever the node count, and asks the GPU for a fresh
 * picture only once the kept one is too far off: zoomed past GLIDE_ZOOM either way, or moved
 * so that more than GLIDE_EXPOSED of the viewport would show nothing. After a settle the kept
 * picture is the still one (still.ts), every edge in it; after that, the last fresh moving frame.
 *
 * Caveat: a zoom-in shows the old picture magnified, up to GLIDE_ZOOM softer, and its nodes and
 * strokes grow with it; a pan or a zoom-out leaves bare background where the old picture does
 * not reach, up to GLIDE_EXPOSED of the view. Both last until the next fresh frame or the
 * settle. A frame with a focus never glides: its dimming is drawn into the picture.
 */
import type { Camera, Viewport } from "../camera.ts";
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { counted } from "./draw.ts";
import type { Still } from "./still.ts";
import { sameRefs } from "./sync.ts";

const GLIDE_ZOOM = Math.SQRT2;
const GLIDE_EXPOSED = 0.2;

export interface Glide {
  /** `viewOf` the kept picture: the camera, then everything else it depends on. Empty when there is none. */
  view: readonly unknown[];
  pictures: readonly CanvasImageSource[];
  /** The fresh frame this glide owns and closes, or null when it shows the still picture. */
  owned: ImageBitmap | null;
  edges: number;
  nodes: number;
}

export function newGlide(): Glide {
  return { view: [], pictures: [], owned: null, edges: 0, nodes: 0 };
}

/** Forgets the kept picture: a settled frame does, so the next move starts from the still one. */
export function dropGlide(glide: Glide): void {
  glide.owned?.close();
  glide.owned = null;
  glide.pictures = [];
  glide.view = [];
}

/** Keeps a fresh moving frame as the picture the next moving frames glide over. */
export function keepFrame(glide: Glide, picture: ImageBitmap, view: readonly unknown[], counts: PaintCounts): void {
  dropGlide(glide);
  glide.owned = picture;
  glide.pictures = [picture];
  glide.view = view;
  glide.edges = counts.edges;
  glide.nodes = counts.nodes;
}

function adoptStill(glide: Glide, still: Still): void {
  if (still.nodes === null) return;
  glide.pictures = [still.canvas, still.nodes];
  glide.view = still.view;
  glide.edges = still.drawn;
  glide.nodes = still.nodeCount;
}

function numberAt(view: readonly unknown[], index: number): number {
  const value = view[index];
  return typeof value === "number" ? value : Number.NaN;
}

/** Where the kept picture lands under `to`, in CSS pixels, or null when it is too far off. */
export function landing(from: readonly unknown[], to: Camera, viewport: Viewport): Camera | null {
  const k = to.scale / numberAt(from, 2);
  if (!(k <= GLIDE_ZOOM && k >= 1 / GLIDE_ZOOM)) return null;
  const at = { x: to.x - numberAt(from, 0) * k, y: to.y - numberAt(from, 1) * k, scale: k };
  const { width, height } = viewport;
  const wide = Math.max(0, Math.min(width, at.x + width * k) - Math.max(0, at.x));
  const high = Math.max(0, Math.min(height, at.y + height * k) - Math.max(0, at.y));
  return wide * high >= (1 - GLIDE_EXPOSED) * width * height ? at : null;
}

/** The pictures a moving frame can glide over (`BulkSlot`). */
export interface Kept {
  readonly glide: Glide;
  readonly still: Still | null | undefined;
}

/** Blits the kept picture under this frame's camera; false when there is none close enough. */
export function glideFrame({ glide, still }: Kept, view: readonly unknown[], input: PaintInput, counts: PaintCounts): boolean {
  if (input.focus >= 0) return false;
  if (glide.view.length === 0 && still) adoptStill(glide, still);
  if (glide.view.length === 0 || !sameRefs(glide.view.slice(3), view.slice(3))) return false;
  const at = landing(glide.view, input.camera, input.viewport);
  if (at === null) return false;
  const { width, height } = input.viewport;
  for (const picture of glide.pictures) input.ctx.drawImage(picture, at.x, at.y, width * at.scale, height * at.scale);
  counted(counts, glide.edges, glide.nodes, input.dpr);
  return true;
}
