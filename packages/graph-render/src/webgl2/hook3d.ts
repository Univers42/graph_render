/**
 * Where a 3D frame hands itself to the WebGL2 3D layer. The view's one bulk slot carries the
 * backend choice and the failure; the 3D layer is kept beside it here, made the first time a
 * frame wants it and, once it fails or its context is lost, never tried again: the Canvas2D
 * painter (`three/paint3d.ts`) draws every 3D frame after that, and `slot.failure` says why.
 *
 * The frame reads only the projection's `wanted` (the columns and the orbit), never its
 * projected columns, so an orbit drag costs no projection and no sort on the CPU. The rings
 * are the one thing still placed on the CPU, two nodes at most.
 *
 * `?backend=webgl2` takes the layer at any size and on any WebGL2. `auto` takes it only where
 * it was measured faster at p95 (docs/measurements/perf-3d-gl.md): from SPACE_THRESHOLD
 * elements, on a hardware renderer. On a software rasteriser (SwiftShader) this layer orbited
 * 5 to 9 times slower than the Canvas2D painter, so `auto` keeps that painter there.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { MIN_SCREEN_RADIUS } from "../canvas2d/nodes.ts";
import { type Locate, paintRings, strokeWidth } from "../three/paint3d.ts";
import { paintGround } from "../canvas2d/ground.ts";
import { type Projection, setupOf } from "../three/projection.ts";
import { project, radiusOnScreen } from "../three/orbit.ts";
import { counted } from "./draw.ts";
import { drawSpace } from "./draw3d.ts";
import { loseContext } from "./gl.ts";
import type { BulkSlot } from "./hook.ts";
import { type SpaceLayer, createSpace, fits } from "./layer3d.ts";
import type { BackendChoice } from "./plan.ts";

/** The 3D layer per view slot: absent until wanted, null once it could not be had. */
const layers = new WeakMap<BulkSlot, SpaceLayer | null>();

/**
 * The elements (nodes plus edges) from which `auto` hands a 3D frame to this layer.
 *
 * Caveat: measured on one GPU (Radeon RX 6600, 1920x1080, one run per size): the orbit's p95
 * frame gap tied at 16.8 ms at 40 004 elements, and was 16.7 ms against the Canvas2D painter's
 * 33.3, 33.4 and 333.3 at 44 996, 59 996 and 599 996. A slower GPU or a larger canvas moves the
 * crossing, and a crossing set too high there keeps a frame on the slower painter.
 */
export const SPACE_THRESHOLD = 44_000;

/** True when the 3D layer draws a frame of `elements`; `available` is false once it could not be had. */
export function spaceWanted(choice: BackendChoice, elements: number, available: boolean): boolean {
  if (!available || choice === "canvas2d") return false;
  return choice === "webgl2" || elements >= SPACE_THRESHOLD;
}

/** Gives the slot's 3D context back, and never makes another for it. */
export function releaseSpace(slot: BulkSlot): void {
  const layer = layers.get(slot);
  if (layer) loseContext(layer.gl);
  layers.set(slot, null);
}

/** Under `auto` a software rasteriser is a choice, not a failure: it leaves `failure` empty. */
function spaceOf(slot: BulkSlot): SpaceLayer | null {
  const known = layers.get(slot);
  if (known !== undefined) return known;
  let made: SpaceLayer | null = null;
  try {
    made = createSpace(slot.backend === "auto");
    if (made === null && slot.backend !== "auto") slot.failure = "this browser gives no WebGL2 context on an OffscreenCanvas";
  } catch (error) {
    slot.failure = error instanceof Error ? error.message : String(error);
  }
  layers.set(slot, made);
  return made;
}

/** Where a ring goes: the node projected on its own, as the Canvas2D painter has it. */
function locatorOf(wanted: Projection): Locate {
  const setup = setupOf(wanted);
  return (node) => {
    const point = { x: wanted.x[node] ?? 0, y: wanted.y[node] ?? 0, z: wanted.frame.z?.[node] ?? 0 };
    const found = project(setup.basis, point, setup.focal, setup.centre);
    if (found === null) return null;
    const radius = Math.max(MIN_SCREEN_RADIUS, radiusOnScreen(setup.focal, wanted.extent[node] ?? 0, found.depth));
    return { x: found.x, y: found.y, radius };
  };
}

/**
 * The stroke width over the pixels a world unit covers at the orbit's target.
 *
 * Caveat: the Canvas2D painter takes that scale as the mean over every node in front of the
 * eye (`three/paths.ts` `pixelsPerUnit`), which needs the projection this path skips; the
 * target's depth is the orbit's distance, so a drawing much deeper than it is wide strokes a
 * look's `edgeWidth` a little differently here. A lookless frame's stroke reads no scale.
 */
function widthOf(input: PaintInput, wanted: Projection): number {
  return strokeWidth(input, setupOf(wanted).focal / wanted.orbit.distance);
}

function wantedHere(slot: BulkSlot, input: PaintInput): boolean {
  const elements = input.frame.nodeCount + input.frame.edgeCount;
  return spaceWanted(slot.backend, elements, layers.get(slot) !== null);
}

/** True when the 3D layer drew this frame whole onto `input.ctx`: ground, edges, nodes, rings. */
export function paintSpace(slot: BulkSlot, input: PaintInput, counts: PaintCounts): boolean {
  const wanted = input.space?.wanted ?? null;
  if (wanted === null || !wantedHere(slot, input)) return false;
  const layer = spaceOf(slot);
  if (layer === null || !fits(layer, input.frame.nodeCount)) return false;
  const width = widthOf(input, wanted);
  const drawn = drawSpace(layer, input, { orbit: wanted.orbit, frame: wanted.frame, placed: slot.placed, width });
  if (drawn === null) {
    layers.set(slot, null);
    slot.failure = "the WebGL2 context was lost";
    return false;
  }
  paintGround(input);
  input.ctx.drawImage(drawn.picture, 0, 0, input.viewport.width, input.viewport.height);
  drawn.picture.close();
  paintRings(input, locatorOf(wanted), counts);
  counted(counts, layer.uploaded.edgeCount, drawn.nodes, input.dpr);
  counts.stroke = width;
  return true;
}
