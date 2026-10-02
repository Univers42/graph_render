/**
 * The 3D camera: an orbit around a target, projected in perspective. A `dim = 1` snapshot
 * carries a z column, and a 2D camera has nothing to say about where a node is in it, so
 * this one turns the drawing around a point and pulls it nearer.
 *
 * The 2D camera (`camera.ts`) is untouched and still owns every 2D frame: a 2D snapshot
 * takes the same path, byte for byte. This module reads the z column and nothing else.
 *
 * WHY the world up is `-y`: the 2D painter draws y downwards, so a height that reads as up
 * on the screen is the negative of y here. At yaw 0 and pitch 0 the eye sits on the +z side
 * of the target and looks down -z, and a node with `z = 0` projects exactly as the 2D camera
 * draws it. So a 3D layout that is flat in z opens on the drawing the 2D layouts would have
 * given, and the first drag is what makes it 3D. That identity is the module's negative
 * control (`tests/three-orbit.test.ts`), and it is why the basis is spelled out here rather
 * than borrowed from a matrix library.
 *
 * Ponytail: the perspective divide by `depth` is the only depth cue — no per-node shading,
 * no fog, no z-buffer. A node at or behind the eye plane is dropped rather than clamped,
 * so a large yaw swings past a node and it leaves the drawing instead of smearing across
 * the viewport; the drag back brings it. That is the one thing this projection cannot do.
 */
import { type Viewport, type ZoomLimits, clamp } from "../camera.ts";

export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface Orbit {
  /** Radians about the screen's vertical axis; a positive one turns the drawing right. */
  readonly yaw: number;
  /**
   * Radians about the screen's horizontal axis. A positive one tips the drawing's top away
   * from the viewer, which is the direction a hand dragging down expects: the drawing rolls
   * back, as if you were pushing its top edge away from you.
   */
  readonly pitch: number;
  /** Eye to target, in world units. */
  readonly distance: number;
  readonly target: Vec3;
  /** The vertical field of view, in radians. */
  readonly fov: number;
  /** The distances this orbit will not pass, taken from the fit that made it. */
  readonly limits: ZoomLimits;
}

/** Where a node lands, and how far in front of the eye it is. */
export interface Projected {
  readonly x: number;
  readonly y: number;
  /** Eye to node along the view direction, in world units. At or below `NEAR` is behind. */
  readonly depth: number;
}

/**
 * The three screen axes, so a world point is two dots and a depth. It carries the orbit's
 * target and distance as well, so a projection is four arguments and no more.
 */
export interface Basis {
  readonly right: Vec3;
  readonly up: Vec3;
  readonly forward: Vec3;
  readonly target: Vec3;
  readonly distance: number;
}

/** The box a z-carrying drawing occupies, as a pair of corners. */
export interface Box3 {
  readonly min: Vec3;
  readonly max: Vec3;
}

export const DEFAULT_FOV = Math.PI / 3;
/** Pitch stops short of the poles, where the view direction is parallel to the world up. */
export const PITCH_LIMIT = Math.PI / 2 - 0.05;
/** A node nearer this than the eye is behind it, and the divide is not taken. */
export const NEAR = 1e-3;
/** Radians per pixel of drag: a 200 px drag is about a third of a turn. */
export const ROTATION_PER_PX = 0.01;
/** A fit leaves this much room, so the outermost node is not on the edge of the viewport. */
const FIT_MARGIN = 1.2;

const ZERO: Vec3 = { x: 0, y: 0, z: 0 };
/** The 2D painter draws y downwards, so the screen's "up" is -y and "right" is +x. */
const WORLD_UP: Vec3 = { x: 0, y: -1, z: 0 };

