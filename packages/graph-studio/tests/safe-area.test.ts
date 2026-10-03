/**
 * ST-4 and ST-10: the box the studio's panels leave visible, and the fit that uses it.
 *
 * The safe area is the pure half (`safeAreaOf`), driven here with the measured rects the
 * panels have; the fit is `fitCamera` over that box, so the camera this pins is the one
 * the studio hands the view.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Bounds, type Camera, type FitArea, type Viewport, fitCamera, worldToScreen } from "../../graph-render/src/camera.ts";
import { FIT_MARGIN } from "../../graph-render/src/look/presets.ts";
import type { Frame } from "../../graph-render/src/frame.ts";
import { EMPTY_FRAME } from "../../graph-render/src/scene.ts";
import { metaOf } from "../src/source/meta.ts";
import { initialState } from "../src/state/model.ts";
import { DEFAULT_SETTINGS, withFilter } from "../src/state/settings.ts";
import { type FitFace, fitResults } from "../src/studio/fitResults.ts";
import { safeAreaOf } from "../src/ui/safeArea.ts";
import { node } from "./support.ts";

const CANVAS: Viewport = { width: 800, height: 600 };
/** `.gs-dock`: 280px wide, 12px from the right edge of an 800px canvas. */
const DOCK: FitArea = { x: 508, y: 12, width: 280, height: 576 };
/** `.gs-left`: 260px wide, 12px from the left edge, down to the console. */
const LEFT: FitArea = { x: 12, y: 12, width: 260, height: 336 };
/** `.gs-console`: 720px wide, 240px tall, 12px off the bottom. */
const CONSOLE: FitArea = { x: 68, y: 348, width: 720, height: 240 };

/** The screen box a world box is drawn into under `camera`. */
function drawn(camera: Camera, world: Bounds): FitArea {
  const near = worldToScreen(camera, { x: world.minX, y: world.minY });
  const far = worldToScreen(camera, { x: world.maxX, y: world.maxY });
  return { x: near.x, y: near.y, width: far.x - near.x, height: far.y - near.y };
}

/** Whether `box` lies inside `area`, to the pixel: a fit pads, so it may sit a hair outside. */
function inside(area: FitArea, box: FitArea, slack = 0.5): boolean {
  return area.x - slack <= box.x && area.y - slack <= box.y
    && box.x + box.width <= area.x + area.width + slack
    && box.y + box.height <= area.y + area.height + slack;
}

test("with no panels the safe area is the whole canvas, so a fit is the one it always was", () => {
  assert.deepEqual(safeAreaOf(CANVAS, []), { x: 0, y: 0, width: 800, height: 600 });
  assert.deepEqual(safeAreaOf(CANVAS, [{ x: 0, y: 0, width: 0, height: 0 }]), { x: 0, y: 0, width: 800, height: 600 });
});

test("a dock on the right takes its 280px off the visible box", () => {
  assert.deepEqual(safeAreaOf(CANVAS, [DOCK]), { x: 0, y: 0, width: 508, height: 600 });
});

/** A 300×200 world box: a third of the width of the canvas it is fitted into. */
const BOUNDS: Bounds = { minX: 0, minY: 0, maxX: 300, maxY: 200 };

test("a fit centred in the safe box keeps the drawing out from under the dock", () => {
  const area = safeAreaOf(CANVAS, [DOCK]);
  assert.ok(area !== null);
  const camera = fitCamera(BOUNDS, CANVAS, { area });
  const centre = worldToScreen(camera, { x: 150, y: 100 });
  assert.ok(centre.x < 508, `the centre is at ${centre.x}, which is under the dock`);
  assert.ok(inside(area, drawn(camera, BOUNDS)), "the drawing runs past the box the dock leaves");
});

test("without the box the same fit runs a third of the drawing under the dock", () => {
  const area = safeAreaOf(CANVAS, [DOCK]);
  assert.ok(area !== null);
  assert.ok(!inside(area, drawn(fitCamera(BOUNDS, CANVAS), BOUNDS)),
    "a whole-canvas fit is what the safe box has to stop");
});

test("the left column and the console together shrink the box on both axes", () => {
  // The left column runs to x=272 and the console's top is y=348; the console stops 12px short
  // of the right edge, so the free box is 528 wide, not 456.
  assert.deepEqual(safeAreaOf(CANVAS, [LEFT, CONSOLE]), { x: 272, y: 0, width: 528, height: 348 });
});

test("panels that leave nothing free give the canvas back rather than a broken box", () => {
  assert.equal(safeAreaOf(CANVAS, [{ x: 0, y: 0, width: 800, height: 600 }]), null);
});

const NODES = [
  node("a", { label: "Target one" }),
  node("b", { label: "Target two" }),
  node("c", { label: "Other" }),
];
const META = metaOf(NODES, ["a", "b", "c"], { source: Uint32Array.of(0, 1), target: Uint32Array.of(1, 2) });

/** Three discs a few units apart: a fit wants far more room than the canvas has. */
function frameOf(x: readonly number[]): Frame {
  return {
    ...EMPTY_FRAME, nodeCount: 3, edgeCount: 2,
    x: Float32Array.from(x), y: Float32Array.from([0, 0, 0]), r: Float32Array.of(4, 4, 4),
  };
}

interface Desk {
  readonly view: FitFace;
  readonly cameras: Camera[];
  readonly message: () => string;
}

/** A view that records the cameras it is given and answers the two a fit asks for. */
function desk(x: readonly number[], area: FitArea | null = null): Desk {
  const cameras: Camera[] = [];
  const view: FitFace = {
    frame: () => frameOf(x),
    setCamera: (camera) => void cameras.push(camera),
    viewport: () => CANVAS,
    limits: () => ({ min: 0.02, max: 40 }),
    ...(area === null ? {} : { safeArea: () => area }),
  };
  const state = { ...initialState(withFilter(DEFAULT_SETTINGS, { text: "target" })), meta: META };
  return { view, cameras, message: () => fitResults(view, state).message };
}

test("two results fit at the renderer's ceiling, not at limits.max", () => {
  const made = desk([0, 10, 400]);
  assert.equal(made.message(), "fitted to 2 of 3 nodes");
  assert.equal(made.cameras.at(0)?.scale, 2, "a fit of two close nodes stops at 2, as the ⤢ button does");
});

test("the fit is centred in the safe area the studio measured", () => {
  const made = desk([0, 10, 400], { x: 0, y: 0, width: 508, height: 600 });
  made.message();
  const camera = made.cameras.at(0);
  assert.ok(camera !== undefined);
  const matched = worldToScreen(camera, { x: 5, y: 0 });
  assert.ok(matched.x < 508, `the matched pair is drawn at ${matched.x}, under the dock`);
});

test("a view that reports no safe area is fitted over the whole canvas, as before", () => {
  const made = desk([0, 10, 400]);
  made.message();
  // The frame's two matched discs at x=0 and x=10, radius 4: world 0..14 by -4..4.
  const wanted = fitCamera({ minX: -4, minY: -4, maxX: 14, maxY: 4 }, CANVAS, { margin: FIT_MARGIN });
  assert.deepEqual(made.cameras.at(0), wanted);
});

test("a non-finite coordinate is refused, not fitted", () => {
  const made = desk([0, Number.NaN, 400]);
  const message = made.message();
  assert.doesNotMatch(message, /fitted to/, "a NaN camera is not a success");
  assert.deepEqual(made.cameras, [], "no camera is set for a box that has no numbers in it");
  assert.match(message, /\S/);
});
