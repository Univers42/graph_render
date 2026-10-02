/**
 * The 3D camera, on its own numbers: a projection, a rotation, a dolly and a fit. These
 * are the unit tests the studio-3d browser gate stands on — if a drag does not move a
 * projected node, one of these has moved under it.
 *
 * The negative control is the last test: a yaw of zero and a flat z column must project
 * exactly as the 2D camera would, or the 3D path is not a superset of the 2D one.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Viewport } from "../src/camera.ts";
import {
  DEFAULT_FOV, NEAR, PITCH_LIMIT, type Orbit, basisOf, boxOf, dollyBy, fitOrbit, focalOf,
  panBy, project, radiusOnScreen, resetTo, rotateBy,
} from "../src/three/orbit.ts";

const VIEWPORT: Viewport = { width: 800, height: 600 };
const CENTRE = { x: 400, y: 300, z: 0 };

function orbitAt(patch: Partial<Orbit> = {}): Orbit {
  return { ...fitOrbit(null), ...patch };
}

function at(orbit: Orbit, point: { x: number; y: number; z: number }): { x: number; y: number; depth: number } {
  const found = project(basisOf(orbit), point, focalOf(orbit, VIEWPORT), CENTRE);
  assert.ok(found !== null, "the point is in front of the eye");
  return found;
}

test("the target lands in the middle of the viewport", () => {
  const orbit = orbitAt({ target: { x: 10, y: -20, z: 30 }, distance: 500 });
  const centre = at(orbit, { x: 10, y: -20, z: 30 });
  assert.ok(Math.abs(centre.x - 400) < 1e-9, `x ${centre.x}`);
  assert.ok(Math.abs(centre.y - 300) < 1e-9, `y ${centre.y}`);
  assert.ok(Math.abs(centre.depth - 500) < 1e-6, `depth ${centre.depth}`);
});

test("a yaw of zero and a flat z column project as the 2D camera does", () => {
  // The negative control for the whole module: if this fails, the 3D path is not the 2D
  // path with a third column, and every 2D guarantee this renderer makes is void.
  const orbit = orbitAt({ target: { x: 0, y: 0, z: 0 }, distance: 200 });
  // The 2D painter's own transform is `screen = world * scale + offset` with y downwards
  // (`camera.ts:49`), and the scale a 3D node at the target's depth gets is focal / depth.
  const scale = focalOf(orbit, VIEWPORT) / 200;
  const cases: readonly (readonly [number, number])[] = [[-100, 0], [0, 0], [100, 50], [40, -80]];
  for (const [x, y] of cases) {
    const found = at(orbit, { x, y, z: 0 });
    assert.ok(Math.abs(found.x - (400 + x * scale)) < 1e-6, `x ${found.x} for ${x}`);
    assert.ok(Math.abs(found.y - (300 + y * scale)) < 1e-6, `y ${found.y} for ${y}`);
  }
});

test("y grows downwards, as the 2D painter draws it", () => {
  const orbit = orbitAt({ target: { x: 0, y: 0, z: 0 }, distance: 200 });
  assert.ok(at(orbit, { x: 0, y: 100, z: 0 }).y > at(orbit, { x: 0, y: 0, z: 0 }).y);
});

test("a drag turns the drawing, and the node under the cursor's side of it moves", () => {
  const orbit = orbitAt({ target: { x: 0, y: 0, z: 0 }, distance: 200 });
  const node = { x: 100, y: 0, z: 0 };
  const before = at(orbit, node);
  const turned = rotateBy(orbit, 200, 0);
  const after = at(turned, node);
  assert.ok(Math.abs(turned.yaw - orbit.yaw) > 1, "the yaw moved");
  assert.ok(Math.abs(after.x - before.x) > 1, `a 200 px drag moved x by ${Math.abs(after.x - before.x)} px`);
  // The target itself is still the middle: turning the camera does not move what it looks at.
  assert.ok(Math.abs(at(turned, orbit.target).x - 400) < 1e-6);
});

test("a vertical drag tips the drawing and a node off the axis changes depth", () => {
  const orbit = orbitAt({ target: { x: 0, y: 0, z: 0 }, distance: 200 });
  const node = { x: 0, y: 0, z: 100 };
  const before = at(orbit, node);
  const tipped = rotateBy(orbit, 0, 150);
  const after = at(tipped, node);
  assert.ok(tipped.pitch > orbit.pitch, "the pitch moved");
  assert.ok(Math.abs(after.depth - before.depth) > 1, `depth moved by ${after.depth - before.depth}`);
});

test("the pitch stops short of the poles", () => {
  const orbit = orbitAt();
  assert.equal(rotateBy(orbit, 0, 1e6).pitch, PITCH_LIMIT);
  assert.equal(rotateBy(orbit, 0, -1e6).pitch, -PITCH_LIMIT);
});

test("a node behind the eye is dropped rather than clamped", () => {
  // The one thing this projection cannot do, and it is deliberate: past the eye plane a
  // divide would throw the node across the viewport instead of leaving the drawing. The eye
  // is at target + distance * z, so `z` past that is behind it.
  const orbit = orbitAt({ target: { x: 0, y: 0, z: 0 }, distance: 100 });
  const basis = basisOf(orbit);
  const focal = focalOf(orbit, VIEWPORT);
  assert.equal(project(basis, { x: 0, y: 0, z: 400 }, focal, CENTRE), null, "z 400 is behind a 100 orbit");
  // A node a hair in front of the same plane is drawn, so the cut is at the plane.
  assert.ok(project(basis, { x: 0, y: 0, z: 100 - NEAR * 2 }, focal, CENTRE) !== null);
  assert.equal(project(basis, { x: 0, y: 0, z: 100 + NEAR * 2 }, focal, CENTRE), null);
});

test("the wheel dollies, and the distance stays inside the orbit's own limits", () => {
  const orbit = fitOrbit(boxOf(Float32Array.of(-50, 50), Float32Array.of(-50, 50), Float32Array.of(0, 0)));
  const nearer = dollyBy(orbit, 1.25);
  assert.ok(nearer.distance < orbit.distance, "a notch in is nearer");
  // A notch out and back is the same distance to within a float: the clamp is not in the way
  // at this depth, so this is the divide, not a rounded one.
  assert.ok(Math.abs(dollyBy(nearer, 1 / 1.25).distance - orbit.distance) < 1e-9);
  let zoomed = orbit;
  for (let step = 0; step < 500; step += 1) zoomed = dollyBy(zoomed, 1.25);
  assert.equal(zoomed.distance, orbit.limits.min, "the floor holds");
  let out = orbit;
  for (let step = 0; step < 500; step += 1) out = dollyBy(out, 1 / 1.25);
  assert.equal(out.distance, orbit.limits.max, "the ceiling holds");
});

test("a right-drag moves the drawing the way it was dragged, across the screen plane", () => {
  // The world point under the cursor stays under the cursor, which is what a pan is for —
  // and that is the 2D camera's own promise (`zoomAt` anchoring, navrows.py's anchor rows),
  // so a 3D pan that did the opposite would be a different gesture wearing the same name.
  const orbit = orbitAt({ target: { x: 0, y: 0, z: 0 }, distance: 200 });
  const node = { x: 0, y: 0, z: 0 };
  const before = at(orbit, node);
  const after = at(panBy(orbit, { x: 100, y: 0 }, VIEWPORT), node);
  assert.ok(Math.abs(after.x - before.x - 100) < 1e-6, `the drag went 100 px, the node went ${after.x - before.x}`);
  assert.ok(Math.abs(after.depth - before.depth) < 1e-6, "a pan does not change the depth");
  // A vertical drag pans the same way, so the camera is not a one-axis affordance.
  const up = at(panBy(orbit, { x: 0, y: 60 }, VIEWPORT), node);
  assert.ok(Math.abs(up.y - before.y - 60) < 1e-6, `y went ${up.y - before.y}`);
});

test("the fit frames a drawing whole, and the reset is the same drawing head on", () => {
  const box = boxOf(Float32Array.of(-100, 100), Float32Array.of(-100, 100), Float32Array.of(-100, 100));
  assert.ok(box !== null);
  const orbit = fitOrbit(box);
  assert.deepEqual(orbit.target, { x: 0, y: 0, z: 0 }, "the target is the centre of the box");
  // Every corner is on screen, with the margin the fit leaves.
  for (const x of [-100, 100]) {
    for (const y of [-100, 100]) {
      for (const z of [-100, 100]) {
        const found = at(orbit, { x, y, z });
        assert.ok(found.x > 0 && found.x < VIEWPORT.width, `x ${found.x}`);
        assert.ok(found.y > 0 && found.y < VIEWPORT.height, `y ${found.y}`);
      }
    }
  }
  const turned = rotateBy(orbit, 400, 90);
  const reset = resetTo(turned);
  assert.deepEqual([reset.yaw, reset.pitch], [0, 0]);
  assert.equal(reset.distance, turned.distance, "a reset is not a zoom");
  assert.deepEqual(reset.target, turned.target, "and it does not lose the target");
});

test("a fit of no drawing is an identity orbit, and a box of no nodes is null", () => {
  assert.equal(boxOf(new Float32Array(0), new Float32Array(0), new Float32Array(0)), null);
  assert.equal(fitOrbit(null).distance, 1);
});

test("a node's drawn radius grows as it comes nearer", () => {
  const focal = focalOf(orbitAt(), VIEWPORT);
  const near = radiusOnScreen(focal, 10, 100);
  const far = radiusOnScreen(focal, 10, 400);
  assert.ok(near > far, `${near} should exceed ${far}`);
  // The radius goes as 1/depth, so a node four times nearer draws four times as wide.
  assert.ok(Math.abs(near / far - 4) < 1e-9, `the ratio is the depth ratio, not ${near / far}`);
});

test("the field of view is the studio's own and is not the 2D camera's scale", () => {
  assert.equal(orbitAt().fov, DEFAULT_FOV);
});
