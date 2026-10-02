/**
 * The settled picture on the GPU: two textures the size of the canvas, one for the edges and one
 * for the nodes, and a framebuffer over each. A settled frame draws a chunk of edges into the edge
 * texture and nothing is copied off the GPU to add it, so the only readback a settled frame pays is
 * the one that shows the picture (still.ts); before, every chunk left the GPU as an ImageBitmap and
 * was composited into a kept 2D picture, two canvas-sized copies a chunk. Showing it is one
 * untextured quad per texture over the canvas, and the dim is that quad's alpha.
 *
 * Caveat: the textures are clamped to the driver's `MAX_TEXTURE_SIZE`, so where that limit is below
 * the canvas the picture is held at a lower resolution and the quad stretches it back over the
 * canvas, which NEAREST makes blocky; the limit is 8192 against a 1920x1080 canvas here, so the
 * clamp never bites on this host. Nothing is drawn into a texture the driver could not hold:
 * `newPicture` returns null for a framebuffer the driver will not complete and the caller paints
 * each frame whole instead. The picture is the canvas's own size in device pixels, so any change of
 * camera, size, positions, style or theme colour starts it again from nothing (still.ts).
 */
import { deviceSize } from "./draw.ts";
import type { BulkLayer, Target } from "./layer.ts";

export interface Picture {
  readonly edges: Target;
  readonly nodes: Target;
  /** The texels each texture holds, 0 until the first frame sizes it. */
  width: number;
  height: number;
}

/** A canvas as the picture asks for a texture to hold it: its device pixels. */
export interface Sizeable {
  readonly viewport: { readonly width: number; readonly height: number };
  readonly dpr: number;
}

/**
 * The picture's size in texels: the canvas's device pixels, scaled by the longer side's overflow of
 * the driver's texture limit so both stay inside it and the aspect is kept. One texel each where the
 * driver reports no texture limit at all, which a frame over then draws into without complaint.
 */
export function pictureSize(view: Sizeable, limit: number): readonly [number, number] {
  const [width, height] = deviceSize(view);
  const longest = Math.max(width, height);
  const k = longest <= limit ? 1 : limit / longest;
  return [Math.max(1, Math.round(width * k)), Math.max(1, Math.round(height * k))];
}

function targetOf(layer: BulkLayer): Target | null {
  const { gl } = layer;
  const texture = gl.createTexture();
  const framebuffer = gl.createFramebuffer();
  if (texture === null || framebuffer === null) return null;
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
  const whole = gl.checkFramebufferStatus(gl.FRAMEBUFFER) === gl.FRAMEBUFFER_COMPLETE;
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  return whole ? { texture, framebuffer } : null;
}

/** The picture, or null where the driver will not complete a framebuffer. */
export function newPicture(layer: BulkLayer): Picture | null {
  const edges = targetOf(layer);
  const nodes = targetOf(layer);
  return edges === null || nodes === null ? null : { edges, nodes, width: 0, height: 0 };
}

/** Both of the picture's targets, edges first: the order the layer's passes paint in. */
export function targetsOf(picture: Picture): readonly Target[] {
  return [picture.edges, picture.nodes];
}

/**
 * Empties the picture for a frame that starts it again, reallocating the textures only where the
 * canvas moved size. Nothing of the last picture survives: a settled frame shows its own edges and
 * no others'.
 */
export function resetPicture(picture: Picture, layer: BulkLayer, size: readonly [number, number]): void {
  const { gl } = layer;
  const [width, height] = size;
  if (picture.width !== width || picture.height !== height) {
    picture.width = width;
    picture.height = height;
    for (const target of targetsOf(picture)) {
      gl.bindTexture(gl.TEXTURE_2D, target.texture);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    }
  }
  gl.clearColor(0, 0, 0, 0);
  for (const target of targetsOf(picture)) {
    gl.bindFramebuffer(gl.FRAMEBUFFER, target.framebuffer);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
}

/**
 * The two textures onto the canvas, edges under nodes, the whole of the picture at `alpha`. The
 * canvas is cleared first, or the frame before it would show through the transparent parts.
 */
export function showPicture(picture: Picture, layer: BulkLayer, alpha: number): void {
  const { gl, canvas, blit } = layer;
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  gl.activeTexture(gl.TEXTURE0);
  gl.useProgram(blit.program);
  gl.bindVertexArray(blit.vao);
  gl.uniform1i(blit.uniforms("u_texture"), 0);
  gl.uniform1f(blit.uniforms("u_alpha"), alpha);
  for (const target of targetsOf(picture)) {
    gl.bindTexture(gl.TEXTURE_2D, target.texture);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }
  gl.bindVertexArray(null);
}