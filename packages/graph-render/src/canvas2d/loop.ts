/**
 * The frame loop. It paints on demand: a frame is requested when something changed, and
 * nothing is scheduled while the view is parked (gate row `perf-idle`).
 */
import { markNeighbourhood } from "../adjacency.ts";
import type { Bounds, Camera, FitArea, Viewport, ZoomLimits } from "../camera.ts";
import { dimAt, fadeLevel } from "../fade.ts";
import { type LabelInput, type LabelPlan, type LabelPolicy, type Occupancy, followLabels, planLabels } from "../labels.ts";
import type { Scene } from "../scene.ts";
import type { Theme } from "../theme.ts";
import { TRANSITION_MS, blend, easeInOutCubic, markTween } from "../transition.ts";
import { type LayoutKey, layoutChanged } from "./layout-key.ts";
import type { PaintCounts } from "./input.ts";
import { paintOverlay } from "./overlay.ts";
import { type Pace, paced, worthPacing } from "./pace.ts";
import { paintFrame } from "./paint.ts";
import { type Rate, stamp } from "./rate.ts";
import type { SpriteCache } from "./sprites.ts";
import type { Orbit } from "../three/orbit.ts";
import { type Drawn, newProjection, projectFrame } from "../three/projection.ts";
import { type BulkSlot, paintBulk } from "../webgl2/hook.ts";

/** How long after the last camera change the view still counts as moving. */
const MOVING_MS = 140;

export interface LoopState {
  readonly ctx: CanvasRenderingContext2D;
  readonly sprites: SpriteCache;
  readonly onFrame: () => void;
  theme: Theme;
  policy: LabelPolicy;
  scene: Scene;
  camera: Camera;
  /** An accessor (`limits.ts`): built on read, so nothing may assign it. */
  readonly limits: ZoomLimits;
  viewport: Viewport;
  /**
   * The part of `viewport` a fit puts its drawing in, in canvas pixels: the chrome a host lays
   * over the canvas is not drawing space. `null` is the whole canvas, which is what a view with
   * nothing on top of it says, and what every caller said before this field existed.
   */
  safe: FitArea | null;
  dpr: number;
  x: Float32Array;
  y: Float32Array;
  fromX: Float32Array;
  fromY: Float32Array;
  /** `performance.now()` when the transition began, or -1. */
  transitionStart: number;
  lit: Uint8Array;
  hovered: number;
  /** `performance.now()` when the focus appeared, or -1 while there is none: the fade's clock. */
  dimStart: number;
  selected: number;
  /** Every selected node, the primary (`selected`) last. */
  selection: readonly number[];
  /** Nodes marked pinned: a view-only flag drawn as a ring, until the motor owns pinning. */
  pinned: readonly number[];
  /** The box a shift-drag is drawing, in canvas pixels, or null. */
  marquee: Bounds | null;
  plan: LabelPlan;
  /**
   * The 3D camera, when the frame carries a z column; `null` for a 2D frame, which is every
   * frame the 2D camera owns. The one place the view says which kind of drawing this is.
   */
  orbit: Orbit | null;
  /** The frame's nodes projected through `orbit`, reused across frames. Null for a 2D one. */
  drawn: Drawn | null;
  /** What the last layout ran for, and whether a writer (a drag) changed it since. */
  layoutKey: LayoutKey | null;
  layoutDirty: boolean;
  /** Label layouts run since the view was made. */
  layoutRuns: number;
  occupancy: Occupancy;
  scheduled: number;
  settleTimer: ReturnType<typeof setTimeout> | null;
  movedAt: number;
  destroyed: boolean;
  counts: PaintCounts;
  frameMs: number;
  frames: number;
  readonly rate: Rate;
  /** The moving edge budget of the 2D passes. */
  readonly pace: Pace;
  /** The GPU layer a large 2D scene is drawn on, and the backend the host asked for. */
  readonly bulk: BulkSlot;
}

export function focusOf(state: Pick<LoopState, "hovered" | "selected">): number {
  return state.hovered >= 0 ? state.hovered : state.selected;
}

export function relight(state: LoopState): void {
  const focus = focusOf(state);
  markNeighbourhood(state.scene.adjacency, focus, state.lit);
  if (focus < 0) state.dimStart = -1;
  else if (state.dimStart < 0) state.dimStart = performance.now();
}

/** The opacity of a node outside the focus right now: 1 with no focus, the dim alpha once faded. */
export function dimOpacity(state: { readonly dimStart: number; readonly theme: { readonly dimAlpha: number } }, now: number): number {
  return dimAt(state.theme.dimAlpha, fadeLevel(state.dimStart, now));
}

