/**
 * The GPU layer: every edge as a line and every node as a point (or, once a node is too large
 * for a point, an instanced quad over the nodes on screen), drawn on an OffscreenCanvas the 2D
 * context then blits over its ground. A frame re-uploads only the columns whose identity (or,
 * for positions, the view's `placed` counter) changed, so a camera move is two draw calls and
 * one blit whatever the scene's size. This file makes the layer; sync.ts uploads, draw.ts draws.
 *
 * Caveat: lines are one device pixel wide whatever the zoom (WebGL caps lineWidth at 1 on
 * most drivers); routed and curved edges are drawn straight between their ends; arrows, the
 * glow and the impostor spheres are not drawn here. Each edge blends on its own, so where
 * Canvas2D's single path covers a pixel once the GPU covers it once per edge and a dense
 * bundle reads darker. The 2D painter keeps the scene below BULK_THRESHOLD (plan.ts) unless
 * the host asks for `webgl2`.
 */
import { type Normalise, type Rgba, normaliserOf } from "./colour.ts";
import { type Uniforms, attribute, programOf, uniformsOf } from "./gl.ts";
import { EDGE_FRAGMENT, EDGE_VERTEX, NODE_FRAGMENT, NODE_VERTEX, POINT_FRAGMENT, POINT_VERTEX } from "./shaders.ts";

/**
 * The largest node, in device pixels across, drawn as a point; also twice the viewport
 * overhang that keeps a point whose centre left the canvas (GL drops it whole).
 */
const POINT_CEILING = 512;

/** Texels per row of the palette texture. */
export const PALETTE_WIDTH = 2048;

export interface Pass {
  readonly program: WebGLProgram;
  readonly uniforms: Uniforms;
  readonly vao: WebGLVertexArrayObject;
}

/** What the layer last uploaded, keyed by the identity of the arrays it came from. */
export interface Uploaded {
  x: Float32Array | null;
  placed: number;
  halvesKey: readonly unknown[];
  /** `nodeHalves`' output, kept for culling the quad pass. */
  halves: Float32Array;
  /** Shown nodes, which is also the length of the spread node order. */
  shown: number;
  /** The largest half side in world units, which decides between points and quads. */
  largest: number;
  slots: Uint16Array | null;
  palette: readonly string[] | null;
  paletteSize: number;
  edges: readonly unknown[];
  indexCount: number;
}

type Column = "x" | "y" | "half" | "slot" | "index" | "order" | "quadX" | "quadY" | "quadHalf" | "quadSlot";

export interface BulkLayer {
  readonly canvas: OffscreenCanvas;
  readonly gl: WebGL2RenderingContext;
  readonly buffers: Readonly<Record<Column, WebGLBuffer>>;
  /** The node quads, one instance per node on screen, from the `quad*` buffers. */
  readonly quads: Pass;
  readonly points: Pass;
  readonly edges: Pass;
  /** Device pixels across of the largest point this layer draws, at most POINT_CEILING. */
  readonly pointLimit: number;
  /** The smaller of the driver's two viewport limits, which the overhang must fit inside. */
  readonly maxViewport: number;
  readonly palette: WebGLTexture;
  readonly normalise: Normalise;
  readonly colours: Map<string, Rgba>;
  readonly uploaded: Uploaded;
}

function passOf(gl: WebGL2RenderingContext, program: WebGLProgram, wire: (program: WebGLProgram) => void): Pass {
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  wire(program);
  gl.bindVertexArray(null);
  return { program, uniforms: uniformsOf(gl, program), vao };
}

function passesOf(gl: WebGL2RenderingContext, buffers: BulkLayer["buffers"]): Pick<BulkLayer, "quads" | "points" | "edges"> {
  const x = { name: "a_x", buffer: buffers.x, size: 1 };
  const y = { name: "a_y", buffer: buffers.y, size: 1 };
  const slot = { name: "a_colour", buffer: buffers.slot, size: 1, slots: true };
  const half = { name: "a_half", buffer: buffers.half, size: 2 };
  const quads = passOf(gl, programOf(gl, NODE_VERTEX, NODE_FRAGMENT), (program) => {
    const columns = [{ ...x, buffer: buffers.quadX }, { ...y, buffer: buffers.quadY }, { ...slot, buffer: buffers.quadSlot }, { ...half, buffer: buffers.quadHalf }];
    for (const spec of columns) attribute(gl, program, spec, 1);
  });
  const points = passOf(gl, programOf(gl, POINT_VERTEX, POINT_FRAGMENT), (program) => {
    for (const spec of [x, y, slot, half]) attribute(gl, program, spec, 0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.order);
  });
  const edges = passOf(gl, programOf(gl, EDGE_VERTEX, EDGE_FRAGMENT), (program) => {
    for (const spec of [x, y, slot]) attribute(gl, program, spec, 0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.index);
  });
  return { quads, points, edges };
}

function paletteTexture(gl: WebGL2RenderingContext): WebGLTexture {
  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  return texture;
}

function freshUploads(): Uploaded {
  return {
    x: null, placed: -1, halvesKey: [], halves: new Float32Array(0), shown: 0, largest: 0,
    slots: null, palette: null, paletteSize: 1, edges: [], indexCount: 0,
  };
}

/** The point-size and viewport limits of this driver, read once. */
function limitsOf(gl: WebGL2RenderingContext): Pick<BulkLayer, "pointLimit" | "maxViewport"> {
  const sizes: unknown = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE);
  const dims: unknown = gl.getParameter(gl.MAX_VIEWPORT_DIMS);
  const largest = sizes instanceof Float32Array ? (sizes[1] ?? 1) : 1;
  const maxViewport = dims instanceof Int32Array ? Math.min(dims[0] ?? 0, dims[1] ?? 0) : 0;
  return { pointLimit: Math.floor(Math.min(POINT_CEILING, largest)), maxViewport };
}

/**
 * The layer, or null when this browser has no WebGL2 on an OffscreenCanvas. A shader that
 * fails to compile throws with the driver's log: that is a defect, not a missing feature.
 */
export function createBulk(): BulkLayer | null {
  if (typeof OffscreenCanvas === "undefined") return null;
  const canvas = new OffscreenCanvas(1, 1);
  const gl = canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: true, antialias: false, depth: false, stencil: false });
  const probe = new OffscreenCanvas(1, 1).getContext("2d");
  if (gl === null || probe === null) return null;
  const make = (): WebGLBuffer => gl.createBuffer();
  const buffers = { x: make(), y: make(), half: make(), slot: make(), index: make(), order: make(), quadX: make(), quadY: make(), quadHalf: make(), quadSlot: make() };
  return {
    canvas, gl, buffers, ...passesOf(gl, buffers), ...limitsOf(gl), palette: paletteTexture(gl),
    normalise: normaliserOf(probe), colours: new Map(), uploaded: freshUploads(),
  };
}
