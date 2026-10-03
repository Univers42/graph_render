/**
 * The camera matrix the WebGL2 3D path draws with (`three/matrix.ts`), against `project()`,
 * the function the Canvas2D painter draws with: the two painters must put a node on the same
 * pixel, or the GL path is a different picture rather than a faster one. And the late
 * projection (`three/projection.ts`): a frame handed over is not projected until it is read,
 * which is what lets the GL path orbit without touching a column.
 *
 * The negative control is the last test: a matrix of a different yaw lands a node elsewhere,
 * so the agreement above is not two numbers that agree with anything.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Viewport } from "../src/camera.ts";
import { type Orbit, type Vec3, basisOf, fitOrbit, focalOf, project, radiusOnScreen } from "../src/three/orbit.ts";
import { cameraMatrix } from "../src/three/matrix.ts";
import { type Projection, newProjection, projectFrame } from "../src/three/projection.ts";
import { lineFrame } from "./support.ts";

const VIEWPORT: Viewport = { width: 1280, height: 800 };
const CENTRE: Vec3 = { x: 640, y: 400, z: 0 };
const POINTS: readonly Vec3[] = [
  { x: 0, y: 0, z: 0 }, { x: 900, y: -300, z: 120 }, { x: -450, y: 610, z: -800 }, { x: 33.5, y: 1000, z: 500 },
];

function orbitOf(patch: Partial<Orbit>): Orbit {
  return { ...fitOrbit({ min: { x: -1000, y: -1000, z: -1000 }, max: { x: 1000, y: 1000, z: 1000 } }), ...patch };
}

/** `M · (p, 1)`, column-major, in f64 over the matrix's own f32 entries. */
function times(m: Float32Array, p: Vec3): readonly [number, number, number, number] {
  const row = (r: number): number => (m[r] ?? 0) * p.x + (m[4 + r] ?? 0) * p.y + (m[8 + r] ?? 0) * p.z + (m[12 + r] ?? 0);
  return [row(0), row(1), row(2), row(3)];
}

function close(got: number, want: number, within: number, name: string): void {
  assert.ok(Math.abs(got - want) <= within, `${name}: ${got} against ${want} (within ${within})`);
}

for (const [name, patch] of [
  ["head on", {}], ["turned", { yaw: 0.7, pitch: -0.4 }], ["pulled in", { yaw: -2.1, pitch: 1.2, distance: 900 }],
  ["off-centre target", { yaw: 0.3, target: { x: 250, y: -125, z: 60 } }],
] as const) {
  test(`the matrix puts a node where project() does: ${name}`, () => {
    const orbit = orbitOf(patch);
    const matrix = cameraMatrix(orbit, VIEWPORT);
    const focal = focalOf(orbit, VIEWPORT);
    for (const point of POINTS) {
      const want = project(basisOf(orbit), point, focal, CENTRE);
      if (want === null) continue;
      const [x, y, z, w] = times(matrix, point);
      close(w, want.depth, 1e-3, "depth");
      close(x / w, want.x, 1e-2, "screen x");
      close(y / w, want.y, 1e-2, "screen y");
      close((z / w) * 10, radiusOnScreen(focal, 10, want.depth), 1e-3, "radius of 10 units");
    }
  });
}

test("a point behind the eye has a depth at or below zero, as project() drops it", () => {
  const orbit = orbitOf({ distance: 100 });
  const behind = { x: 0, y: 0, z: 500 };
  assert.equal(project(basisOf(orbit), behind, focalOf(orbit, VIEWPORT), CENTRE), null);
  assert.ok(times(cameraMatrix(orbit, VIEWPORT), behind)[3] <= 0);
});

test("a frame handed over is not projected until a column is read", () => {
  const frame = lineFrame({ x: [-100, 100], y: [0, 0], z: [0, 50], edges: [[0, 1]] });
  let reads = 0;
  const orbit = orbitOf({});
  const wanted: Projection = {
    frame, x: frame.x, y: frame.y, extent: new Float32Array([5, 5]), viewport: VIEWPORT,
    get orbit() { reads += 1; return orbit; },
  };
  const drawn = projectFrame(newProjection(2), wanted);
  assert.equal(drawn.wanted, wanted, "the hand-over keeps what it was given");
  assert.equal(reads, 0, "the hand-over projected nothing");
  assert.ok((drawn.depth[1] ?? 0) > 0);
  const projected = reads;
  assert.ok(projected > 0, "the first read projected");
  void drawn.x;
  void drawn.order;
  assert.equal(reads, projected, "a second read reuses the projection");
  assert.equal(projectFrame(drawn, wanted), drawn, "the next hand-over reuses the same value");
  assert.equal(reads, projected, "and projects nothing either");
});

test("negative control: a matrix of another yaw puts the node elsewhere", () => {
  const point = POINTS[1] ?? { x: 0, y: 0, z: 0 };
  const orbit = orbitOf({ yaw: 0.7 });
  const want = project(basisOf(orbit), point, focalOf(orbit, VIEWPORT), CENTRE);
  assert.ok(want !== null);
  const [x, , , w] = times(cameraMatrix({ ...orbit, yaw: 0.8 }, VIEWPORT), point);
  assert.ok(Math.abs(x / w - want.x) > 1, `another yaw moved x by ${Math.abs(x / w - want.x)} px`);
});