/** True while the fade is still moving, so the loop asks for another frame. */
export function fading(state: LoopState, now: number): boolean {
  return state.dimStart >= 0 && fadeLevel(state.dimStart, now) < 1;
}

export function invalidate(state: LoopState): void {
  if (state.scheduled !== 0 || state.destroyed) return;
  state.scheduled = requestAnimationFrame((now) => renderFrame(state, now));
}

export function markMoved(state: LoopState): void {
  state.movedAt = performance.now();
  state.layoutDirty = true;
  invalidate(state);
}

/**
 * The node count from which the 2D painter stops easing a layout switch and snaps to it.
 *
 * Measured, not guessed (docs/measurements/perf-transition.md, `canvas2d`, a tween of the same
 * graph under `layout.random` -> `layout.grid`, 600 ms, 1920x1080 at DPR 1): 20 000 nodes held
 * 16.7 ms frames and drew 31 of the 36 the tween wanted; 32 768 drew 20 and its frame gaps were
 * already 33 ms; 65 536 drew 13; 131 072 drew 6 and 200 000 drew 4, at 183 ms a frame. Past this
 * count the tween is a slideshow, and the snap shows the new layout on the first frame instead.
 *
 * Ponytail: the GPU layer mixes the same two columns in its vertex shader for one float a frame,
 * so this budget is the 2D painter's alone — a tween over it on WebGL2 still runs, and `counts`
 * is what says which painter has the frame. Failing input: a host that put a node budget like
 * this one above 32 768 on purpose gets the snap, which is cheaper and less smooth, not wrong.
 * Direction: snap above the budget. Escape hatch: set `TWEEN_BUDGET` above the scene's node count.
 */
export const TWEEN_BUDGET = 32_768;

/** The frame is where the nodes are now, and the layer has nothing left to mix. */
function arrive(state: LoopState): void {
  state.transitionStart = -1;
  state.x = state.scene.frame.x;
  state.y = state.scene.frame.y;
  state.bulk.tween = null;
}

/**
 * True when the 2D painter, not the GPU layer, is the one that would have to ease this tween:
 * `bulk` is the draw count of the last painted frame, so it is 0 exactly when the layer drew
 * nothing (hook.ts). Exported, and narrow on purpose, so the budget is a number a test reads.
 */
export function overBudget(counts: Pick<PaintCounts, "bulk">, nodeCount: number): boolean {
  return counts.bulk === 0 && nodeCount > TWEEN_BUDGET;
}

/**
 * Moves the nodes towards the frame; true while they are still on their way.
 *
 * Exported, and narrow on purpose, so a test can step a tween to a given fraction without a
 * browser's animation frames — node has no `requestAnimationFrame`, so the loop cannot run
 * there at all. Nothing outside this file calls it.
 */
export function advance(state: LoopState, now: number): boolean {
  if (state.transitionStart < 0) return false;
  const t = (now - state.transitionStart) / TRANSITION_MS;
  const snapped = overBudget(state.counts, state.scene.frame.nodeCount);
  markTween(state, state.transitionStart, t >= 1 || snapped);
  if (t >= 1 || snapped) {
    arrive(state);
    return false;
  }
  const eased = easeInOutCubic(t);
  const { frame } = state.scene;
  blend(state.fromX, frame.x, eased, state.x);
  blend(state.fromY, frame.y, eased, state.y);
  state.bulk.placed += 1;
  // The layer mixes these four columns on the GPU; `state.x`/`state.y` stay the eased pose for
  // the labels, the edges and the hit test, which are all on this side of the fence.
  state.bulk.tween = { fromX: state.fromX, fromY: state.fromY, toX: frame.x, toY: frame.y, eased };
  return true;
}

/**
 * The node count from which a moving frame follows the last label plan instead of laying one
 * out. Estimated, not measured per size: the full plan cost about 7 ms a frame at 1 000 000
 * nodes zoomed in (target/p5-zoom.log), a linear rank scan, so about 1 ms here.
 */
const FOLLOW_FROM = 131_072;

function labelInput(state: LoopState, focus: number): LabelInput {
  const { scene, sprites } = state;
  return {
    style: scene.style, x: state.x, y: state.y, extent: scene.extent, camera: state.camera,
    viewport: state.viewport, lit: focus >= 0 ? state.lit : null, policy: state.policy,
    widthOf: (node) => sprites.widthOf(scene.style.labels[node] ?? ""), height: state.theme.labelHeight,
  };
}

