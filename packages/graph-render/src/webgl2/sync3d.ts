/**
 * The uploads of the WebGL2 3D layer. Each is sent again only when the arrays it came from
 * changed identity (positions also when the view's `placed` counter moved), so an orbit drag
 * over a settled frame uploads nothing: the camera is a uniform (draw3d.ts).
 *
 * The palette upload is sync.ts's `syncPalette` over this layer's own texture; that one takes
 * the 2D layer whole, and sync.ts is outside this path's files.
 */
import type { PaintInput } from "../canvas2d/input.ts";
import type { Frame } from "../frame.ts";
import { paletteTexels } from "./colour.ts";
import { PALETTE_WIDTH } from "./layer.ts";
import type { SpaceLayer } from "./layer3d.ts";
import { nodeHalves, visibleEdges } from "./plan.ts";

function sameRefs(a: readonly unknown[], b: readonly unknown[]): boolean {
  return a.length === b.length && a.every((value, at) => value === b[at]);
}

function upload(gl: WebGL2RenderingContext, buffer: WebGLBuffer, data: ArrayBufferView): void {
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
}

/** x, y and z of every node as one texel each, into the layer's reused scratch. */
function interleaved(layer: SpaceLayer, input: PaintInput, z: Float32Array | null): Float32Array {
  const count = Math.min(input.x.length, input.y.length);
  const size = layer.positionsWidth * Math.max(1, Math.ceil(count / layer.positionsWidth)) * 4;
  if (layer.scratch.length !== size) layer.scratch = new Float32Array(size);
  const out = layer.scratch;
  for (let node = 0; node < count; node += 1) {
    out[4 * node] = input.x[node] ?? 0;
    out[4 * node + 1] = input.y[node] ?? 0;
    out[4 * node + 2] = z?.[node] ?? 0;
  }
  return out;
}

function syncPositions(layer: SpaceLayer, input: PaintInput, frame: Frame, placed: number): void {
  const { gl, uploaded } = layer;
  const key = [input.x, input.y, frame.z];
  if (uploaded.placed === placed && sameRefs(uploaded.positions, key)) return;
  const texels = interleaved(layer, input, frame.z);
  gl.bindTexture(gl.TEXTURE_2D, layer.positions);
  const rows = texels.length / 4 / layer.positionsWidth;
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, layer.positionsWidth, rows, 0, gl.RGBA, gl.FLOAT, texels);
  uploaded.positions = key;
  uploaded.placed = placed;
  uploaded.nodeCount = Math.min(input.x.length, input.y.length);
}

/** The world half sizes, -1 for a hidden node: the 2D layer's own column (plan.ts `nodeHalves`). */
function syncHalves(layer: SpaceLayer, input: PaintInput): void {
  const boxes = input.frame.nodeKind === "Box";
  const w = boxes ? input.frame.w : null;
  const h = boxes ? input.frame.h : null;
  const key = [input.extent, w, h, input.style.hidden];
  if (sameRefs(layer.uploaded.halves, key)) return;
  upload(layer.gl, layer.buffers.half, nodeHalves({ extent: input.extent, w, h, hidden: input.style.hidden }));
  layer.uploaded.halves = key;
}

function syncSlots(layer: SpaceLayer, input: PaintInput): void {
  if (layer.uploaded.slots === input.style.colours) return;
  upload(layer.gl, layer.buffers.slot, input.style.colours);
  layer.uploaded.slots = input.style.colours;
}

function syncPalette(layer: SpaceLayer, palette: readonly string[]): void {
  const { gl, uploaded } = layer;
  if (uploaded.palette === palette) return;
  const size = Math.max(1, palette.length);
  const width = Math.min(PALETTE_WIDTH, size);
  const texels = new Uint8Array(width * Math.ceil(size / width) * 4);
  texels.set(paletteTexels(palette, layer.normalise));
  gl.bindTexture(gl.TEXTURE_2D, layer.palette);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, Math.ceil(size / width), 0, gl.RGBA, gl.UNSIGNED_BYTE, texels);
  uploaded.palette = palette;
  uploaded.paletteSize = size;
}

/** The shown edges as node index pairs, one instance each. */
function syncEdges(layer: SpaceLayer, input: PaintInput): void {
  const key = [input.frame.source, input.frame.target, input.style.hidden];
  if (sameRefs(layer.uploaded.edges, key)) return;
  const ends = visibleEdges(input.frame.source, input.frame.target, input.style.hidden);
  upload(layer.gl, layer.buffers.ends, ends);
  layer.uploaded.edges = key;
  layer.uploaded.edgeCount = ends.length / 2;
}

/** Everything a frame draws from, re-sent only where it changed. `frame` carries the z column. */
export function syncSpace(layer: SpaceLayer, input: PaintInput, frame: Frame, placed: number): void {
  syncPositions(layer, input, frame, placed);
  syncHalves(layer, input);
  syncSlots(layer, input);
  syncPalette(layer, input.style.palette);
  syncEdges(layer, input);
}
