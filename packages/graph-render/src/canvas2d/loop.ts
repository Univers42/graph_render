/**
 * The frame loop. It paints on demand: a frame is requested when something changed, and
 * nothing is scheduled while the view is parked (gate row `perf-idle`).
 */
import { markNeighbourhood } from "../adjacency.ts";
import type { Bounds, Camera, Viewport, ZoomLimits } from "../camera.ts";
import { dimAt, fadeLevel } from "../fade.ts";
import { type LabelPlan, type LabelPolicy, type Occupancy, planLabels } from "../labels.ts";
import type { Scene } from "../scene.ts";
import type { Theme } from "../theme.ts";
import { TRANSITION_MS, blend, easeInOutCubic } from "../transition.ts";
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
  limits: ZoomLimits;
  viewport: Viewport;
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

/** Moves the nodes towards the frame; true while they are still on their way. */
function advance(state: LoopState, now: number): boolean {
  if (state.transitionStart < 0) return false;
  const t = (now - state.transitionStart) / TRANSITION_MS;
  if (t >= 1) {
    state.transitionStart = -1;
    state.x = state.scene.frame.x;
    state.y = state.scene.frame.y;
    return false;
  }
  const eased = easeInOutCubic(t);
  blend(state.fromX, state.scene.frame.x, eased, state.x);
  blend(state.fromY, state.scene.frame.y, eased, state.y);
  state.bulk.placed += 1;
  return true;
}

function plan(state: LoopState, focus: number, travelling: boolean): void {
  if (!layoutChanged(state, focus, travelling)) return;
  state.layoutRuns += 1;
  const { scene, sprites } = state;
  planLabels({
    style: scene.style,
    x: state.x,
    y: state.y,
    extent: scene.extent,
    camera: state.camera,
    viewport: state.viewport,
    lit: focus >= 0 ? state.lit : null,
    policy: state.policy,
    widthOf: (node) => sprites.widthOf(scene.style.labels[node] ?? ""),
    height: state.theme.labelHeight,
  }, state.plan, state.occupancy);
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
  if (drawn === null) plan(state, focus, travelling);
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
  if (travelling || fading(state, performance.now()) || rebake) invalidate(state);
  else if (moving && (state.scene.frame.edgeCount > state.pace.budget || drewAWay(state.counts))) armSettle(state);
}
