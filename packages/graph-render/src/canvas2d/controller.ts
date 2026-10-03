/** The view's state changes: everything `createView` does to a canvas between frames. */
import {
  type Camera, type FitArea, type Point, fitCamera, screenToWorld,
} from "../camera.ts";
import { type LiveDrag, movedScene } from "../drag.ts";
import type { Frame } from "../frame.ts";
import { DEFAULT_POLICY, type LabelPolicy, newLabelPlan, occupancyFor } from "../labels.ts";
import type { LocalLayer } from "../local.ts";
import { EMPTY_FRAME, pickEased, pickIn, sceneOf } from "../scene.ts";
import { plainStyle } from "../style.ts";
import { DARK_THEME, type Theme } from "../theme.ts";
import { type Orbit, boxOf, fitOrbit } from "../three/orbit.ts";
import type { BackendChoice } from "../webgl2/plan.ts";
import { newBulkSlot } from "../webgl2/hook.ts";
import { setSelection } from "./choose.ts";
import { newCounts } from "./input.ts";
import { type LoopState, invalidate, markMoved, relight } from "./loop.ts";
import { currentLimits, safeOf } from "./limits.ts";
import { MIN_SCREEN_RADIUS } from "./nodes.ts";
import { newPace } from "./pace.ts";
import { newRate } from "./rate.ts";
import { createSpriteCache } from "./sprites.ts";
import type { SpriteSurface } from "./surface.ts";

export interface Notify {
  hover(node: number): void;
  select(node: number): void;
  selection(nodes: readonly number[]): void;
  context(node: number, at: Point): void;
  camera(camera: Camera): void;
}

export interface Controller {
  readonly canvas: HTMLCanvasElement;
  readonly state: LoopState;
  readonly notify: Notify;
  /** True until the user moves the camera: a resize then re-fits instead of cropping. */
  fitted: boolean;
  /**
   * True once the user has touched the canvas: a pan, a zoom, a drag, a box select, a click or a
   * context menu. From then on a safe-area change only refreshes the limits; see `setSafeArea`.
   */
  gestured: boolean;
  /** The local graph, when one is shown: a fit frames it and not the whole graph. */
  readonly local: LocalLayer;
  /** The motor's live session, when the host gave one; a drag goes to it while it is enabled. */
  readonly live?: LiveDrag;
}

export interface Setup {
  readonly theme: Theme | undefined;
  readonly policy: LabelPolicy | undefined;
  readonly onFrame: () => void;
  /** Default "auto". */
  readonly backend?: BackendChoice | undefined;
}

const MAX_DPR = 2;
const PICK_TOLERANCE = 4;

function spriteSurface(): SpriteSurface | null {
  const canvas = new OffscreenCanvas(1, 1);
  const ctx = canvas.getContext("2d");
  if (ctx === null) return null;
  return {
    image: canvas,
    ctx,
    // The impostor spheres arrive as raw RGBA; the ImageData constructor is the host's.
    pixels: { putPixels: (data, width, height) => ctx.putImageData(new ImageData(data, width, height), 0, 0) },
    resize: (width, height) => {
      canvas.width = Math.max(1, width);
      canvas.height = Math.max(1, height);
    },
  };
}

export function newState(canvas: HTMLCanvasElement, setup: Setup): LoopState {
  const ctx = canvas.getContext("2d", { alpha: false });
  if (ctx === null) throw new Error("graph-render: the canvas gave no 2D context; it may already hold another kind");
  const theme = setup.theme ?? DARK_THEME;
  const policy = setup.policy ?? DEFAULT_POLICY;
  const scene = sceneOf(EMPTY_FRAME, plainStyle(0), null);
  const viewport = { width: 1, height: 1 };
  return {
    ctx, sprites: createSpriteCache(spriteSurface, theme), onFrame: setup.onFrame, theme, policy, scene,
    camera: fitCamera(null, viewport), get limits() { return currentLimits(this); }, viewport, safe: null, dpr: 1,
    x: scene.frame.x, y: scene.frame.y, fromX: scene.frame.x, fromY: scene.frame.y, transitionStart: -1,
    lit: new Uint8Array(0), hovered: -1, dimStart: -1, selected: -1, selection: [], pinned: [], marquee: null,
    plan: newLabelPlan(policy.budget), orbit: null, drawn: null,
    layoutKey: null, layoutDirty: false, layoutRuns: 0, occupancy: occupancyFor(viewport),
    scheduled: 0, settleTimer: null, movedAt: 0, destroyed: false,
    counts: newCounts(), frameMs: 0, frames: 0, rate: newRate(), pace: newPace(),
    bulk: newBulkSlot(setup.backend ?? "auto"),
  };
}

