/**
 * shadeSphere: one lit sphere per node colour, baked once and drawn scaled, so the
 * studio's nodes carry SciGraphs' impostor shading without a GPU.
 *   SciGraphs/ui/gpu_render/glsl/sphere_frag.glsl (z = sqrt(1-r2), n, the rim term)
 *   SciGraphs/ui/gpu_render/draw.py:12 (headlight (0,0,1)), 76-83 (ambient, no fill light)
 *   SciGraphs/ui/gpu_render/properties.py:1106 (ambient 0.25), 1111 (rim 0.15)
 */
import { srgbEncode } from "../colour/srgb.ts";
import type { Rgb } from "../colour/srgb.ts";

/** Ambient term of the shader's u_light.ambient (properties.py:1106). */
const AMBIENT = 0.25;

/** How hard the key light drives the lit side, against ambient (draw.py:78, 0.9 key colour). */
const KEY_GAIN = 0.9;

/** The rim highlight coefficient (properties.py:1111). */
const RIM = 0.15;

/**
 * The key direction. SciGraphs' fallback headlight is (0,0,1) (draw.py:12); this nudges
 * it toward (0.3, 0.3) so a lit sphere reads as a sphere rather than a flat disc.
 */
const KEY: readonly [number, number, number] = (() => {
  const length = Math.hypot(0.3, 0.3, 1);
  return [0.3 / length, 0.3 / length, 1 / length];
})();

/** n.key, the clamped cosine between the surface normal and the key (sphere_frag.glsl:11). */
function keyDot(nx: number, ny: number, nz: number): number {
  return Math.max(nx * KEY[0] + ny * KEY[1] + nz * KEY[2], 0);
}

/**
 * Ponytail: this models the NO-LAMP fallback rig only. SciGraphs' use_scene_lights
 * defaults to True (properties.py:1087-1094), so a scene with a sun takes its key and
 * fill from the lamps and not from the headlight (draw.py:44-73), and the gallery spec
 * has one at energy 3.0 (05-reproducible-pipeline.qmd:147-151); the shaded bytes move
 * by up to +36 and -45 per channel against that rig at a 64 px sprite. The fallback
 * also multiplies ambient by the World colour when the world has no node tree
 * (draw.py:38-42), while this is untinted 0.25. The fill light is black in the source
 * too (draw.py:80), so nothing is dropped there; sRGB encoding here stands in for
 * Blender's Standard view transform, which the shader itself does not apply
 * (sphere_frag.glsl:15 writes linear), and the alpha edge is cut hard with no
 * antialiasing. The escape hatch for the aliasing is to draw the sprite scaled, which
 * is what the painter does; for the rig, pass the lamp directions in.
 */
function shadePixel(base: Rgb, px: number, py: number, size: number): readonly [number, number, number, number] {
  const x = ((px + 0.5) / size) * 2 - 1;
  const y = 1 - ((py + 0.5) / size) * 2;
  const r2 = x * x + y * y;
  if (r2 > 1) return [0, 0, 0, 0];
  const z = Math.sqrt(1 - r2);
  const light = AMBIENT + KEY_GAIN * keyDot(x, y, z);
  const rim = (1 - z) ** 3 * RIM;
  return [
    Math.min(255, Math.round(255 * srgbEncode(base[0] * light + rim))),
    Math.min(255, Math.round(255 * srgbEncode(base[1] * light + rim))),
    Math.min(255, Math.round(255 * srgbEncode(base[2] * light + rim))),
    255,
  ];
}

/**
 * The sprite for one linear base colour: size*size RGBA pixels, row-major from the top.
 * A size that is not a positive integer is refused rather than rounded, because a
 * sprite cache keyed on radius times device pixel ratio can hand over a fraction and
 * the buffer would no longer be size*size RGBA.
 */
export function shadeSphere(base: Rgb, size: number): Uint8ClampedArray {
  if (!Number.isInteger(size) || size <= 0) {
    throw new RangeError(`sprite size must be a positive integer, got ${size}`);
  }
  const out = new Uint8ClampedArray(size * size * 4);
  for (let py = 0; py < size; py += 1) {
    for (let px = 0; px < size; px += 1) {
      const [r, g, b, a] = shadePixel(base, px, py, size);
      const at = (py * size + px) * 4;
      out[at] = r;
      out[at + 1] = g;
      out[at + 2] = b;
      out[at + 3] = a;
    }
  }
  return out;
}
