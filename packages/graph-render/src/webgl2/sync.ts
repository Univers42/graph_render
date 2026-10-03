/**
 * The uploads of the GPU layer: a column that fits the buffer it is going into is written into
 * it, and only a column that outgrows its buffer reallocates.
 *
 * WHY: `bufferData` on a buffer that already holds the right size makes the driver throw the
 * old store away and hand back a new one. At a million nodes the moving `x` and `y` columns
 * are the same size every frame, so every frame was paying for an allocation to store the same
 * number of bytes. `bufferSubData` writes into the store that is already there.
 *
 * Ponytail: one `Map` per layer records the byte length each buffer was last given. Failing
 * input: a column that shrinks is still written with `bufferSubData` when it fits, which
 * leaves the tail of the buffer holding the previous frame's numbers — harmless here because
 * every draw reads the columns through `uploaded.shown`, a count this same function sets, and
 * never the whole buffer. Direction: `bufferData` on growth, `bufferSubData` otherwise.
 * Escape hatch: a store the driver discards behind our back shows up as a wrong drawing, not
 * as an error; `gl.getError()` after a sub-data is what would catch it, and nothing calls it.
 */
import type { PaintInput } from "../canvas2d/input.ts";
import { paletteTexels } from "./colour.ts";
import { type BulkLayer, PALETTE_WIDTH, type Pace, type Tween } from "./layer.ts";
import { gathered, largestHalf, nodeHalves, spreadPairs, spreadShown, visibleEdges } from "./plan.ts";
import { measurePairs } from "./sample.ts";

/**
 * The byte length each buffer was last given, so a same-size column need not reallocate.
 *
 * Generic in the buffer's identity rather than taking a `WebGLBuffer`: the policy is the part
 * worth testing, and a test that has to build a WebGL2 context to reach it is a test that does
 * not run. Nothing here knows what a buffer is.
 */
export class Sizes<K> {
  private readonly seen = new Map<K, number>();

  /** True while the buffer has never been given this much room. */
  needsAlloc(buffer: K, bytes: number): boolean {
    return (this.seen.get(buffer) ?? -1) < bytes;
  }

  /** The buffer now holds `bytes`, so a later column of that size is written into it. */
  record(buffer: K, bytes: number): void {
    this.seen.set(buffer, bytes);
  }
}

const sized = new WeakMap<BulkLayer, Sizes<WebGLBuffer>>();

function subData(layer: BulkLayer, target: GLenum, buffer: WebGLBuffer, data: ArrayBufferView): void {
  const { gl } = layer;
  gl.bindBuffer(target, buffer);
  let sizes = sized.get(layer);
  if (sizes === undefined) {
    sizes = new Sizes();
    sized.set(layer, sizes);
  }
  if (sizes.needsAlloc(buffer, data.byteLength)) {
    gl.bufferData(target, data, gl.DYNAMIC_DRAW);
    sizes.record(buffer, data.byteLength);
    return;
  }
  gl.bufferSubData(target, 0, data);
}

export const sameRefs = (a: readonly unknown[], b: readonly unknown[]): boolean => a.length === b.length && a.every((value, at) => value === b[at]);

function syncHalves(layer: BulkLayer, input: PaintInput): void {
  const { gl, buffers, uploaded } = layer;
  const boxes = input.frame.nodeKind === "Box";
  const w = boxes ? input.frame.w : null;
  const h = boxes ? input.frame.h : null;
  const key = [input.extent, w, h, input.style.hidden];
  if (sameRefs(uploaded.halvesKey, key)) return;
  const halves = nodeHalves({ extent: input.extent, w, h, hidden: input.style.hidden });
  const order = spreadShown(halves);
  subData(layer, gl.ARRAY_BUFFER, buffers.half, halves);
  gl.bindVertexArray(layer.points.vao);
  subData(layer, gl.ELEMENT_ARRAY_BUFFER, buffers.order, order);
  gl.bindVertexArray(null);
  uploaded.halvesKey = key;
  uploaded.halves = halves;
  uploaded.shown = order.length;
  uploaded.largest = largestHalf(halves);
}

