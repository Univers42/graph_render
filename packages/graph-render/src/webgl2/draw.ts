/**
 * One frame of the GPU layer: the edges as lines, then the nodes as one point each, or as
 * instanced quads over the nodes on screen once the largest node outgrows a point.
 *
 * Caveat: a moving frame draws a prefix of the spread edge and node orders, `pace.budget` of
 * each, and the settled frames then fill a kept picture with all of them (still.ts); the
 * sampled nodes stack in spread order rather than index order, so where two overlap the
 * other may be on top while the camera moves. Quads are never sampled: they are drawn zoomed in,
 * over the nodes on screen, and in a moving frame whose on-screen nodes fit its budget. Finding
 * them is a pass over the nodes on the CPU, which a moving frame cuts short once more than its
 * budget are on screen. A point is clipped by its centre, so the GL viewport overhangs the
 * canvas and a node larger than the overhang is drawn as a quad instead.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { MIN_SCREEN_RADIUS } from "../canvas2d/nodes.ts";
import { type Rgba, bytesOf } from "./colour.ts";
import { type BulkLayer, PALETTE_WIDTH, type Pass, type Target } from "./layer.ts";
import { onScreen } from "./plan.ts";
import { syncEdges, syncNodes, syncPalette, syncQuads } from "./sync.ts";

/** How the view paces the layer: its in-place move counter and the moving budget. */
export interface Pace {
  readonly placed: number;
  readonly budget: number;
}

interface Frame {
  readonly input: PaintInput;
  /** Device pixels the GL viewport reaches past each side of the canvas. */
  readonly overhang: number;
  /** Nodes every column has a value for. */
  readonly count: number;
  /** What every element's alpha is multiplied by: the dim alpha under a focus, else 1. */
  readonly alpha: number;
}

function rgbaOfCss(layer: BulkLayer, css: string): Rgba {
  const known = layer.colours.get(css);
  if (known !== undefined) return known;
  const bytes = bytesOf(css, layer.normalise);
  layer.colours.set(css, bytes);
  return bytes;
}

function shared(layer: BulkLayer, pass: Pass, frame: Frame): void {
  const { gl } = layer;
  const { input } = frame;
  const at = pass.uniforms;
  gl.useProgram(pass.program);
  gl.uniform3f(at("u_camera"), input.camera.x, input.camera.y, input.camera.scale);
  const margin = frame.overhang / input.dpr;
  gl.uniform2f(at("u_viewport"), input.viewport.width + 2 * margin, input.viewport.height + 2 * margin);
  gl.uniform1f(at("u_margin"), margin);
  gl.uniform1i(at("u_palette"), 0);
  gl.uniform1i(at("u_paletteSize"), layer.uploaded.paletteSize);
  gl.uniform1i(at("u_paletteWidth"), Math.min(PALETTE_WIDTH, layer.uploaded.paletteSize));
  gl.uniform1f(at("u_alpha"), frame.alpha);
  gl.bindVertexArray(pass.vao);
}

/** The edge pairs from `first` in spread order, `count` of them at most; how many were drawn. */
function drawEdges(layer: BulkLayer, frame: Frame, first: number, count: number): number {
  const { gl, edges } = layer;
  const { input } = frame;
  const drawn = Math.max(0, Math.min(layer.uploaded.indexCount / 2 - first, count));
  if (drawn === 0) return 0;
  shared(layer, edges, frame);
  const edge = rgbaOfCss(layer, input.theme.edge);
  gl.uniform1i(edges.uniforms("u_gradient"), input.style.edgeColour === "gradient" ? 1 : 0);
  gl.uniform4f(edges.uniforms("u_edge"), edge[0] / 255, edge[1] / 255, edge[2] / 255, edge[3] / 255);
  gl.drawElements(gl.LINES, drawn * 2, gl.UNSIGNED_INT, first * 2 * Uint32Array.BYTES_PER_ELEMENT);
  return drawn;
}

function nodeUniforms(layer: BulkLayer, pass: Pass, frame: Frame, pad: number): void {
  const { gl } = layer;
  const { input } = frame;
  shared(layer, pass, frame);
  const rim = rgbaOfCss(layer, input.theme.rim);
  gl.uniform1f(pass.uniforms("u_minRadius"), MIN_SCREEN_RADIUS);
  gl.uniform1f(pass.uniforms("u_pad"), pad);
  gl.uniform1f(pass.uniforms("u_pixel"), 1 / input.dpr);
  gl.uniform1f(pass.uniforms("u_dpr"), input.dpr);
  gl.uniform1i(pass.uniforms("u_box"), input.frame.nodeKind === "Box" ? 1 : 0);
  gl.uniform4f(pass.uniforms("u_rim"), rim[0] / 255, rim[1] / 255, rim[2] / 255, rim[3] / 255);
}

function drawQuads(layer: BulkLayer, frame: Frame, nodes: Uint32Array, pad: number): number {
  if (nodes.length === 0) return 0;
  syncQuads(layer, frame.input, nodes);
  nodeUniforms(layer, layer.quads, frame, pad);
  layer.gl.drawArraysInstanced(layer.gl.TRIANGLE_STRIP, 0, 4, nodes.length);
  return nodes.length;
}

