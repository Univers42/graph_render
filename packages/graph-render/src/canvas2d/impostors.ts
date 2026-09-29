/**
 * One lit sphere per (base colour, size), baked with shadeSphere and blitted by the node
 * painter. The impostor half of the label sprite cache's frame allowance.
 *   SciGraphs/ui/gpu_render/draw.py:12,76-83 (the headlight and the no-fill rig)
 *   SciGraphs/ui/gpu_render/glsl/sphere_frag.glsl:11,15 (the key dot and the linear write)
 */
import { type Rgb, cssOf } from "../colour/srgb.ts";
import { shadeSphere } from "../sprite/impostor.ts";
import type { Sprite } from "./sprites.ts";
import type { SpriteFactory, SpriteImage } from "./surface.ts";

/** Bakes a frame may afford before the painter has to come back for the rest. */
const BAKES_PER_FRAME = 32;

/**
 * Past this many pixels on a side a sphere is a rasteriser's problem, not a node's.
 *
 * Ponytail: 512 is where baking a sphere costs more than the disc it replaces (a
 * 512x512 sphere is a megabyte of shading for one node), so a bigger one is refused and
 * the node is not drawn rather than drawn flat — the caller decides that. The escape
 * hatch is to raise it, at the cost of a frame the rasteriser has to chew.
 */
const MAX_SIZE = 512;

export interface ImpostorCache<Image = SpriteImage> {
  /** The sphere for `base` at `size` device pixels square, baked on a miss. */
  get(base: Rgb, size: number): Sprite<Image> | null;
  /** Starts a frame: a fresh allowance of bakes. */
  beginFrame(): void;
  /** True when this frame asked for more bakes than its allowance. */
  starved(): boolean;
  reset(dpr: number): void;
}

interface State<Image> {
  dpr: number;
  allowance: number;
  starved: boolean;
  readonly factory: SpriteFactory<Image>;
  readonly entries: Map<string, Sprite<Image>>;
}

function keyOf(base: Rgb, size: number): string {
  return `${size}:${cssOf(base)}`;
}

/**
 * The size has to be whole and no bigger than MAX_SIZE, because the buffer shadeSphere
 * fills is size*size RGBA; the painter rounds its own radius, so a refusal here is a
 * caller error, and it comes back as null rather than as a throw in the middle of a frame.
 */
function bakeable(size: number): boolean {
  return Number.isInteger(size) && size > 0 && size <= MAX_SIZE;
}

function bake<Image>(state: State<Image>, base: Rgb, size: number): Sprite<Image> | null {
  const surface = state.factory();
  const pixels = surface?.pixels;
  if (surface === null || pixels === undefined) return null;
  surface.resize(size, size);
  pixels.putPixels(shadeSphere(base, size), size, size);
  return { image: surface.image, width: size, height: size };
}

export function createImpostorCache<Image = SpriteImage>(
  factory: SpriteFactory<Image>,
): ImpostorCache<Image> {
  const state: State<Image> = { dpr: 1, allowance: BAKES_PER_FRAME, starved: false, factory, entries: new Map() };
  return {
    get: (base, size) => {
      if (!bakeable(size)) return null;
      const key = keyOf(base, size);
      const hit = state.entries.get(key);
      if (hit !== undefined) return hit;
      if (state.allowance === 0) {
        state.starved = true;
        return null;
      }
      state.allowance -= 1;
      const sprite = bake(state, base, size);
      if (sprite !== null) state.entries.set(key, sprite);
      return sprite;
    },
    beginFrame: () => {
      state.allowance = BAKES_PER_FRAME;
      state.starved = false;
    },
    starved: () => state.starved,
    reset: (dpr) => {
      if (dpr === state.dpr) return;
      state.entries.clear();
      state.dpr = dpr;
    },
  };
}
