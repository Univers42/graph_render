/**
 * Where a 2D frame hands its bulk to the GPU layer. The view keeps one slot; the layer is
 * made the first time a frame wants it and, once it fails, never tried again: the 2D
 * painter draws every frame after that, and `failure` says why. A moving frame draws a
 * paced sample; a settled one grows and blits the kept picture (still.ts).
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { impostorOf } from "../canvas2d/nodes.ts";
import { MOVING_BUDGET } from "../canvas2d/edges.ts";
import { drawBulk } from "./draw.ts";
import { type BulkLayer, createBulk } from "./layer.ts";
import { type BackendChoice, bulkWanted, nextBudget } from "./plan.ts";
import { type Still, newStill, paintStill } from "./still.ts";

export interface BulkSlot {
  backend: BackendChoice;
  /** Undefined until a frame first wants the layer, null once it could not be had. */
  layer: BulkLayer | null | undefined;
  /** Why the layer is null, or "". */
  failure: string;
  /** Bumped whenever the view writes node positions in place. */
  placed: number;
  /** Edges, and nodes, a moving frame draws, paced by what the last moving frame cost (`nextBudget`). */
  budget: number;
  /** Undefined until a settled frame first wants it, null where it cannot be kept. */
  still: Still | null | undefined;
  /** True while the last frame's still picture lacked edges: the loop asks for another frame. */
  refining: boolean;
}

export function newBulkSlot(backend: BackendChoice): BulkSlot {
  return { backend, layer: undefined, failure: "", placed: 0, budget: MOVING_BUDGET, still: undefined, refining: false };
}

function layerOf(slot: BulkSlot): BulkLayer | null {
  if (slot.layer !== undefined) return slot.layer;
  try {
    slot.layer = createBulk();
    if (slot.layer === null) slot.failure = "this browser gives no WebGL2 context on an OffscreenCanvas";
  } catch (error) {
    slot.layer = null;
    slot.failure = error instanceof Error ? error.message : String(error);
  }
  return slot.layer;
}

/** The whole frame drawn at once, or the budget's sample while moving; false when the context is lost. */
function paintWhole(slot: BulkSlot, layer: BulkLayer, input: PaintInput, counts: PaintCounts): boolean {
  const started = performance.now();
  const picture = drawBulk(layer, input, slot, counts);
  if (picture === null) return false;
  input.ctx.drawImage(picture, 0, 0, input.viewport.width, input.viewport.height);
  picture.close();
  const total = Math.max(layer.uploaded.indexCount / 2, layer.uploaded.shown);
  if (input.moving) slot.budget = nextBudget(slot.budget, performance.now() - started, total);
  return true;
}

function paintSettled(slot: BulkSlot, layer: BulkLayer, input: PaintInput, counts: PaintCounts): boolean {
  slot.still ??= newStill(slot.budget);
  if (slot.still === null) return paintWhole(slot, layer, input, counts);
  const lacking = paintStill(slot.still, { layer, placed: slot.placed }, input, counts);
  slot.refining = lacking > 0;
  return lacking >= 0;
}

/**
 * True when the GPU layer drew this frame's edges and nodes onto `input.ctx`. Under `auto`,
 * a scene small enough for the impostor spheres keeps them and stays on the 2D painter.
 */
export function paintBulk(slot: BulkSlot, input: PaintInput, counts: PaintCounts): boolean {
  slot.refining = false;
  if (slot.backend === "auto" && impostorOf(input)) return false;
  const elements = input.frame.nodeCount + input.frame.edgeCount;
  if (!bulkWanted(slot.backend, elements, slot.layer !== null)) return false;
  const layer = layerOf(slot);
  if (layer === null) return false;
  if (input.moving ? paintWhole(slot, layer, input, counts) : paintSettled(slot, layer, input, counts)) return true;
  slot.layer = null;
  slot.failure = "the WebGL2 context was lost";
  slot.refining = false;
  return false;
}
