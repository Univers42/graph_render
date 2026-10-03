/**
 * The orbit camera as one 4x4 matrix, for a GPU that projects on its own: `project()` in
 * `orbit.ts`, written so a vertex shader can multiply instead of calling it.
 *
 * For `v = M · (x, y, z, 1)`: `v.x / v.w` and `v.y / v.w` are the screen point in CSS pixels,
 * `v.w` is the depth, and `v.z / v.w` is the pixels one world unit covers at that depth (the
 * focal length rides in the matrix as a constant `v.z`). So a node's screen point, its depth and
 * its drawn radius all come out of one multiply, and the camera is one uniform.
 *
 * Column-major, as `uniformMatrix4fv` reads it with `transpose = false`.
 *
 * Caveat: the matrix is f32 and `project()` is f64, and the target is folded into the
 * translation column, so a drawing far from the origin loses the low bits there: the screen
 * error is about |p|·2^-24·focal/depth pixels, a hundredth of a pixel for a drawing a thousand
 * units out. `tests/three-matrix.test.ts` holds it to that against `project()`.
 */
import type { Viewport } from "../camera.ts";
import { type Orbit, type Vec3, basisOf, focalOf } from "./orbit.ts";

function dot(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** `scale · axis + offset · forward`, the row that puts a screen axis over the depth. */
function blended(axis: Vec3, scale: number, offset: number, forward: Vec3): Vec3 {
  return { x: scale * axis.x + offset * forward.x, y: scale * axis.y + offset * forward.y, z: scale * axis.z + offset * forward.z };
}

/** One row of the matrix, written into its column-major slots. */
function setRow(out: Float32Array, row: number, axis: Vec3, translation: number): void {
  out[row] = axis.x;
  out[4 + row] = axis.y;
  out[8 + row] = axis.z;
  out[12 + row] = translation;
}

/** The camera of `orbit` over `viewport`, written into `out` (16 floats) and returned. */
export function cameraMatrix(orbit: Orbit, viewport: Viewport, out = new Float32Array(16)): Float32Array {
  const { right, up, forward, target, distance } = basisOf(orbit);
  const focal = focalOf(orbit, viewport);
  const cx = viewport.width / 2;
  const cy = viewport.height / 2;
  // depth = distance + forward·(p - target); screen·depth = centre·depth + focal·axis·(p - target).
  const across = blended(right, focal, cx, forward);
  const down = blended(up, focal, cy, forward);
  setRow(out, 0, across, cx * distance - dot(across, target));
  setRow(out, 1, down, cy * distance - dot(down, target));
  setRow(out, 2, { x: 0, y: 0, z: 0 }, focal);
  setRow(out, 3, forward, distance - dot(forward, target));
  return out;
}
