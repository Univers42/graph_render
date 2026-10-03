/**
 * The WebGL2 3D layer: its own OffscreenCanvas with a depth buffer (the 2D layer's has none,
 * layer.ts), two programs, and what it last uploaded. sync3d.ts uploads, draw3d.ts draws.
 *
 * The positions are one RGBA32F texture, a node per texel, rows of `positionsWidth`: the nodes
 * read their own texel by instance and the edges read both ends', so one upload feeds both
 * passes.
 *
 * Caveat: the texture holds `positionsWidth` × MAX_TEXTURE_SIZE nodes (16M at the usual 4096);
 * a larger frame is refused by `fits` and the Canvas2D painter draws it.
 */
import { type Normalise, normaliserOf } from "./colour.ts";
import { type Uniforms, attribute, programOf, uniformsOf } from "./gl.ts";
import type { Pass } from "./layer.ts";
import { NODE_FRAGMENT } from "./shaders.ts";
import { SPACE_EDGE_FRAGMENT, SPACE_EDGE_VERTEX, SPACE_NODE_VERTEX } from "./shaders3d.ts";

/** What the layer last uploaded, keyed by the identity of the arrays it came from. */
export interface SpaceUploads {
  positions: readonly unknown[];
  placed: number;
  nodeCount: number;
  halves: readonly unknown[];
  slots: Uint16Array | null;
  palette: readonly string[] | null;
  paletteSize: number;
  edges: readonly unknown[];
  edgeCount: number;
}

export interface SpaceLayer {
  readonly canvas: OffscreenCanvas;
  readonly gl: WebGL2RenderingContext;
  readonly nodes: Pass;
  readonly edges: Pass;
  readonly buffers: Readonly<Record<"half" | "slot" | "ends", WebGLBuffer>>;
  readonly positions: WebGLTexture;
  readonly palette: WebGLTexture;
  /** Texels per row of the positions texture. */
  readonly positionsWidth: number;
  /** The most rows the driver allows. */
  readonly maxRows: number;
  readonly normalise: Normalise;
  /** The camera matrix, written in place every frame. */
  readonly camera: Float32Array<ArrayBuffer>;
  /** The interleaved positions, reused while the node count holds. */
  scratch: Float32Array<ArrayBuffer>;
  readonly uploaded: SpaceUploads;
}

function passOf(gl: WebGL2RenderingContext, program: WebGLProgram, wire: (program: WebGLProgram) => void): Pass {
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  wire(program);
  gl.bindVertexArray(null);
  const uniforms: Uniforms = uniformsOf(gl, program);
  return { program, uniforms, vao };
}

/** The edge ends: two node indices per instance, read as `uvec2`. */
function wireEnds(gl: WebGL2RenderingContext, program: WebGLProgram, buffer: WebGLBuffer): void {
  const at = gl.getAttribLocation(program, "a_ends");
  if (at < 0) return;
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.enableVertexAttribArray(at);
  gl.vertexAttribIPointer(at, 2, gl.UNSIGNED_INT, 0, 0);
  gl.vertexAttribDivisor(at, 1);
}

function passesOf(gl: WebGL2RenderingContext, buffers: SpaceLayer["buffers"]): Pick<SpaceLayer, "nodes" | "edges"> {
  const nodes = passOf(gl, programOf(gl, SPACE_NODE_VERTEX, NODE_FRAGMENT), (program) => {
    attribute(gl, program, { name: "a_half", buffer: buffers.half, size: 2 }, 1);
    attribute(gl, program, { name: "a_colour", buffer: buffers.slot, size: 1, slots: true }, 1);
  });
  const edges = passOf(gl, programOf(gl, SPACE_EDGE_VERTEX, SPACE_EDGE_FRAGMENT), (program) => {
    wireEnds(gl, program, buffers.ends);
  });
  return { nodes, edges };
}

function textureOf(gl: WebGL2RenderingContext): WebGLTexture {
  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  return texture;
}

function freshUploads(): SpaceUploads {
  return {
    positions: [], placed: -1, nodeCount: 0, halves: [], slots: null, palette: null, paletteSize: 1, edges: [], edgeCount: 0,
  };
}

function maxTexture(gl: WebGL2RenderingContext): number {
  const size: unknown = gl.getParameter(gl.MAX_TEXTURE_SIZE);
  return typeof size === "number" ? size : 2048;
}

/** Renderer names that mean a CPU rasteriser: the perf probes' list (deploy/nav/gpu.py). */
const SOFTWARE_RENDERERS = ["swiftshader", "llvmpipe"];

/**
 * True when a renderer of this name draws on the CPU.
 *
 * Caveat: it reads a name, so a software rasteriser under another name, or a browser that masks
 * the name, reads as hardware and `auto` hands it the 3D layer; `?backend=canvas2d` is the
 * escape hatch. A hardware renderer is never read as software.
 */
export function softwareNamed(renderer: string): boolean {
  const name = renderer.toLowerCase();
  return SOFTWARE_RENDERERS.some((word) => name.includes(word));
}

function rendererOf(gl: WebGL2RenderingContext): string {
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const name: unknown = gl.getParameter(info === null ? gl.RENDERER : info.UNMASKED_RENDERER_WEBGL);
  return typeof name === "string" ? name : "";
}

/** True when `nodes` fit the positions texture. */
export function fits(layer: SpaceLayer, nodes: number): boolean {
  return Math.ceil(nodes / layer.positionsWidth) <= layer.maxRows;
}

/**
 * The layer, or null when this browser has no WebGL2 on an OffscreenCanvas, or, when
 * `hardwareOnly`, when its WebGL2 is a software rasteriser (the context is then given back).
 * A shader that fails to compile throws with the driver's log: a defect, not a missing feature.
 */
export function createSpace(hardwareOnly: boolean): SpaceLayer | null {
  if (typeof OffscreenCanvas === "undefined") return null;
  const canvas = new OffscreenCanvas(1, 1);
  const gl = canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: true, antialias: false, depth: true, stencil: false });
  const probe = new OffscreenCanvas(1, 1).getContext("2d");
  if (gl === null || probe === null) return null;
  if (hardwareOnly && softwareNamed(rendererOf(gl))) {
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return null;
  }
  const buffers = { half: gl.createBuffer(), slot: gl.createBuffer(), ends: gl.createBuffer() };
  const largest = maxTexture(gl);
  return {
    canvas, gl, buffers, ...passesOf(gl, buffers), positions: textureOf(gl), palette: textureOf(gl),
    positionsWidth: Math.min(4096, largest), maxRows: largest, normalise: normaliserOf(probe),
    camera: new Float32Array(16), scratch: new Float32Array(0), uploaded: freshUploads(),
  };
}