/**
 * The node columns, and the one place a tween is uploaded. Outside a tween this is the gate it
 * always was: the columns go up when their identity or the view's `placed` counter moved. Inside
 * one, all four columns go up once and `eased` moves the nodes from then on — the counters are
 * still recorded, so the frame that ends the tween uploads nothing either.
 *
 * Ponytail: `placed` is recorded every tween frame but nothing reads it until the tween ends, so
 * a tween that a camera interrupts mid-flight still leaves the buffers holding the columns they
 * were last mixed from. Failing input: a `from` column shorter than the node count reads past its
 * end as the shader's generic attribute, which `blend` already treats as 0. Direction: four
 * columns on the first tween frame, none after. Escape hatch: set `u_eased` to 1 by hand and the
 * layer draws the `to` columns.
 */
/** What the position buffers last took, so `positionsDue` can decide whether they need them again. */
export interface PositionKey {
  /** The column `buffers.x` holds, or null before the first frame. */
  readonly x: Float32Array | null;
  readonly placed: number;
  /** The `from` half of the tween the four position buffers hold, or null outside one. */
  readonly fromX: Float32Array | null;
  /** The `to` half, which is `x` itself while a tween is in flight. */
  readonly toX: Float32Array | null;
}

/**
 * Whether this frame's node columns have to reach the driver. Outside a tween that is the gate
 * there always was — a new column, or the view's in-place `placed` counter having moved. Inside
 * one it is the columns themselves: all four go up on the tween's first frame and then only
 * `u_eased` moves the nodes, which is the whole of what a layout switch costs on the GPU
 * (docs/measurements/perf-transition.md).
 *
 * WHY a function and not a line inside `syncNodes`: node has no WebGL2 context, and a missing
 * upload is invisible to every test in the package — the layer would still draw a moving tween,
 * just from the columns it happened to hold. This is the decision, on its own, so the test can
 * say "a tween uploads once" instead of "still looks like it moves".
 */
export function positionsDue(last: PositionKey, x: Float32Array, pace: Pace): boolean {
  const tween = pace.tween;
  if (tween === null) return last.x !== x || last.placed !== pace.placed;
  return last.fromX !== tween.fromX || last.toX !== tween.toX;
}

/** The four position columns a tween frame uploads: `to` into `x`/`y`, `from` into their own. */
function uploadTween(layer: BulkLayer, tween: Tween): void {
  const { gl, buffers } = layer;
  subData(layer, gl.ARRAY_BUFFER, buffers.x, tween.toX);
  subData(layer, gl.ARRAY_BUFFER, buffers.y, tween.toY);
  subData(layer, gl.ARRAY_BUFFER, buffers.fromX, tween.fromX);
  subData(layer, gl.ARRAY_BUFFER, buffers.fromY, tween.fromY);
}

/** The two position columns of any other frame. */
function uploadPlain(layer: BulkLayer, input: PaintInput): void {
  const { gl, buffers } = layer;
  subData(layer, gl.ARRAY_BUFFER, buffers.x, input.x);
  subData(layer, gl.ARRAY_BUFFER, buffers.y, input.y);
}

export function syncNodes(layer: BulkLayer, input: PaintInput, pace: Pace): void {
  const { gl, buffers, uploaded } = layer;
  const tween = pace.tween;
  if (positionsDue(uploaded, input.x, pace)) {
    if (tween === null) uploadPlain(layer, input);
    else uploadTween(layer, tween);
    uploaded.x = tween === null ? input.x : tween.toX;
    uploaded.fromX = tween?.fromX ?? null;
    uploaded.toX = tween?.toX ?? null;
  }
  // Recorded every tween frame, not only on an upload: the frame that ends the tween hands the
  // layer the `to` columns themselves, so it must find them already up and the counter matched.
  uploaded.placed = pace.placed;
  syncHalves(layer, input);
  if (uploaded.slots !== input.style.colours) {
    subData(layer, gl.ARRAY_BUFFER, buffers.slot, input.style.colours);
    uploaded.slots = input.style.colours;
  }
}