function visible(layer: BulkLayer, input: PaintInput, pad: number, limit = Infinity): Uint32Array {
  const { x, y, camera, viewport } = input;
  return onScreen({ x, y, halves: layer.uploaded.halves, camera, viewport, pad: pad + MIN_SCREEN_RADIUS }, limit);
}

/**
 * The nodes drawn: the shown nodes as points, or quads over those on screen. A moving frame
 * with more shown nodes than its budget draws every node on screen when they fit the budget,
 * else a `budget` prefix of the spread order: zoomed in on 1M nodes, the prefix alone kept one
 * of the twenty nodes on screen (target/p5-zoom-moving.png).
 */
function drawNodes(layer: BulkLayer, frame: Frame, budget: number): number {
  const { gl, uploaded } = layer;
  const { input } = frame;
  const pad = 0.5 + 1 / input.dpr;
  const reach = Math.max(MIN_SCREEN_RADIUS, 0.5, uploaded.largest * input.camera.scale) + pad;
  if (reach * input.dpr > frame.overhang) return drawQuads(layer, frame, visible(layer, input, pad), pad);
  if (input.moving && budget < uploaded.shown) {
    const nodes = visible(layer, input, pad, budget);
    if (nodes.length <= budget) return drawQuads(layer, frame, nodes, pad);
  }
  nodeUniforms(layer, layer.points, frame, pad);
  if (!input.moving || budget >= uploaded.shown) gl.drawArrays(gl.POINTS, 0, frame.count);
  else gl.drawElements(gl.POINTS, budget, gl.UNSIGNED_INT, 0);
  return Math.min(budget, uploaded.shown);
}

/** The canvas size in device pixels. */
export function deviceSize(input: Pick<PaintInput, "viewport" | "dpr">): readonly [number, number] {
  return [Math.max(1, Math.round(input.viewport.width * input.dpr)), Math.max(1, Math.round(input.viewport.height * input.dpr))];
}

/**
 * Sizes the canvas and the GL viewport and draws the frame's edges and nodes into `framebuffer`:
 * the canvas itself for a moving or whole frame, where it is cleared first, or one of the settled
 * picture's textures for a chunk, where it is not, so the texture keeps the chunks before it.
 */
function begin(layer: BulkLayer, input: PaintInput, alpha: number, framebuffer: WebGLFramebuffer | null): Frame {
  const { gl, canvas } = layer;
  const [width, height] = deviceSize(input);
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  const room = Math.floor((layer.maxViewport - Math.max(width, height)) / 2);
  const overhang = Math.max(0, Math.min(Math.floor(layer.pointLimit / 2), room));
  gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
  gl.viewport(-overhang, -overhang, width + 2 * overhang, height + 2 * overhang);
  if (framebuffer === null) {
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, layer.palette);
  const count = Math.min(input.x.length, input.y.length, input.extent.length, input.style.colours.length);
  return { input, overhang, count, alpha };
}

function sync(layer: BulkLayer, input: PaintInput, placed: number): void {
  syncNodes(layer, input, placed);
  syncPalette(layer, input.style.palette);
  syncEdges(layer, input);
}

/** What a GPU frame drew, in the counts the 2D painter keeps. */
export function counted(counts: PaintCounts, edges: number, nodes: number, dpr: number): void {
  counts.edges = edges;
  counts.nodes = nodes;
  counts.draws += 2;
  counts.bulk = 2;
  counts.stroke = 1 / dpr;
}

/**
 * Draws one frame's edges and nodes, dimmed under a focus, and hands the picture back, or
 * null when the context is lost (the caller falls back to the 2D painter for good).
 */
export function drawBulk(layer: BulkLayer, input: PaintInput, pace: Pace, counts: PaintCounts): ImageBitmap | null {
  if (layer.gl.isContextLost()) return null;
  sync(layer, input, pace.placed);
  const frame = begin(layer, input, input.focus >= 0 ? input.theme.dimAlpha : 1, null);
  const edges = drawEdges(layer, frame, 0, input.moving ? pace.budget : Infinity);
  const nodes = frame.count > 0 ? drawNodes(layer, frame, input.moving ? pace.budget : Infinity) : 0;
  layer.gl.bindVertexArray(null);
  counted(counts, edges, nodes, input.dpr);
  return layer.canvas.transferToImageBitmap();
}

/** One chunk of the settled picture: the edge pairs from `first`, `count` of them, or every node. */
export interface Chunk {
  readonly input: PaintInput;
  readonly placed: number;
  /** The spread index the chunk's edge pairs start at; ignored where `nodes` is set. */
  readonly first: number;
  readonly count: number;
  /** Every node rather than a range of edge pairs. */
  readonly nodes: boolean;
}

/**
 * What one chunk drew into `target`, undimmed, or -1 when the context is lost. Nothing is read back
 * and nothing is cleared: the caller adds the chunk to a texture of the settled picture and reads
 * the picture back once, to show it (still.ts).
 */
export function drawChunk(layer: BulkLayer, target: Target, chunk: Chunk): number {
  if (layer.gl.isContextLost()) return -1;
  sync(layer, chunk.input, chunk.placed);
  const frame = begin(layer, chunk.input, 1, target.framebuffer);
  const drawn = chunk.nodes
    ? (frame.count > 0 ? drawNodes(layer, frame, Infinity) : 0)
    : drawEdges(layer, frame, chunk.first, chunk.count);
  layer.gl.bindVertexArray(null);
  return drawn;
}
