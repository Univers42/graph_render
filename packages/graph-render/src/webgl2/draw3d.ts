/**
 * One frame of the WebGL2 3D layer: the edges, then the nodes over them, each one instanced
 * draw, projected on the GPU by the camera matrix (`three/matrix.ts`). The nodes are depth
 * tested, so the nearest is on top whatever the instance order and nothing is sorted; the
 * edges are not, so they sit under every node as the Canvas2D painter draws them.
 *
 * Ponytail: a node's antialiased fringe writes depth too, so where a nearer node is drawn
 * before a farther one that it overlaps, the farther one stops a fraction of a pixel short of
 * the nearer one's edge and the ground shows through that seam. It fails at the rim of
 * overlapping nodes, never inside one; the escape hatch is `?backend=canvas2d`, which paints
 * back to front.
 */
import type { PaintInput } from "../canvas2d/input.ts";
import { MIN_SCREEN_RADIUS } from "../canvas2d/nodes.ts";
import { cameraMatrix } from "../three/matrix.ts";
import type { Orbit } from "../three/orbit.ts";
import type { Frame } from "../frame.ts";
import { bytesOf } from "./colour.ts";
import { deviceSize } from "./draw.ts";
import { PALETTE_WIDTH, type Pass } from "./layer.ts";
import type { SpaceLayer } from "./layer3d.ts";
import { syncSpace } from "./sync3d.ts";

/** What a 3D frame is drawn from beyond the paint input: the camera, and the columns' z. */
export interface SpaceFrame {
  readonly orbit: Orbit;
  readonly frame: Frame;
  /** The view's in-place move counter: positions are re-sent when it moved. */
  readonly placed: number;
  /** The edge stroke, in CSS pixels. */
  readonly width: number;
}

function begin(layer: SpaceLayer, input: PaintInput): void {
  const { gl, canvas } = layer;
  const [width, height] = deviceSize(input);
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  gl.viewport(0, 0, width, height);
  gl.clearColor(0, 0, 0, 0);
  gl.clearDepth(1);
  gl.depthMask(true);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, layer.palette);
  gl.activeTexture(gl.TEXTURE1);
  gl.bindTexture(gl.TEXTURE_2D, layer.positions);
}

/** The uniforms both passes read: the camera, the positions and the viewport. */
function shared(layer: SpaceLayer, pass: Pass, input: PaintInput): void {
  const { gl } = layer;
  const at = pass.uniforms;
  gl.useProgram(pass.program);
  gl.uniformMatrix4fv(at("u_camera"), false, layer.camera);
  gl.uniform1i(at("u_positions"), 1);
  gl.uniform1i(at("u_positionsWidth"), layer.positionsWidth);
  gl.uniform2f(at("u_viewport"), input.viewport.width, input.viewport.height);
  gl.uniform1f(at("u_pixel"), 1 / input.dpr);
  gl.bindVertexArray(pass.vao);
}

function uniformColour(gl: WebGL2RenderingContext, at: WebGLUniformLocation | null, css: string, layer: SpaceLayer): void {
  const [r, g, b, a] = bytesOf(css, layer.normalise);
  gl.uniform4f(at, r / 255, g / 255, b / 255, a / 255);
}

function drawEdges(layer: SpaceLayer, input: PaintInput, width: number): void {
  const { gl, edges } = layer;
  if (layer.uploaded.edgeCount === 0) return;
  gl.disable(gl.DEPTH_TEST);
  gl.depthMask(false);
  shared(layer, edges, input);
  gl.uniform1f(edges.uniforms("u_width"), width);
  uniformColour(gl, edges.uniforms("u_edge"), input.theme.edge, layer);
  gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, layer.uploaded.edgeCount);
}

/** The nodes every column has a value for. */
function nodeCountOf(layer: SpaceLayer, input: PaintInput): number {
  return Math.min(layer.uploaded.nodeCount, input.extent.length, input.style.colours.length);
}

function drawNodes(layer: SpaceLayer, input: PaintInput, count: number): void {
  const { gl, nodes } = layer;
  if (count === 0) return;
  gl.enable(gl.DEPTH_TEST);
  gl.depthFunc(gl.LEQUAL);
  gl.depthMask(true);
  shared(layer, nodes, input);
  const at = nodes.uniforms;
  gl.uniform1i(at("u_palette"), 0);
  gl.uniform1i(at("u_paletteSize"), layer.uploaded.paletteSize);
  gl.uniform1i(at("u_paletteWidth"), Math.min(PALETTE_WIDTH, layer.uploaded.paletteSize));
  gl.uniform1f(at("u_minRadius"), MIN_SCREEN_RADIUS);
  gl.uniform1f(at("u_pad"), 0.5 + 1 / input.dpr);
  gl.uniform1f(at("u_alpha"), 1);
  gl.uniform1i(at("u_box"), input.frame.nodeKind === "Box" ? 1 : 0);
  uniformColour(gl, at("u_rim"), input.theme.rim, layer);
  gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count);
}

/**
 * Draws one 3D frame and hands the picture back with the nodes it drew, or null when the
 * context is lost (the caller falls back to the Canvas2D painter for good).
 */
export function drawSpace(layer: SpaceLayer, input: PaintInput, space: SpaceFrame): { picture: ImageBitmap; nodes: number } | null {
  if (layer.gl.isContextLost()) return null;
  syncSpace(layer, input, space.frame, space.placed);
  cameraMatrix(space.orbit, input.viewport, layer.camera);
  begin(layer, input);
  drawEdges(layer, input, space.width);
  const nodes = nodeCountOf(layer, input);
  drawNodes(layer, input, nodes);
  return { picture: layer.canvas.transferToImageBitmap(), nodes };
}
