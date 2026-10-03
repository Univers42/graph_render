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
import { type BulkLayer, PALETTE_WIDTH } from "./layer.ts";
import { gathered, largestHalf, nodeHalves, spreadPairs, spreadShown, visibleEdges } from "./plan.ts";

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

export function syncNodes(layer: BulkLayer, input: PaintInput, placed: number): void {
  const { gl, buffers, uploaded } = layer;
  if (uploaded.x !== input.x || uploaded.placed !== placed) {
    subData(layer, gl.ARRAY_BUFFER, buffers.x, input.x);
    subData(layer, gl.ARRAY_BUFFER, buffers.y, input.y);
    uploaded.x = input.x;
    uploaded.placed = placed;
  }
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

/** The edge pairs in spread order, so that a moving frame's prefix samples the whole graph. */
export function syncEdges(layer: BulkLayer, input: PaintInput): void {
  const { gl, uploaded } = layer;
  const key = [input.frame.source, input.frame.target, input.style.hidden];
  if (sameRefs(uploaded.edges, key)) return;
  const index = spreadPairs(visibleEdges(input.frame.source, input.frame.target, input.style.hidden));
  gl.bindVertexArray(layer.edges.vao);
  subData(layer, gl.ELEMENT_ARRAY_BUFFER, layer.buffers.index, index);
  gl.bindVertexArray(null);
  uploaded.edges = key;
  uploaded.indexCount = index.length;
}

/** The quad pass's columns, holding only `nodes`. */
export function syncQuads(layer: BulkLayer, input: PaintInput, nodes: Uint32Array): void {
  const { gl, buffers } = layer;
  subData(layer, gl.ARRAY_BUFFER, buffers.quadX, gathered(input.x, nodes, 1));
  subData(layer, gl.ARRAY_BUFFER, buffers.quadY, gathered(input.y, nodes, 1));
  subData(layer, gl.ARRAY_BUFFER, buffers.quadHalf, gathered(layer.uploaded.halves, nodes, 2));
  subData(layer, gl.ARRAY_BUFFER, buffers.quadSlot, gathered(input.style.colours, nodes, 1));
}
