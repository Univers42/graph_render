/**
 * Where a 2D frame hands its bulk to the GPU layer. The view keeps one slot; the layer is
 * made the first time a frame wants it and, once it fails, never tried again: the 2D
 * painter draws every frame after that, and `failure` says why.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { impostorOf } from "../canvas2d/nodes.ts";
import { MOVING_BUDGET } from "../canvas2d/edges.ts";
import { drawBulk } from "./draw.ts";
import { type BulkLayer, createBulk } from "./layer.ts";
import { type BackendChoice, bulkWanted, nextBudget } from "./plan.ts";

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
}

export function newBulkSlot(backend: BackendChoice): BulkSlot {
  return { backend, layer: undefined, failure: "", placed: 0, budget: MOVING_BUDGET };
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

/**
 * True when the GPU layer drew this frame's edges and nodes onto `input.ctx`. Under `auto`,
 * a scene small enough for the impostor spheres keeps them and stays on the 2D painter.
 */
export function paintBulk(slot: BulkSlot, input: PaintInput, counts: PaintCounts): boolean {
  if (slot.backend === "auto" && impostorOf(input)) return false;
  const elements = input.frame.nodeCount + input.frame.edgeCount;
  if (!bulkWanted(slot.backend, elements, slot.layer !== null)) return false;
  const layer = layerOf(slot);
  if (layer === null) return false;
  const started = performance.now();
  const picture = drawBulk(layer, input, slot, counts);
  if (picture === null) {
    slot.layer = null;
    slot.failure = "the WebGL2 context was lost";
    return false;
  }
  input.ctx.drawImage(picture, 0, 0, input.viewport.width, input.viewport.height);
  picture.close();
  const total = Math.max(layer.uploaded.indexCount / 2, layer.uploaded.shown);
  if (input.moving) slot.budget = nextBudget(slot.budget, performance.now() - started, total);
  return true;
}
