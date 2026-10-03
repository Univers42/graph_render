/**
 * World ↔ screen: `screen = world · scale + offset`. Ported from the osionos engine's
 * `camera/{transform,controls}.ts` (docs/decisions/render-ports-not-imports.md), with the
 * zoom limits made a parameter: a 20 000-node graph does not fit at the engine's 0.15 floor.
 */

export interface Camera {
  /** Screen-space offset of the world origin, in CSS pixels. */
  readonly x: number;
  readonly y: number;
  readonly scale: number;
}

export interface Point {
  readonly x: number;
  readonly y: number;
}

export interface Bounds {
  readonly minX: number;
  readonly minY: number;
  readonly maxX: number;
  readonly maxY: number;
}

export interface Viewport {
  readonly width: number;
  readonly height: number;
}

export interface ZoomLimits {
  readonly min: number;
  readonly max: number;
}

export const IDENTITY: Camera = { x: 0, y: 0, scale: 1 };
/**
 * The studio's own limits (S1): a label is readable at ×0.02 and a 20 000-node graph is
 * still worth looking at ×40. `limitsFor` may drop the floor further for a drawing that
 * would not fit at all, which is what that function is for.
 */
export const DEFAULT_LIMITS: ZoomLimits = { min: 0.02, max: 40 };
export const FIT_PADDING = 64;
/** What `fitCamera` never zooms past: three nodes at the ceiling read as a broken view. */
export const FIT_MAX_SCALE = 2;

/**
 * The box inside the viewport a fit puts its drawing in and centres it on. A host whose chrome
 * lies over the canvas passes the part the chrome leaves visible; the renderer's own default is
 * the whole canvas, which is right for a view with nothing on top of it.
 */
export interface FitArea {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/**
 * How a fit is measured. Every field is optional and every default is the renderer's own
 * behaviour, so a caller that passes nothing gets the fit this file had before the options.
 */
export interface FitOptions {
  /** The box fitted and centred in; the whole viewport when absent. */
  readonly area?: FitArea | undefined;
  /** The scale ceiling; `FIT_MAX_SCALE` (2) when absent. */
  readonly maxScale?: number | undefined;
  /** Slack in CSS pixels on every side of `area`; `FIT_PADDING` (64) when absent. */
  readonly padding?: number | undefined;
  /**
   * Slack as a factor on the room rather than in pixels: `1.12` fits the drawing into 89% of
   * it, which is how a look states its margin (`look/presets.ts` FIT_MARGIN). 1 when absent.
   * A pixel padding and a factor compose: the room is shrunk by the padding and then divided.
   */
  readonly margin?: number | undefined;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function worldToScreen(camera: Camera, world: Point): Point {
  return { x: world.x * camera.scale + camera.x, y: world.y * camera.scale + camera.y };
}

export function screenToWorld(camera: Camera, screen: Point): Point {
  return { x: (screen.x - camera.x) / camera.scale, y: (screen.y - camera.y) / camera.scale };
}

/** Zoom by `factor`, keeping the world point under `at` where it is on screen. */
export function zoomAt(camera: Camera, at: Point, factor: number, limits: ZoomLimits = DEFAULT_LIMITS): Camera {
  const scale = clamp(camera.scale * factor, limits.min, limits.max);
  const ratio = scale / camera.scale;
  return { scale, x: at.x - (at.x - camera.x) * ratio, y: at.y - (at.y - camera.y) * ratio };
}

export function panBy(camera: Camera, delta: Point): Camera {
  return { scale: camera.scale, x: camera.x + delta.x, y: camera.y + delta.y };
}

/** The room inside `area` that is left once the padding and the margin are taken off it. */
function roomOf(area: FitArea, options: FitOptions): { width: number; height: number } {
  const padding = options.padding ?? FIT_PADDING;
  const margin = options.margin ?? 1;
  return {
    width: Math.max(1, (area.width - padding * 2) / margin),
    height: Math.max(1, (area.height - padding * 2) / margin),
  };
}

/** `area` when the caller named one, and the whole viewport when it did not. */
export function areaOf(viewport: Viewport, options?: FitOptions): FitArea {
  return options?.area ?? { x: 0, y: 0, width: viewport.width, height: viewport.height };
}

/** The scale at which `bounds` fills the room, before any clamp. */
export function fitScale(bounds: Bounds, viewport: Viewport, options?: FitOptions): number {
  const worldW = Math.max(1, bounds.maxX - bounds.minX);
  const worldH = Math.max(1, bounds.maxY - bounds.minY);
  const room = roomOf(areaOf(viewport, options), options ?? {});
  return Math.min(room.width / worldW, room.height / worldH);
}

/** The limits that let `bounds` be seen whole: the floor drops to the fit scale. */
export function limitsFor(bounds: Bounds | null, viewport: Viewport, options?: FitOptions): ZoomLimits {
  if (bounds === null) return DEFAULT_LIMITS;
  return { min: Math.min(DEFAULT_LIMITS.min, fitScale(bounds, viewport, options) * 0.5), max: DEFAULT_LIMITS.max };
}

export function fitCamera(bounds: Bounds | null, viewport: Viewport, options?: FitOptions): Camera {
  if (bounds === null) return IDENTITY;
  const area = areaOf(viewport, options);
  const limits = limitsFor(bounds, viewport, options);
  const ceiling = Math.min(options?.maxScale ?? FIT_MAX_SCALE, limits.max);
  const scale = clamp(fitScale(bounds, viewport, options), limits.min, ceiling);
  // Centred in `area`, not in the viewport: a fit that ignores the chrome puts a third of the
  // drawing under the panels the user cannot see through.
  const centerX = (bounds.minX + bounds.maxX) / 2;
  const centerY = (bounds.minY + bounds.maxY) / 2;
  return {
    scale,
    x: area.x + area.width / 2 - centerX * scale,
    y: area.y + area.height / 2 - centerY * scale,
  };
}

export function centreOn(camera: Camera, world: Point, viewport: Viewport): Camera {
  return {
    scale: camera.scale,
    x: viewport.width / 2 - world.x * camera.scale,
    y: viewport.height / 2 - world.y * camera.scale,
  };
}

/** 1:1 with the world origin in the middle: what the key `0` and the reset button mean. */
export function resetCamera(viewport: Viewport): Camera {
  return { scale: 1, x: viewport.width / 2, y: viewport.height / 2 };
}
