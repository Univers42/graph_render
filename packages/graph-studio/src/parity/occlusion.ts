/**
 * The occlusion stage of the label pipeline: a node hidden behind another one's glyph is
 * not a label candidate at all, and the declutter never sees it. The ray, the origin
 * offset, the nearest-hit rule and the tolerance are the source's own.
 *   SciGraphs/core/visualization/text_overlay.py:250-299 (test_depth_occlusion)
 *   harness/scigraphs_lesmis_camera.py:195-227 (the port this follows, line for line)
 */
import type { Fixture, FixtureParams } from "./fixture.ts";

/** The origin is pulled this far along the ray, so a node cannot meet its own glyph. */
const RAY_ORIGIN_OFFSET = 0.01;

/** Below this the tolerance is the constant, not the glyph's (text_overlay.py:290-292). */
const TOLERANCE_FLOOR = 0.1;

/** The tolerance is this many glyph radii, so one fat glyph does not hide its neighbour. */
const TOLERANCE_FACTOR = 1.5;

type Vec = readonly [number, number, number];

function sub(a: Vec, b: Vec): Vec {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

function dot(a: Vec, b: Vec): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function norm(a: Vec): number {
  return Math.sqrt(dot(a, a));
}

/** The centre of the cloud's box, the `centre` of executor.py:606-620. */
function centreOf(params: FixtureParams): Vec {
  const lo = params.bbox_lo;
  const hi = params.bbox_hi;
  return [
    (lo[0] + hi[0]) / 2,
    (lo[1] + hi[1]) / 2,
    (lo[2] + hi[2]) / 2,
  ];
}

/** The camera's own position: the centre, pushed back along the forward axis. */
export function cameraLocation(params: FixtureParams): Vec {
  const centre = centreOf(params);
  const forward = params.camera_forward;
  const back = params.camera_distance;
  return [
    centre[0] + forward[0] * back,
    centre[1] + forward[1] * back,
    centre[2] + forward[2] * back,
  ];
}

/** The unit ray from the camera to a world point, and the point's distance from it. */
function aimAt(point: Vec, location: Vec): { direction: Vec; distance: number } {
  const aim = sub(point, location);
  const distance = norm(aim);
  const length = distance > 0 ? distance : 1;
  return { direction: [aim[0] / length, aim[1] / length, aim[2] / length], distance };
}

/** The nearest glyph sphere on the ray, or null when the ray misses every one. */
function nearestHit(origin: Vec, direction: Vec, glyphs: readonly Vec[], radius: number): number | null {
  let best: number | null = null;
  for (const glyph of glyphs) {
    const offset = sub(glyph, origin);
    const along = dot(offset, direction);
    const disc = along * along - (dot(offset, offset) - radius * radius);
    if (disc < 0) continue;
    const near = along - Math.sqrt(disc);
    if (near <= 0) continue;
    if (best === null || near < best) best = near;
  }
  return best;
}

/**
 * 1 for a node that may carry a label, 0 for one hidden behind a glyph.
 *
 * Ponytail: the source casts against the evaluated mesh — icosphere facets and the bezier
 * edge curves (fixture.md, "The occlusion raycast is against spheres"); this casts against
 * one sphere per node, so it can only under-count the occluded and never invent one. The
 * escape hatch is to pass the edge geometry's own samples as extra glyphs.
 */
export function candidatesOf(given: Fixture): Uint8Array {
  const location = cameraLocation(given.params);
  const radius = given.radii.node;
  const tolerance = Math.max(TOLERANCE_FLOOR, radius * TOLERANCE_FACTOR);
  const glyphs = given.nodes.map((node) => node.world);
  const mask = new Uint8Array(given.nodes.length);
  given.nodes.forEach((node, at) => {
    const { direction, distance } = aimAt(node.world, location);
    const origin: Vec = [
      location[0] + direction[0] * RAY_ORIGIN_OFFSET,
      location[1] + direction[1] * RAY_ORIGIN_OFFSET,
      location[2] + direction[2] * RAY_ORIGIN_OFFSET,
    ];
    const hit = nearestHit(origin, direction, glyphs, radius);
    mask[at] = hit !== null && hit < distance - tolerance ? 0 : 1;
  });
  return mask;
}
