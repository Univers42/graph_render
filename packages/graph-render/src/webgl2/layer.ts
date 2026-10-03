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
import { type EdgeTimer, edgeTimerOf } from "./gputimer.ts";
import { type Uniforms, attribute, programOf, uniformsOf } from "./gl.ts";
import type { EdgeShape } from "./sample.ts";
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
  /** What `u_eased` is: 1 whenever nothing is being mixed, so the shaders read `a_x`/`a_y`. */
  eased: number;
  /** The `from` half of the tween the four position buffers hold, or null outside one. */
  fromX: Float32Array | null;
  /** The `to` half, which is `x` itself while a tween is in flight. */
  toX: Float32Array | null;
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
  /** The drawn pairs themselves, so a placement change can re-measure them without a rebuild. */
  index: Uint32Array;
  indexCount: number;
  /** Identity of the arrays `shape` was measured from: the edge key plus positions and `placed`. */
  shapeKey: readonly unknown[];
  /** What the sample step is measured from (sample.ts); null before the first measure. */
  shape: EdgeShape | null;
}

type Column = "x" | "y" | "fromX" | "fromY" | "half" | "slot" | "index" | "order"
  | "quadX" | "quadY" | "quadFromX" | "quadFromY" | "quadHalf" | "quadSlot";

/**
 * A layout switch in flight: the two columns the nodes ease between, and how far along it is.
 * The layer uploads all four once and moves `eased` per frame, so a tween costs no upload and no
 * CPU blend (docs/measurements/perf-transition.md).
 */
export interface Tween {
  readonly fromX: Float32Array;
  readonly fromY: Float32Array;
  readonly toX: Float32Array;
  readonly toY: Float32Array;
  /** `easeInOutCubic` of the tween's clock: 0 at the `from` columns, 1 at the `to` columns. */
  readonly eased: number;
}

/**
 * How the view paces the layer: its in-place move counter, the moving budget, and the tween the
 * shader is mixing. `view.ts` passes its own `BulkSlot` as this.
 */
export interface Pace {
  readonly placed: number;
  readonly budget: number;
  /** The tween to mix, or null: at null the shaders read `a_x`/`a_y` unchanged. */
  readonly tween: Tween | null;
}

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
  /** The edge draw's GPU time on this context (gputimer.ts), silent where the browser has no query. */
  readonly timer: EdgeTimer;
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
    const from = { name: "a_fx", buffer: buffers.quadFromX, size: 1 };
    const columns = [{ ...x, buffer: buffers.quadX }, { ...y, buffer: buffers.quadY }, { ...from, buffer: buffers.quadFromY },
      { ...slot, buffer: buffers.quadSlot }, { ...half, buffer: buffers.quadHalf }];
    for (const spec of columns) attribute(gl, program, spec, 1);
  });
  const points = passOf(gl, programOf(gl, POINT_VERTEX, POINT_FRAGMENT), (program) => {
    const from = [{ name: "a_fx", buffer: buffers.fromX, size: 1 }, { name: "a_fy", buffer: buffers.fromY, size: 1 }];
    for (const spec of [x, y, ...from, slot, half]) attribute(gl, program, spec, 0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.order);
  });
  const edges = passOf(gl, programOf(gl, EDGE_VERTEX, EDGE_FRAGMENT), (program) => {
    const from = [{ name: "a_fx", buffer: buffers.fromX, size: 1 }, { name: "a_fy", buffer: buffers.fromY, size: 1 }];
    for (const spec of [x, y, ...from, slot]) attribute(gl, program, spec, 0);
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
    x: null, placed: -1, eased: 1, fromX: null, toX: null,
    halvesKey: [], halves: new Float32Array(0), shown: 0, largest: 0,
    slots: null, palette: null, paletteSize: 1, edges: [], index: new Uint32Array(0), indexCount: 0,
    shapeKey: [], shape: null,
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
  const buffers = {
    x: make(), y: make(), fromX: make(), fromY: make(), half: make(), slot: make(), index: make(), order: make(),
    quadX: make(), quadY: make(), quadFromX: make(), quadFromY: make(), quadHalf: make(), quadSlot: make(),
  };
  return {
    canvas, gl, buffers, ...passesOf(gl, buffers), ...limitsOf(gl), palette: paletteTexture(gl),
    normalise: normaliserOf(probe), colours: new Map(), uploaded: freshUploads(), timer: edgeTimerOf(gl),
  };
}