function dot(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

function cross(a: Vec3, b: Vec3): Vec3 {
  return { x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x };
}

function unit(v: Vec3): Vec3 {
  const length = Math.sqrt(dot(v, v));
  return length > 0 ? { x: v.x / length, y: v.y / length, z: v.z / length } : v;
}

/** The limits a fitted orbit gets: a hundredth of the fit, and a hundred times it. */
export function limitsFor(distance: number): ZoomLimits {
  return { min: Math.max(NEAR * 10, distance / 100), max: distance * 100 };
}

export function identityOrbit(): Orbit {
  return { yaw: 0, pitch: 0, distance: 1, target: ZERO, fov: DEFAULT_FOV, limits: limitsFor(1) };
}

/**
 * The direction the eye looks, from the orbit's two angles: yaw turns it about the world
 * up, pitch tips it. At yaw 0 and pitch 0 it is `-z`, so the eye is on the +z side of the
 * target and the x-y plane is the screen plane — the 2D view.
 */
function forwardOf(orbit: Orbit): Vec3 {
  const { yaw, pitch } = orbit;
  const flat = Math.cos(pitch);
  // Yaw about the world up (-y), then the pitch about the right axis. Written out rather
  // than composed from two matrices so the identity at 0/0 is visible in the source.
  const x = Math.sin(yaw) * flat;
  const y = -Math.sin(pitch);
  const z = -Math.cos(yaw) * flat;
  return unit({ x, y, z });
}

/**
 * The screen axes: `right` is the world up crossed with the view direction, and `up` is
 * `right × forward`. At yaw 0 and pitch 0 that is right `+x`, up `+y`, forward `-z` — the
 * 2D painter's own frame, y downwards, with the eye on the +z side and a third axis added.
 * It is deliberately the 2D camera's reference frame, not a graphics-convention one.
 */
export function basisOf(orbit: Orbit): Basis {
  const forward = forwardOf(orbit);
  const right = unit(cross(WORLD_UP, forward));
  return { right, up: cross(right, forward), forward, target: orbit.target, distance: orbit.distance };
}

/** The pixels one world unit covers at unit depth: the projection's scale. */
export function focalOf(orbit: Orbit, viewport: Viewport): number {
  return (viewport.height / 2) / Math.tan(orbit.fov / 2);
}

/**
 * `null` when the node is at or behind the eye plane, which is where the divide stops.
 * The centre is the viewport's middle, so the caller states it once per frame.
 */
export function project(basis: Basis, point: Vec3, focal: number, centre: Vec3): Projected | null {
  const { forward, right, up, target, distance } = basis;
  const dx = point.x - target.x;
  const dy = point.y - target.y;
  const dz = point.z - target.z;
  // A point at the target is exactly `distance` from the eye, which is the fit's own number.
  const depth = distance + (dx * forward.x + dy * forward.y + dz * forward.z);
  if (depth <= NEAR) return null;
  const scale = focal / depth;
  // `right` and `up` are already the screen's own directions, y downwards, so both dots are
  // added as they are: a positive world y lands below the centre, as the 2D painter has it.
  return {
    x: centre.x + (dx * right.x + dy * right.y + dz * right.z) * scale,
    y: centre.y + (dx * up.x + dy * up.y + dz * up.z) * scale,
    depth,
  };
}

/** A node's world radius as it is drawn: a nearer node covers more of the screen. */
export function radiusOnScreen(focal: number, worldRadius: number, depth: number): number {
  return (worldRadius * focal) / depth;
}

/**
 * The drag, as a hand sends it: right turns the drawing right, and dragging down rolls its
 * top edge away from the viewer. Both signs are the same as the 2D camera's — a drag moves
 * what is under the pointer the way the pointer moved — which is why the studio's nav gate
 * and this one agree about what a drag is.
 */
export function rotateBy(orbit: Orbit, dx: number, dy: number): Orbit {
  return {
    ...orbit,
    yaw: orbit.yaw + dx * ROTATION_PER_PX,
    pitch: clamp(orbit.pitch + dy * ROTATION_PER_PX, -PITCH_LIMIT, PITCH_LIMIT),
  };
}

/** The wheel: a notch in is nearer, and the distance stays inside this orbit's limits. */
export function dollyBy(orbit: Orbit, factor: number): Orbit {
  return { ...orbit, distance: clamp(orbit.distance / factor, orbit.limits.min, orbit.limits.max) };
}

/** A right-drag: the target slides across the screen plane, by the pixels the drag went. */
export function panBy(orbit: Orbit, delta: { x: number; y: number }, viewport: Viewport): Orbit {
  const basis = basisOf(orbit);
  // World units per screen pixel at the target's own depth: the inverse of the divide.
  const world = orbit.distance / focalOf(orbit, viewport);
  const { right, up } = basis;
  return {
    ...orbit,
    target: {
      x: orbit.target.x - (delta.x * world * right.x + delta.y * world * up.x),
      y: orbit.target.y - (delta.x * world * right.y + delta.y * world * up.y),
      z: orbit.target.z - (delta.x * world * right.z + delta.y * world * up.z),
    },
  };
}

/** The box the drawing occupies, or null when there are no nodes to bound. */
export function boxOf(x: Float32Array, y: Float32Array, z: Float32Array): Box3 | null {
  if (x.length === 0) return null;
  const min = { x: Infinity, y: Infinity, z: Infinity };
  const max = { x: -Infinity, y: -Infinity, z: -Infinity };
  for (let i = 0; i < x.length; i += 1) {
    const px = x[i] ?? 0;
    const py = y[i] ?? 0;
    const pz = z[i] ?? 0;
    if (px < min.x) min.x = px;
    if (px > max.x) max.x = px;
    if (py < min.y) min.y = py;
    if (py > max.y) max.y = py;
    if (pz < min.z) min.z = pz;
    if (pz > max.z) max.z = pz;
  }
  return { min, max };
}

function centreOf(box: Box3): Vec3 {
  return {
    x: (box.min.x + box.max.x) / 2,
    y: (box.min.y + box.max.y) / 2,
    z: (box.min.z + box.max.z) / 2,
  };
}

/** The half-diagonal: the distance from the centre to the furthest corner. */
function radiusOf(box: Box3): number {
  const dx = (box.max.x - box.min.x) / 2;
  const dy = (box.max.y - box.min.y) / 2;
  const dz = (box.max.z - box.min.z) / 2;
  return Math.sqrt(dx * dx + dy * dy + dz * dz);
}

/**
 * The orbit that frames a drawing whole: its centre, and far enough back to see all of it.
 *
 * The field of view, not the viewport, sets the distance, so the same drawing comes out the
 * same size whatever the window is: the viewport only chooses the focal length that turns
 * that angle into pixels (`focalOf`). That is why this takes no viewport.
 */
export function fitOrbit(box: Box3 | null): Orbit {
  if (box === null) return identityOrbit();
  const distance = Math.max(NEAR * 10, (radiusOf(box) * FIT_MARGIN) / Math.tan(DEFAULT_FOV / 2));
  return { yaw: 0, pitch: 0, distance, target: centreOf(box), fov: DEFAULT_FOV, limits: limitsFor(distance) };
}

/** The reset: the same drawing at the same distance, seen head on. */
export function resetTo(orbit: Orbit): Orbit {
  return { ...orbit, yaw: 0, pitch: 0 };
}
