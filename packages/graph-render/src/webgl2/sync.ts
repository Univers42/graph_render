/**
 * The uploads of the GPU layer. Each column is sent again only when the array it came from
 * changed identity (positions also when the view's `placed` counter moved), so a camera move
 * uploads nothing but the quad pass's compact columns. Edges and shown nodes are also listed
 * in spread order (plan.ts), whose every prefix samples the whole graph.
 */
import type { PaintInput } from "../canvas2d/input.ts";
import { paletteTexels } from "./colour.ts";
import { type BulkLayer, PALETTE_WIDTH } from "./layer.ts";
import { gathered, largestHalf, nodeHalves, spreadPairs, spreadShown, visibleEdges } from "./plan.ts";
import { measurePairs } from "./sample.ts";

function upload(gl: WebGL2RenderingContext, target: GLenum, buffer: WebGLBuffer, data: ArrayBufferView): void {
  gl.bindBuffer(target, buffer);
  gl.bufferData(target, data, gl.DYNAMIC_DRAW);
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
  upload(gl, gl.ARRAY_BUFFER, buffers.half, halves);
  gl.bindVertexArray(layer.points.vao);
  upload(gl, gl.ELEMENT_ARRAY_BUFFER, buffers.order, order);
  gl.bindVertexArray(null);
  uploaded.halvesKey = key;
  uploaded.halves = halves;
  uploaded.shown = order.length;
  uploaded.largest = largestHalf(halves);
}

export function syncNodes(layer: BulkLayer, input: PaintInput, placed: number): void {
  const { gl, buffers, uploaded } = layer;
  if (uploaded.x !== input.x || uploaded.placed !== placed) {
    upload(gl, gl.ARRAY_BUFFER, buffers.x, input.x);
    upload(gl, gl.ARRAY_BUFFER, buffers.y, input.y);
    uploaded.x = input.x;
    uploaded.placed = placed;
  }
  syncHalves(layer, input);
  if (uploaded.slots !== input.style.colours) {
    upload(gl, gl.ARRAY_BUFFER, buffers.slot, input.style.colours);
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
 */
export function syncEdges(layer: BulkLayer, input: PaintInput, placed: number): void {
  const { gl, uploaded } = layer;
  const key = [input.frame.source, input.frame.target, input.style.hidden];
  if (!sameRefs(uploaded.edges, key)) {
    const index = spreadPairs(visibleEdges(input.frame.source, input.frame.target, input.style.hidden));
    gl.bindVertexArray(layer.edges.vao);
    upload(gl, gl.ELEMENT_ARRAY_BUFFER, layer.buffers.index, index);
    gl.bindVertexArray(null);
    uploaded.edges = key;
    uploaded.index = index;
    uploaded.indexCount = index.length;
  }
  const shape = [...key, input.x, input.y, placed];
  if (!sameRefs(uploaded.shapeKey, shape)) {
    uploaded.shape = measurePairs(input.x, input.y, uploaded.index);
    uploaded.shapeKey = shape;
  }
}

/** The quad pass's columns, holding only `nodes`. */
export function syncQuads(layer: BulkLayer, input: PaintInput, nodes: Uint32Array): void {
  const { gl, buffers } = layer;
  upload(gl, gl.ARRAY_BUFFER, buffers.quadX, gathered(input.x, nodes, 1));
  upload(gl, gl.ARRAY_BUFFER, buffers.quadY, gathered(input.y, nodes, 1));
  upload(gl, gl.ARRAY_BUFFER, buffers.quadHalf, gathered(layer.uploaded.halves, nodes, 2));
  upload(gl, gl.ARRAY_BUFFER, buffers.quadSlot, gathered(input.style.colours, nodes, 1));
}
