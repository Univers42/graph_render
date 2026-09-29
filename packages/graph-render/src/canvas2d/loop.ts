/**
 * The frame loop. It paints on demand: a frame is requested when something changed, and
 * nothing is scheduled while the view is parked (gate row `perf-idle`).
 */
import { markNeighbourhood } from "../adjacency.ts";
import type { Camera, Viewport, ZoomLimits } from "../camera.ts";
import { type LabelPlan, type LabelPolicy, type Occupancy, planLabels } from "../labels.ts";
import type { Scene } from "../scene.ts";
import type { Theme } from "../theme.ts";
import { TRANSITION_MS, blend, easeInOutCubic } from "../transition.ts";
import { MOVING_BUDGET } from "./edges.ts";
import type { PaintCounts } from "./input.ts";
import { paintFrame } from "./paint.ts";
import type { SpriteCache } from "./sprites.ts";

/** How long after the last camera change the view still counts as moving. */
const MOVING_MS = 140;
const STAMPS = 32;

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
  selected: number;
  plan: LabelPlan;
  occupancy: Occupancy;
  scheduled: number;
  settleTimer: ReturnType<typeof setTimeout> | null;
  movedAt: number;
  destroyed: boolean;
  counts: PaintCounts;
  frameMs: number;
  frames: number;
  readonly stamps: Float64Array;
}

export function focusOf(state: LoopState): number {
  return state.hovered >= 0 ? state.hovered : state.selected;
}

export function relight(state: LoopState): void {
  markNeighbourhood(state.scene.adjacency, focusOf(state), state.lit);
}

export function invalidate(state: LoopState): void {
  if (state.scheduled !== 0 || state.destroyed) return;
  state.scheduled = requestAnimationFrame((now) => renderFrame(state, now));
}

export function markMoved(state: LoopState): void {
  state.movedAt = performance.now();
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
  return true;
}

function plan(state: LoopState, focus: number): void {
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
  }, state.plan, state.occupancy);
}

function paint(state: LoopState, moving: boolean, settled: boolean): void {
  const focus = focusOf(state);
  plan(state, focus);
  const { scene } = state;
  state.counts = paintFrame({
    ctx: state.ctx, viewport: state.viewport, dpr: state.dpr, camera: state.camera, theme: state.theme,
    frame: scene.frame, style: scene.style, adjacency: scene.adjacency, extent: scene.extent,
    x: state.x, y: state.y, settled, moving, focus, lit: state.lit, selected: state.selected,
    labels: state.plan, sprites: state.sprites,
  });
}

/** One full frame once the view has stopped, when the moving frames drew a sample. */
function armSettle(state: LoopState): void {
  if (state.settleTimer !== null) clearTimeout(state.settleTimer);
  state.settleTimer = setTimeout(() => {
    state.settleTimer = null;
    invalidate(state);
  }, MOVING_MS);
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
  state.stamps[state.frames % STAMPS] = now;
  state.frames += 1;
  state.onFrame();
  if (travelling || state.sprites.starved()) invalidate(state);
  else if (moving && state.scene.frame.edgeCount > MOVING_BUDGET) armSettle(state);
}

/** Frames per second over the last second; 0 when nothing was painted in the last 400 ms. */
export function measuredFps(state: LoopState, now: number): number {
  let count = 0;
  let oldest = now;
  let newest = 0;
  for (const stamp of state.stamps) {
    if (stamp === 0 || now - stamp > 1000) continue;
    count += 1;
    oldest = Math.min(oldest, stamp);
    newest = Math.max(newest, stamp);
  }
  if (count < 2 || now - newest > 400) return 0;
  return ((count - 1) * 1000) / (newest - oldest);
}

export function newStamps(): Float64Array {
  return new Float64Array(STAMPS);
}
