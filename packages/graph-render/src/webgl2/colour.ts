/**
 * Colours for the GPU layer, as bytes. A CSS colour is first normalised by the host's own 2D
 * context (`fillStyle` reads back as `#rrggbb` or `rgba(r, g, b, a)` for any colour it
 * accepts), so the parser below needs only those two forms and never re-implements CSS.
 */

/** sRGB-encoded bytes and a straight (not premultiplied) alpha byte. */
export type Rgba = readonly [number, number, number, number];

const HEX6 = /^#([0-9a-f]{6})$/i;
const RGBA = /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*(?:,\s*([0-9.]+)\s*)?\)$/;

/** The bytes of a normalised colour, or null for anything else. */
export function rgbaOf(normalised: string): Rgba | null {
  const text = normalised.trim();
  const hex = HEX6.exec(text);
  if (hex !== null) {
    const value = Number.parseInt(hex[1] ?? "0", 16);
    return [(value >> 16) & 255, (value >> 8) & 255, value & 255, 255];
  }
  const parts = RGBA.exec(text);
  if (parts === null) return null;
  const alpha = parts[4] === undefined ? 1 : Number(parts[4]);
  if (!(alpha >= 0 && alpha <= 1)) return null;
  const channel = (at: number): number => Math.min(255, Number(parts[at]));
  return [channel(1), channel(2), channel(3), Math.round(alpha * 255)];
}

/** What turns any CSS colour into one of the two forms `rgbaOf` reads. */
export type Normalise = (css: string) => string;

/** A normaliser over a 2D context: an unparseable colour leaves the reset value in place. */
export function normaliserOf(ctx: { fillStyle: unknown }): Normalise {
  return (css) => {
    ctx.fillStyle = "#000000";
    ctx.fillStyle = css;
    return String(ctx.fillStyle);
  };
}

/** Mid grey, the 2D painter's own fallback for a missing palette entry (nodes.ts). */
const FALLBACK: Rgba = [0x9a, 0x9a, 0x9a, 255];

/** The colour of one CSS string as bytes, the fallback grey when it cannot be read. */
export function bytesOf(css: string, normalise: Normalise): Rgba {
  return rgbaOf(normalise(css)) ?? FALLBACK;
}

/** One RGBA texel per palette entry, in palette order. */
export function paletteTexels(palette: readonly string[], normalise: Normalise): Uint8Array {
  const texels = new Uint8Array(Math.max(1, palette.length) * 4);
  palette.forEach((css, at) => texels.set(bytesOf(css, normalise), at * 4));
  return texels;
}