/** Reads the canvas's CSS box and sizes its backing store to it. */
export function measure(controller: Controller): void {
  const { canvas, state } = controller;
  const box = canvas.getBoundingClientRect();
  state.dpr = Math.min(MAX_DPR, globalThis.devicePixelRatio || 1);
  state.viewport = { width: Math.max(1, box.width), height: Math.max(1, box.height) };
  canvas.width = Math.round(state.viewport.width * state.dpr);
  canvas.height = Math.round(state.viewport.height * state.dpr);
  state.occupancy = occupancyFor(state.viewport);
  state.sprites.reset(state.theme, state.dpr);
}

/**
 * The area a host declared free of chrome, in canvas pixels; `null` (the default) is the whole
 * canvas. It is clamped to the canvas rather than trusted: a host that hands over a stale or
 * inverted box gets the canvas.
 *
 * WHY it moves the camera only while the camera is still automatic: the chrome resizes for reasons
 * that have nothing to do with the drawing — a selection filling the Inspector is the one that
 * mattered — and re-fitting on each of those took whatever the user was pointing at out from under
 * the pointer. Until the first gesture the camera is the view's own and a new free box is fitted
 * into; after one it is the user's, and the area only bounds how far the drawing may be panned. A
 * layout arriving is a new drawing, and `sceneApi.setFrame` still fits it.
 */
export function setSafeArea(controller: Controller, area: FitArea | null): void {
  const { state } = controller;
  const next = area !== null && Number.isFinite(area.x) && Number.isFinite(area.y)
    && area.width > 0 && area.height > 0 ? area : null;
  const same = (next?.x === state.safe?.x && next?.y === state.safe?.y
    && next?.width === state.safe?.width && next?.height === state.safe?.height) || (next === null && state.safe === null);
  if (same) return;
  state.safe = next;
  if (controller.fitted && !controller.gestured) fit(controller);
  else invalidate(state);
}

export function moveTo(controller: Controller, camera: Camera, byFit: boolean): void {
  controller.fitted = byFit;
  controller.state.camera = camera;
  markMoved(controller.state);
  controller.notify.camera(camera);
}

/** The single commit point for the 3D camera, as `moveTo` is for the 2D one. */
export function moveOrbit(controller: Controller, orbit: Orbit): void {
  controller.fitted = false;
  controller.state.orbit = orbit;
  markMoved(controller.state);
}

/** The orbit that frames the drawing a 3D frame holds; null when the frame has no z column. */
function fittedOrbit(state: LoopState): Orbit | null {
  const { frame } = state.scene;
  if (frame.z === null) return null;
  return fitOrbit(boxOf(frame.x, frame.y, frame.z));
}

export function fit(controller: Controller): void {
  const { state } = controller;
  const orbit = fittedOrbit(state);
  if (orbit !== null) {
    moveOrbit(controller, orbit);
    return;
  }
  // The safe area, not the canvas: the panels a host lays over the canvas are not drawing space,
  // and a fit that ignores them puts part of the drawing where the user cannot see it.
  const area = safeOf(state, state.viewport);
  const bounds = controller.local.bounds ?? state.scene.bounds;
  moveTo(controller, fitCamera(bounds, state.viewport, { area: area ?? undefined }), true);
}

export function hover(controller: Controller, node: number): void {
  const { state } = controller;
  if (node === state.hovered) return;
  state.hovered = node;
  controller.canvas.style.cursor = node >= 0 ? "pointer" : "grab";
  relight(state);
  invalidate(state);
  controller.notify.hover(node);
}

export function select(controller: Controller, node: number): void {
  setSelection(controller, node >= 0 ? [node] : []);
}

/** Starts the move to `frame` from wherever the nodes are drawn now. */
function startTransition(state: LoopState, frame: Frame): void {
  state.fromX = state.x.slice();
  state.fromY = state.y.slice();
  state.x = state.fromX.slice();
  state.y = state.fromY.slice();
  state.transitionStart = performance.now();
  state.scene = sceneOf(frame, state.scene.style, null);
}

