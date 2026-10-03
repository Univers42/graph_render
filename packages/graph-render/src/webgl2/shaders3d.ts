/**
 * The programs of the WebGL2 3D path: every node as one instanced quad, every edge as one
 * instanced quad, both projected on the GPU by the orbit camera as one matrix
 * (`three/matrix.ts`). The positions live in one float texture read by node index, so the
 * nodes and both ends of every edge read the same upload, and an orbit drag uploads nothing:
 * it changes `u_camera` and nothing else.
 *
 * The node fragment is the 2D layer's own (`NODE_FRAGMENT`, shaders.ts): a disc, or a box with
 * its rim, antialiased over one device pixel, so a node looks as the 2D GPU layer draws it.
 * The depth written is the eye depth on a log scale, so the depth test keeps the nearest node
 * on top whatever order the instances come in, and the CPU never sorts.
 *
 * Output is premultiplied, blended with ONE, ONE_MINUS_SRC_ALPHA onto a clear canvas that the
 * 2D context composites over its ground, as the 2D layer's is.
 *
 * Ponytail: edges are straight between their two ends, whatever the frame's edge kind; a
 * routed or curved 3D edge is drawn as its chord (the Canvas2D painter traces the interior
 * points of a settled one). It fails for a Polyline/Curve 3D layout, whose bends are lost;
 * the escape hatch is `?backend=canvas2d`.
 *
 * Ponytail: a node below 1.75 px is a disc here and a square on the Canvas2D painter
 * (`three/paint3d.ts` DOT_RADIUS), and each edge blends on its own, so a dense bundle reads
 * darker where Canvas2D's one stroke covers a pixel once. Both are inside the parity
 * tolerance the gate measured (`scripts/studio-3d-gl.sh`).
 */
/** Below this eye depth a point is behind the eye: `three/orbit.ts` NEAR. */
const SPACE = `
const float NEAR = 1e-3;
uniform mat4 u_camera;
uniform highp sampler2D u_positions;
uniform int u_positionsWidth;
uniform vec2 u_viewport;
vec4 eyeOf(int node) {
  vec3 p = texelFetch(u_positions, ivec2(node % u_positionsWidth, node / u_positionsWidth), 0).xyz;
  return u_camera * vec4(p, 1.0);
}
vec4 clipOf(vec2 screen, float depth) {
  float z = clamp(log2(depth / NEAR) / 32.0 - 1.0, -1.0, 1.0);
  return vec4(screen.x / u_viewport.x * 2.0 - 1.0, 1.0 - screen.y / u_viewport.y * 2.0, z, 1.0);
}
const vec4 HIDDEN = vec4(2.0, 2.0, 2.0, 1.0);
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

/**
 * One node: `v.xy / v.w` is its screen point, `v.w` its depth and `v.z / v.w` the pixels one
 * world unit covers there, so its drawn half size is its world half size times that.
 */
export const SPACE_NODE_VERTEX = `#version 300 es
in vec2 a_half;
in uint a_colour;
uniform float u_minRadius;
uniform float u_pad;
uniform int u_box;
${SPACE}${PALETTE}
out vec2 v_local;
out vec2 v_half;
flat out vec4 v_colour;
void main() {
  vec4 eye = eyeOf(gl_InstanceID);
  if (a_half.x < 0.0 || eye.w <= NEAR) {
    gl_Position = HIDDEN;
    return;
  }
  float scale = eye.z / eye.w;
  vec2 half_ = u_box == 1 ? max(a_half * scale, vec2(0.5)) : vec2(max(u_minRadius, a_half.x * scale));
  vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 2.0 - 1.0;
  v_local = corner * (half_ + u_pad);
  v_half = half_;
  v_colour = paletteOf(a_colour);
  gl_Position = clipOf(eye.xy / eye.w + v_local, eye.w);
}
`;

/** One edge: a quad along the screen chord of its two ends, a pixel wider than the stroke for the fringe. */
export const SPACE_EDGE_VERTEX = `#version 300 es
in uvec2 a_ends;
uniform float u_width;
uniform float u_pixel;
${SPACE}
out float v_across;
flat out float v_half;
void main() {
  vec4 a = eyeOf(int(a_ends.x));
  vec4 b = eyeOf(int(a_ends.y));
  if (a.w <= NEAR || b.w <= NEAR) {
    gl_Position = HIDDEN;
    return;
  }
  vec2 from = a.xy / a.w;
  vec2 to = b.xy / b.w;
  vec2 along = to - from;
  float length_ = length(along);
  vec2 normal = length_ > 0.0 ? vec2(-along.y, along.x) / length_ : vec2(0.0, 1.0);
  v_half = u_width * 0.5;
  v_across = (float(gl_VertexID & 1) * 2.0 - 1.0) * (v_half + u_pixel);
  gl_Position = clipOf(mix(from, to, float(gl_VertexID >> 1)) + normal * v_across, NEAR);
}
`;

/** The share of one device pixel, across the edge, that the stroke covers: a box filter, as Canvas2D's. */
export const SPACE_EDGE_FRAGMENT = `#version 300 es
precision highp float;
in float v_across;
flat in float v_half;
uniform vec4 u_edge;
uniform float u_pixel;
out vec4 o_colour;
void main() {
  float d = abs(v_across);
  float lo = max(d - u_pixel * 0.5, -v_half);
  float hi = min(d + u_pixel * 0.5, v_half);
  float alpha = u_edge.a * clamp((hi - lo) / u_pixel, 0.0, 1.0);
  if (alpha <= 0.0) discard;
  o_colour = vec4(u_edge.rgb * alpha, alpha);
}
`;
