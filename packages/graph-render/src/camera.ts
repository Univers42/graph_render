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
export const DEFAULT_LIMITS: ZoomLimits = { min: 0.15, max: 6 };
export const FIT_PADDING = 64;

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

/** The scale at which `bounds` fills the viewport minus the padding, before any clamp. */
export function fitScale(bounds: Bounds, viewport: Viewport): number {
  const worldW = Math.max(1, bounds.maxX - bounds.minX);
  const worldH = Math.max(1, bounds.maxY - bounds.minY);
  const roomW = Math.max(1, viewport.width - FIT_PADDING * 2);
  const roomH = Math.max(1, viewport.height - FIT_PADDING * 2);
  return Math.min(roomW / worldW, roomH / worldH);
}

/** The limits that let `bounds` be seen whole: the floor drops to the fit scale. */
export function limitsFor(bounds: Bounds | null, viewport: Viewport): ZoomLimits {
  if (bounds === null) return DEFAULT_LIMITS;
  return { min: Math.min(DEFAULT_LIMITS.min, fitScale(bounds, viewport) * 0.5), max: DEFAULT_LIMITS.max };
}

export function fitCamera(bounds: Bounds | null, viewport: Viewport): Camera {
  if (bounds === null) return IDENTITY;
  const limits = limitsFor(bounds, viewport);
  // Never past 2: three nodes zoomed to the ceiling read as a broken view.
  const scale = clamp(fitScale(bounds, viewport), limits.min, Math.min(2, limits.max));
  const centerX = (bounds.minX + bounds.maxX) / 2;
  const centerY = (bounds.minY + bounds.maxY) / 2;
  return { scale, x: viewport.width / 2 - centerX * scale, y: viewport.height / 2 - centerY * scale };
}

export function centreOn(camera: Camera, world: Point, viewport: Viewport): Camera {
  return {
    scale: camera.scale,
    x: viewport.width / 2 - world.x * camera.scale,
    y: viewport.height / 2 - world.y * camera.scale,
  };
}
