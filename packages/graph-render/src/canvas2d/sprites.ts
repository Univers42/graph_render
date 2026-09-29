/**
 * Labels baked once and blitted after: a `drawImage` costs a fraction of a haloed
 * `strokeText` + `fillText` in the rasteriser, which is where the first studio spent its
 * frame (docs/measurements/studio-perf-baseline.md).
 *
 * Ponytail: the cache is bounded by count, not bytes, and evicts the least recently drawn
 * sprite. A view that shows more than CAPACITY distinct labels at once re-bakes on every
 * frame; the label budget (160) keeps a frame far below that.
 */
import type { Rgb } from "../colour/srgb.ts";
import type { Theme } from "../theme.ts";
import { createImpostorCache } from "./impostors.ts";
import type { SpriteFactory, SpriteImage, SpriteSurface } from "./surface.ts";

export interface Sprite<Image = SpriteImage> {
  readonly image: Image;
  /** CSS pixels. */
  readonly width: number;
  readonly height: number;
}

export interface SpriteCache<Image = SpriteImage> {
  /** The sprite for `text`, baked on a miss while this frame's allowance lasts. */
  get(text: string): Sprite<Image> | null;
  /** The lit sphere for a linear base colour, at `size` device pixels square. */
  sphere(base: Rgb, size: number): Sprite<Image> | null;
  /** Width of a sprite already baked, else 0. */
  widthOf(text: string): number;
  /** Starts a frame: a fresh allowance of bakes. */
  beginFrame(): void;
  /** True when this frame asked for more bakes than its allowance: paint another. */
  starved(): boolean;
  reset(theme: Theme, dpr: number): void;
}

const CAPACITY = 512;
const BAKES_PER_FRAME = 32;
const PADDING = 4;
const MAX_CHARACTERS = 48;

interface Entry<Image> {
  readonly sprite: Sprite<Image>;
  readonly surface: SpriteSurface<Image>;
}

interface State<Image> {
  theme: Theme;
  dpr: number;
  allowance: number;
  starved: boolean;
  readonly factory: SpriteFactory<Image>;
  readonly entries: Map<string, Entry<Image>>;
  readonly spare: SpriteSurface<Image>[];
}

function shown(text: string): string {
  return text.length > MAX_CHARACTERS ? `${text.slice(0, MAX_CHARACTERS - 1)}…` : text;
}

function bake<Image>(state: State<Image>, surface: SpriteSurface<Image>, text: string): Sprite<Image> {
  const { theme, dpr } = state;
  const box = theme.labelBox;
  const pad = box === null ? PADDING : box.padding;
  const height = theme.labelHeight;
  surface.ctx.font = theme.labelFont;
  const width = Math.ceil(surface.ctx.measureText(text).width) + pad * 2;
  surface.resize(Math.ceil(width * dpr), Math.ceil(height * dpr));
  const ctx = surface.ctx;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.font = theme.labelFont;
  ctx.textBaseline = "middle";
  if (box === null) {
    ctx.lineJoin = "round";
    ctx.lineWidth = 3;
    ctx.strokeStyle = theme.labelHalo;
    ctx.strokeText(text, pad, height / 2);
  } else {
    ctx.fillStyle = box.fill;
    ctx.fillRect(0, 0, width, height);
  }
  ctx.fillStyle = theme.label;
  ctx.fillText(text, pad, height / 2);
  return { image: surface.image, width, height };
}

function surfaceFor<Image>(state: State<Image>): SpriteSurface<Image> | null {
  const spare = state.spare.pop();
  if (spare !== undefined) return spare;
  if (state.entries.size < CAPACITY) return state.factory();
  // A Map iterates in insertion order and a hit re-inserts, so the first key is the oldest.
  const oldest = state.entries.entries().next().value;
  if (oldest === undefined) return state.factory();
  state.entries.delete(oldest[0]);
  return oldest[1].surface;
}

function lookUp<Image>(state: State<Image>, text: string): Sprite<Image> | null {
  const key = shown(text);
  const hit = state.entries.get(key);
  if (hit !== undefined) {
    state.entries.delete(key);
    state.entries.set(key, hit);
    return hit.sprite;
  }
  if (state.allowance === 0) {
    state.starved = true;
    return null;
  }
  const surface = surfaceFor(state);
  if (surface === null) return null;
  state.allowance -= 1;
  const entry = { sprite: bake(state, surface, key), surface };
  state.entries.set(key, entry);
  return entry.sprite;
}

export function createSpriteCache<Image = SpriteImage>(
  factory: SpriteFactory<Image>,
  theme: Theme,
): SpriteCache<Image> {
  const state: State<Image> = {
    theme, dpr: 1, allowance: BAKES_PER_FRAME, starved: false, factory, entries: new Map(), spare: [],
  };
  const spheres = createImpostorCache<Image>(factory);
  return {
    get: (text) => lookUp(state, text),
    sphere: (base, size) => spheres.get(base, size),
    widthOf: (text) => state.entries.get(shown(text))?.sprite.width ?? 0,
    beginFrame: () => {
      state.allowance = BAKES_PER_FRAME;
      state.starved = false;
      spheres.beginFrame();
    },
    starved: () => state.starved || spheres.starved(),
    reset: (next, dpr) => {
      spheres.reset(dpr);
      if (next === state.theme && dpr === state.dpr) return;
      for (const entry of state.entries.values()) state.spare.push(entry.surface);
      state.entries.clear();
      state.theme = next;
      state.dpr = dpr;
    },
  };
}
