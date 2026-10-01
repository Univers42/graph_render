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

/** The knee the other way round: the encoded value 12.92 and 0.04045 meet at. */
const DECODE_KNEE = 0.04045;
const DECODE_SCALE = 2.4;

/** The linear-light channel of one normalised sRGB value: the exact inverse of srgbEncode. */
export function srgbDecode(c: number): number {
  const x = clamp01(c);
  return x <= DECODE_KNEE ? x / SLOPE : ((x + OFFSET) / SCALE) ** DECODE_SCALE;
}

/** One `rrggbb` (or `rgb`) pair of hex digits as a linear channel. */
function channelOf(pair: string): number {
  return srgbDecode(Number.parseInt(pair, 16) / 255);
}

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;
const RGB = /^rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)$/;

/**
 * The linear triple a palette entry was drawn from, so an edge can be blended in linear
 * light from the very colours its nodes are filled with. `#rgb`, `#rrggbb` and the
 * `rgb(r, g, b)` cssOf writes are read; anything else (a named colour, a user-typed
 * string, `rgba(...)`) is null, and the caller then has no edge gradient to draw.
 */
export function linearOf(css: string): Rgb | null {
  const text = css.trim();
  const hex = HEX.exec(text);
  if (hex !== null) {
    const d = hex[1] ?? "";
    // Three digits are one per channel and are doubled; six are two per channel already.
    const pair = (at: number): string => (d.length === 3 ? `${d[at]}${d[at]}` : d.slice(2 * at, 2 * at + 2));
    return [channelOf(pair(0)), channelOf(pair(1)), channelOf(pair(2))];
  }
  const bytes = RGB.exec(text);
  if (bytes === null) return null;
  return [srgbDecode(Number(bytes[1]) / 255), srgbDecode(Number(bytes[2]) / 255), srgbDecode(Number(bytes[3]) / 255)];
}
