/** The view's state changes: everything `createView` does to a canvas between frames. */
import {
  type Camera, type FitArea, type Point, fitCamera, limitsFor, screenToWorld,
} from "../camera.ts";
import { type LiveDrag, movedScene } from "../drag.ts";
import type { Frame } from "../frame.ts";
import { DEFAULT_POLICY, type LabelPolicy, newLabelPlan, occupancyFor } from "../labels.ts";
import type { LocalLayer } from "../local.ts";
import { EMPTY_FRAME, pickIn, sceneOf } from "../scene.ts";
import { plainStyle } from "../style.ts";
import { DARK_THEME, type Theme } from "../theme.ts";
import { type Orbit, boxOf, fitOrbit } from "../three/orbit.ts";
import type { BackendChoice } from "../webgl2/plan.ts";
import { newBulkSlot } from "../webgl2/hook.ts";
import { safeOf } from "../gestured.ts";
import { setSelection } from "./choose.ts";
import { newCounts } from "./input.ts";
import { type LoopState, invalidate, markMoved, relight } from "./loop.ts";
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
    camera: fitCamera(null, viewport), limits: limitsFor(null, viewport), viewport, safe: null, dpr: 1,
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
  state.limits = limitsFor(state.scene.bounds, state.viewport, { area: safeOf(state, state.viewport) ?? undefined });
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
  state.limits = limitsFor(state.scene.bounds, state.viewport, { area: safeOf(state, state.viewport) ?? undefined });
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
  state.limits = limitsFor(state.scene.bounds, state.viewport, { area: safeOf(state, state.viewport) ?? undefined });
  relight(state);
}

/**
 * New positions for the nodes already in the frame, from a live simulation. A pair that does
 * not have one entry per node is another graph's drawing and is ignored. The columns the motor
 * handed over are read, never kept: they go into the ones the view already draws.
 *
 * A batch carrying a non-finite coordinate is refused whole, the way `snapshot/decode.ts:116`
 * refuses one: a NaN is a motor bug, and half a batch of them would draw a sprite, a pick-grid
 * entry and a NaN camera, none of which the user can undo. One bad coordinate is a bug report,
 * not a drawing.
 */
export function setPositions(state: LoopState, xs: Float64Array, ys: Float64Array): void {
  const count = state.scene.frame.nodeCount;
  if (xs.length !== count || ys.length !== count) return;
  for (let node = 0; node < count; node += 1) {
    if (Number.isFinite(xs[node]) && Number.isFinite(ys[node])) continue;
    return;
  }
  if (state.x.length !== count) {
    state.x = new Float32Array(count);
    state.y = new Float32Array(count);
  }
  state.x.set(xs);
  state.y.set(ys);
  state.bulk.placed += 1;
  // The scene is the single source of truth: it carries the frame and the grid rebuilt on it.
  state.scene = movedScene(state.scene, { x: state.x, y: state.y });
  state.x = state.scene.frame.x;
  state.y = state.scene.frame.y;
  state.limits = limitsFor(state.scene.bounds, state.viewport, { area: safeOf(state, state.viewport) ?? undefined });
  markMoved(state);
}

/** -1 while the nodes are moving: the grid holds where they will be, not where they are. */
export function pickAt(state: LoopState, at: Point): number {
  if (state.transitionStart >= 0) return -1;
  if (state.orbit !== null) return pickInSpace(state, at);
  const world = screenToWorld(state.camera, at);
  const { scale } = state.camera;
  return pickIn(state.scene, {
    x: world.x, y: world.y, tolerance: PICK_TOLERANCE / scale, floor: MIN_SCREEN_RADIUS / scale,
  });
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