export function showFrame(state: LoopState, frame: Frame, animate: boolean): void {
  const resized = frame.nodeCount !== state.scene.frame.nodeCount;
  if (animate && !resized && frame.nodeCount > 0) {
    startTransition(state, frame);
  } else {
    state.scene = sceneOf(frame, state.scene.style, null);
    state.transitionStart = -1;
    // Nothing is easing, so nothing is there to mix: the layer reads `u_eased` of 1 and a pick
    // reads the grid. A tween cut short by a snap or a resize has to leave both.
    state.bulk.tween = null;
    state.x = frame.x;
    state.y = frame.y;
  }
  // A 3D frame gets a fresh orbit, because the z column changed what "in view" means: the
  // old angles are about a drawing that is no longer the one on screen. A 2D frame leaves
  // the orbit null, and the 2D camera is untouched by any of this.
  state.orbit = fittedOrbit(state);
  // The projection belongs to the orbit that made it: a new frame invalidates it, and a
  // camera change does not, because the loop re-projects into it every frame either way.
  state.drawn = null;
  if (resized) {
    state.lit = new Uint8Array(frame.nodeCount);
    state.hovered = -1;
    state.selected = -1;
    state.selection = [];
    state.pinned = [];
  }
  relight(state);
}

/**
 * New positions for the nodes already in the frame, from a live simulation. A pair that does
 * not have one entry per node is another graph's drawing and is ignored.
 *
 * The columns are adopted, not copied. The motor hands its buffers over on a transfer list
 * (`motor/liveLoop.ts`), which detaches them in the worker, so the page holds the only copy and
 * nothing else can be writing it; a `state.x.set(xs)` on top of that would be 8 MB of copying
 * per frame at a million nodes, for an array already the right length and the right precision.
 * The worker narrows to f32 before it sends (`motor/protocol.ts`), so the columns arrive in the
 * precision both the drawing and the GPU attribute want.
 *
 * A batch carrying a non-finite coordinate is refused whole, the way `snapshot/decode.ts:116`
 * refuses one: a NaN is a motor bug, and half a batch of them would draw a sprite, a pick-grid
 * entry and a NaN camera, none of which the user can undo. One bad coordinate is a bug report,
 * not a drawing. It is the one pass over the batch this function still makes, and it is the
 * one that keeps a broken frame off the screen.
 */
export function setPositions(state: LoopState, xs: Float32Array, ys: Float32Array): void {
  const count = state.scene.frame.nodeCount;
  if (xs.length !== count || ys.length !== count) return;
  for (let node = 0; node < count; node += 1) {
    if (Number.isFinite(xs[node]) && Number.isFinite(ys[node])) continue;
    return;
  }
  state.x = xs;
  state.y = ys;
  state.bulk.placed += 1;
  // The scene is the single source of truth: it carries the frame, and the bounds and the pick
  // grid are rebuilt on it only when something reads them (`src/lazy.ts`).
  state.scene = movedScene(state.scene, { x: xs, y: ys });
  markMoved(state);
}

/**
 * The node under a screen point.
 *
 * Mid-tween this scans the eased pose rather than asking the grid, because the grid indexes
 * the *target* frame and every node is somewhere else until the tween ends. The scan reads the
 * two halves and the fraction the loop already publishes for the shader, so it is the pose
 * both backends draw; outside a tween the grid still answers, and it is O(cells) not O(nodes).
 */
export function pickAt(state: LoopState, at: Point): number {
  if (state.orbit !== null) return pickInSpace(state, at);
  const world = screenToWorld(state.camera, at);
  const { scale } = state.camera;
  const query = {
    x: world.x, y: world.y, tolerance: PICK_TOLERANCE / scale, floor: MIN_SCREEN_RADIUS / scale,
  };
  const tween = state.bulk.tween;
  return tween === null ? pickIn(state.scene, query) : pickEased(state.scene, query, tween);
}

/**
 * The 3D hit test: the nearest node whose drawn disc is under the point. There is no
 * un-projection here, and there does not need to be — the screen points are already what the
 * painter drew, so the same radii and the same tolerance pick the same node a hand would.
 * The nearest wins, so a node in front is picked over one behind it at the same point.
 */
function pickInSpace(state: LoopState, at: Point): number {
  const drawn = state.drawn;
  if (drawn === null) return -1;
  let best = -1;
  let bestDepth = Infinity;
  for (let step = 0; step < drawn.drawn; step += 1) {
    const node = drawn.order[step] ?? 0;
    if (state.scene.style.hidden?.[node] === 1) continue;
    const depth = drawn.depth[node] ?? 0;
    if (depth <= 0) continue;
    const reach = Math.max(MIN_SCREEN_RADIUS, drawn.radius[node] ?? 0) + PICK_TOLERANCE;
    const dx = (drawn.x[node] ?? 0) - at.x;
    const dy = (drawn.y[node] ?? 0) - at.y;
    if (dx * dx + dy * dy > reach * reach) continue;
    if (depth < bestDepth) {
      best = node;
      bestDepth = depth;
    }
  }
  return best;
}