export function syncPalette(layer: BulkLayer, palette: readonly string[]): void {
  const { gl, uploaded } = layer;
  if (uploaded.palette === palette) return;
  const size = Math.max(1, palette.length);
  const width = Math.min(PALETTE_WIDTH, size);
  const height = Math.ceil(size / width);
  const texels = new Uint8Array(width * height * 4);
  texels.set(paletteTexels(palette, layer.normalise));
  gl.bindTexture(gl.TEXTURE_2D, layer.palette);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, texels);
  uploaded.palette = palette;
  uploaded.paletteSize = size;
}

/**
 * The edge pairs in spread order, so that a moving frame's prefix samples the whole graph,
 * and what the sample step is measured from (sample.ts): re-measured whenever the pairs or
 * the positions behind them change identity, or the view's `placed` counter moves.
 *
 * Ponytail: a tween leaves the measure at whatever the columns said when it started, because the
 * chords it holds are the ones the tween is walking between and the step is a thinning estimate.
 * Failing input: a tween between two layouts whose chord lengths differ by more than the step's
 * ceiling reads a moving frame drawn slightly too thick or too thin; the settled frame after it
 * measures again and is exact. Direction: one measure per tween. Escape hatch: the measure is
 * `uploaded.shape`, readable from the console.
 */
export function syncEdges(layer: BulkLayer, input: PaintInput, pace: Pace): void {
  const { gl, uploaded } = layer;
  const key = [input.frame.source, input.frame.target, input.style.hidden];
  if (!sameRefs(uploaded.edges, key)) {
    const index = spreadPairs(visibleEdges(input.frame.source, input.frame.target, input.style.hidden));
    gl.bindVertexArray(layer.edges.vao);
    subData(layer, gl.ELEMENT_ARRAY_BUFFER, layer.buffers.index, index);
    gl.bindVertexArray(null);
    uploaded.edges = key;
    uploaded.index = index;
    uploaded.indexCount = index.length;
  }
  const placed = pace.tween === null ? pace.placed : 0;
  const shape = [...key, input.x, input.y, placed];
  if (!sameRefs(uploaded.shapeKey, shape)) {
    uploaded.shape = measurePairs(input.x, input.y, uploaded.index);
    uploaded.shapeKey = shape;
  }
}

/**
 * The quad pass's columns, holding only `nodes`. The `from` columns are gathered only while a
 * tween is in flight: at `u_eased` of 1 the shaders return before they read `a_fx`/`a_fy`, so
 * outside one the quad buffers' contents are never fetched.
 */
export function syncQuads(layer: BulkLayer, input: PaintInput, nodes: Uint32Array, tween: Tween | null): void {
  const { gl, buffers } = layer;
  subData(layer, gl.ARRAY_BUFFER, buffers.quadX, gathered(input.x, nodes, 1));
  subData(layer, gl.ARRAY_BUFFER, buffers.quadY, gathered(input.y, nodes, 1));
  if (tween !== null) {
    subData(layer, gl.ARRAY_BUFFER, buffers.quadFromX, gathered(tween.fromX, nodes, 1));
    subData(layer, gl.ARRAY_BUFFER, buffers.quadFromY, gathered(tween.fromY, nodes, 1));
  }
  subData(layer, gl.ARRAY_BUFFER, buffers.quadHalf, gathered(layer.uploaded.halves, nodes, 2));
  subData(layer, gl.ARRAY_BUFFER, buffers.quadSlot, gathered(input.style.colours, nodes, 1));
}
