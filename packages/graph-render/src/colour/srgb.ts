/**
 * The single encode step every drawn colour goes through: SciGraphs bakes linear
 * values and the GPU/Blender converts on output (SciGraphs/api/render.py:45-101
 * stores linear background, color and edge_color triples).
 *   SciGraphs/engine/scigraphs_engine/backends/wgpu/target.py:19-25 (to_display)
 */

/** A linear-light triple, 0..1 per channel. */
export type Rgb = readonly [number, number, number];

/** The sRGB transfer function, IEC 61966-2-1; the 0.0031308 knee is its own. */
const KNEE = 0.0031308;
const SLOPE = 12.92;
const SCALE = 1.055;
const GAMMA = 1 / 2.4;
const OFFSET = 0.055;

function clamp01(c: number): number {
  if (!(c > 0)) return 0;
  return c > 1 ? 1 : c;
}

/** Encode one linear channel to sRGB. Out-of-range input is clamped, not extrapolated. */
export function srgbEncode(c: number): number {
  const x = clamp01(c);
  return x <= KNEE ? SLOPE * x : SCALE * x ** GAMMA - OFFSET;
}

/** The 8-bit byte of one linear channel, rounded once. */
export function byteOf(c: number): number {
  return Math.round(255 * srgbEncode(c));
}

/** The CSS colour of a linear triple. Each channel is encoded exactly once. */
export function cssOf(rgb: Rgb): string {
  const [r, g, b] = rgb;
  return `rgb(${byteOf(r)}, ${byteOf(g)}, ${byteOf(b)})`;
}
