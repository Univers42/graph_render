/**
 * The programs of the GPU layer. The points and the edge lines read the node positions per
 * vertex from the same two buffers (the lines through an element list of node indices); the
 * quads read them per instance from compact copies holding only the nodes on screen. The camera is the 2D view's own (`screen = world·scale + offset`,
 * in CSS pixels), so the layer draws exactly where the 2D painter would.
 *
 * Output is premultiplied, blended with ONE, ONE_MINUS_SRC_ALPHA onto a clear canvas that
 * the 2D context then composites over its ground.
 */

/**
 * The GL viewport overhangs the canvas by `u_margin` CSS pixels on every side: a point whose
 * centre is just off screen is clipped whole, and the overhang keeps it until it is gone.
 */
const SCREEN = `
uniform vec3 u_camera;
uniform vec2 u_viewport;
uniform float u_margin;
vec4 clipOf(vec2 screen) {
  vec2 at = screen + u_margin;
  return vec4(at.x / u_viewport.x * 2.0 - 1.0, 1.0 - at.y / u_viewport.y * 2.0, 0.0, 1.0);
}
`;

const PALETTE = `
uniform highp sampler2D u_palette;
uniform int u_paletteSize;
uniform int u_paletteWidth;
vec4 paletteOf(uint slot) {
  int at = min(int(slot), u_paletteSize - 1);
  return texelFetch(u_palette, ivec2(at % u_paletteWidth, at / u_paletteWidth), 0);
}
`;

/** The sRGB transfer both ways, as srgb.ts writes it: an edge blends in linear light. */
const TRANSFER = `
vec3 decode(vec3 c) {
  return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}
vec3 encode(vec3 c) {
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
`;

const NODE_INPUTS = `
in float a_x;
in float a_y;
in vec2 a_half;
in uint a_colour;
uniform float u_minRadius;
uniform float u_pad;
uniform int u_box;
${SCREEN}${PALETTE}
vec2 halfOf(vec2 half_) {
  return u_box == 1 ? max(half_ * u_camera.z, vec2(0.5)) : vec2(max(u_minRadius, half_.x * u_camera.z));
}
`;

export const NODE_VERTEX = `#version 300 es
${NODE_INPUTS}
out vec2 v_local;
out vec2 v_half;
flat out vec4 v_colour;
void main() {
  if (a_half.x < 0.0) {
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    return;
  }
  vec2 centre = vec2(a_x, a_y) * u_camera.z + u_camera.xy;
  vec2 half_ = halfOf(a_half);
  vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 2.0 - 1.0;
  v_local = corner * (half_ + u_pad);
  v_half = half_;
  v_colour = paletteOf(a_colour);
  gl_Position = clipOf(centre + v_local);
}
`;

/** A disc or a box with its rim, antialiased over one device pixel, at `local` CSS pixels from its centre. */
const SHADE = `
uniform int u_box;
uniform vec4 u_rim;
uniform float u_pixel;
uniform float u_alpha;
out vec4 o_colour;
void shade(vec2 local, vec2 half_, vec4 colour) {
  float cover;
  if (u_box == 1) {
    vec2 d = abs(local) - half_;
    float dist = max(d.x, d.y);
    cover = clamp(0.5 + (0.5 - dist) / u_pixel, 0.0, 1.0);
    colour = mix(colour, u_rim, clamp(0.5 + (dist + 0.5) / u_pixel, 0.0, 1.0));
  } else {
    cover = clamp(0.5 - (length(local) - half_.x) / u_pixel, 0.0, 1.0);
  }
  float alpha = colour.a * cover * u_alpha;
  if (alpha <= 0.0) discard;
  o_colour = vec4(colour.rgb * alpha, alpha);
}
`;

export const NODE_FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
in vec2 v_local;
in vec2 v_half;
flat in vec4 v_colour;
${SHADE}
void main() {
  shade(v_local, v_half, v_colour);
}
`;

/**
 * The same nodes as one point each: a vertex per node instead of an instanced quad of four,
 * which software rasterisers draw several times faster. Used only while every node fits in
 * a point (`pointLimit` in layer.ts): a larger point is clamped by the driver.
 */
export const POINT_VERTEX = `#version 300 es
${NODE_INPUTS}
uniform float u_dpr;
flat out vec2 v_half;
flat out float v_reach;
flat out vec4 v_colour;
void main() {
  if (a_half.x < 0.0) {
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    gl_PointSize = 0.0;
    return;
  }
  v_half = halfOf(a_half);
  v_reach = max(v_half.x, v_half.y) + u_pad;
  v_colour = paletteOf(a_colour);
  gl_PointSize = 2.0 * v_reach * u_dpr;
  gl_Position = clipOf(vec2(a_x, a_y) * u_camera.z + u_camera.xy);
}
`;

export const POINT_FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
flat in vec2 v_half;
flat in float v_reach;
flat in vec4 v_colour;
${SHADE}
void main() {
  shade((gl_PointCoord * 2.0 - 1.0) * v_reach, v_half, v_colour);
}
`;

export const EDGE_VERTEX = `#version 300 es
in float a_x;
in float a_y;
in uint a_colour;
uniform int u_gradient;
${SCREEN}${PALETTE}${TRANSFER}
out vec4 v_linear;
void main() {
  vec4 colour = u_gradient == 1 ? paletteOf(a_colour) : vec4(0.0);
  v_linear = vec4(decode(colour.rgb), colour.a);
  gl_Position = clipOf(vec2(a_x, a_y) * u_camera.z + u_camera.xy);
}
`;

export const EDGE_FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
in vec4 v_linear;
uniform int u_gradient;
uniform vec4 u_edge;
uniform float u_alpha;
${TRANSFER}
out vec4 o_colour;
void main() {
  vec4 colour = u_gradient == 1 ? vec4(encode(v_linear.rgb), v_linear.a) : u_edge;
  float alpha = colour.a * u_alpha;
  o_colour = vec4(colour.rgb * alpha, alpha);
}
`;

/**
 * The settled picture shown: a quad over the whole canvas, four corners and no buffer, sampling
 * one of the picture's textures. `o_colour` is premultiplied already, so the dim is every channel
 * multiplied by `u_alpha` and the blend stays ONE, ONE_MINUS_SRC_ALPHA (picture.ts).
 */
export const BLIT_VERTEX = `#version 300 es
out vec2 v_uv;
void main() {
  vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
  v_uv = corner;
  gl_Position = vec4(corner * 2.0 - 1.0, 0.0, 1.0);
}
`;

export const BLIT_FRAGMENT = `#version 300 es
precision highp float;
uniform sampler2D u_texture;
uniform float u_alpha;
in vec2 v_uv;
out vec4 o_colour;
void main() {
  o_colour = texture(u_texture, v_uv) * u_alpha;
}
`;