/** True when the last plan was laid out for this scene, focus, policy and viewport, which only a camera or a move changed since. */
function followable(previous: LayoutKey | null, state: LoopState, focus: number): boolean {
  return previous !== null && previous.scene === state.scene && previous.focus === focus
    && previous.policy === state.policy && previous.theme === state.theme && previous.viewport === state.viewport
    && state.scene.frame.nodeCount >= FOLLOW_FROM;
}

function plan(state: LoopState, focus: number, travelling: boolean, moving: boolean): void {
  const previous = state.layoutKey;
  if (!layoutChanged(state, focus, travelling)) return;
  const input = labelInput(state, focus);
  if (moving && followable(previous, state, focus)) {
    followLabels(input, state.plan);
    // The settled frame lays the labels out in full again, whatever its key says.
    state.layoutDirty = true;
    return;
  }
  state.layoutRuns += 1;
  planLabels(input, state.plan, state.occupancy);
}

/**
 * The 3D drawing for this frame, or null for a 2D one. A frame with a z column is projected
 * through the orbit camera; anything else is drawn by the 2D passes, which never see it.
 */
function space(state: LoopState): Drawn | null {
  const { frame, orbit } = { frame: state.scene.frame, orbit: state.orbit };
  if (frame.z === null || orbit === null) return null;
  const drawn = projectFrame(state.drawn ?? newProjection(frame.nodeCount), {
    frame, x: state.x, y: state.y, extent: state.scene.extent, orbit, viewport: state.viewport,
  });
  state.drawn = drawn;
  return drawn;
}

function paint(state: LoopState, moving: boolean, settled: boolean): void {
  const travelling = !settled;
  const focus = focusOf(state);
  const theme = { ...state.theme, dimAlpha: dimOpacity(state, performance.now()) };
  const drawn = space(state);
  if (drawn === null) plan(state, focus, travelling, moving);
  const { scene } = state;
  state.counts = paintFrame({
    ctx: state.ctx, viewport: state.viewport, dpr: state.dpr, camera: state.camera, theme, space: drawn,
    frame: scene.frame, style: scene.style, adjacency: scene.adjacency, extent: scene.extent,
    x: state.x, y: state.y, settled, moving, edgeBudget: state.pace.budget, focus, lit: state.lit, selected: state.selected,
    labels: state.plan, sprites: state.sprites, bulk: (input, counts) => paintBulk(state.bulk, input, counts),
  });
  paintOverlay(state);
}

/** One full frame once the view has stopped, when the moving frames drew a sample. */
function armSettle(state: LoopState): void {
  if (state.settleTimer !== null) clearTimeout(state.settleTimer);
  state.settleTimer = setTimeout(() => {
    state.settleTimer = null;
    invalidate(state);
  }, MOVING_MS);
}

/** One full frame once the view has stopped, when a moving frame drew less than the whole set. */
export function drewAWay(counts: Pick<PaintCounts, "mixedEdges" | "gradientStrokes">): boolean {
  // The mixed edges gave the gradient up while the view moved: one settled frame puts it back.
  return counts.mixedEdges > 0 && counts.gradientStrokes === 0;
}

/** Times a moving frame up to the next animation frame, raster included, and paces the budget by it. */
function probeCost(state: LoopState, now: number): void {
  const { pace, scene } = state;
  const flat = scene.frame.z === null || state.orbit === null;
  if (pace.probing || !worthPacing(scene.frame.edgeCount, state.counts.bulk > 0, flat)) return;
  pace.probing = true;
  requestAnimationFrame((next) => {
    pace.probing = false;
    paced(pace, next - now);
  });
}

function renderFrame(state: LoopState, now: number): void {
  state.scheduled = 0;
  if (state.destroyed) return;
  const travelling = advance(state, now);
  const moving = travelling || performance.now() - state.movedAt < MOVING_MS;
  state.sprites.beginFrame();
  const started = performance.now();
  paint(state, moving, !travelling);
  state.frameMs = performance.now() - started;
  stamp(state.rate, now, moving);
  if (moving) probeCost(state, now);
  state.frames += 1;
  state.onFrame();
  // A frame that baked a label planned it at width 0: one more frame lays it out at its width.
  const rebake = state.sprites.starved() || state.sprites.rasterised() > 0;
  if (travelling || fading(state, performance.now()) || rebake || state.bulk.refining) invalidate(state);
  else if (moving && (state.scene.frame.edgeCount > state.pace.budget || drewAWay(state.counts) || state.layoutDirty)) armSettle(state);
}
