/** The view's state changes: everything `createView` does to a canvas between frames. */
import {
  type Camera, type Point, fitCamera, limitsFor, screenToWorld,
} from "../camera.ts";
import { type LiveDrag, movedScene } from "../drag.ts";
import type { Frame } from "../frame.ts";
import { DEFAULT_POLICY, type LabelPolicy, newLabelPlan, occupancyFor } from "../labels.ts";
import type { LocalLayer } from "../local.ts";
import { EMPTY_FRAME, pickIn, sceneOf } from "../scene.ts";
import { plainStyle } from "../style.ts";
import { DARK_THEME, type Theme } from "../theme.ts";
import { setSelection } from "./choose.ts";
import { newCounts } from "./input.ts";
import { type LoopState, invalidate, markMoved, relight } from "./loop.ts";
import { MIN_SCREEN_RADIUS } from "./nodes.ts";
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
  /** The local graph, when one is shown: a fit frames it and not the whole graph. */
  readonly local: LocalLayer;
  /** The motor's live session, when the host gave one; a drag goes to it while it is enabled. */
  readonly live?: LiveDrag;
}

export interface Setup {
  readonly theme: Theme | undefined;
  readonly policy: LabelPolicy | undefined;
  readonly onFrame: () => void;
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
    camera: fitCamera(null, viewport), limits: limitsFor(null, viewport), viewport, dpr: 1,
    x: scene.frame.x, y: scene.frame.y, fromX: scene.frame.x, fromY: scene.frame.y, transitionStart: -1,
    lit: new Uint8Array(0), hovered: -1, dimStart: -1, selected: -1, selection: [], pinned: [], marquee: null,
    plan: newLabelPlan(policy.budget), layoutKey: null, layoutDirty: false, layoutRuns: 0, occupancy: occupancyFor(viewport),
    scheduled: 0, settleTimer: null, movedAt: 0, destroyed: false,
    counts: newCounts(), frameMs: 0, frames: 0, rate: newRate(),
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
  state.limits = limitsFor(state.scene.bounds, state.viewport);
  state.sprites.reset(state.theme, state.dpr);
}

export function moveTo(controller: Controller, camera: Camera, byFit: boolean): void {
  controller.fitted = byFit;
  controller.state.camera = camera;
  markMoved(controller.state);
  controller.notify.camera(camera);
}

export function fit(controller: Controller): void {
  const { state } = controller;
  moveTo(controller, fitCamera(controller.local.bounds ?? state.scene.bounds, state.viewport), true);
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
  if (resized) {
    state.lit = new Uint8Array(frame.nodeCount);
    state.hovered = -1;
    state.selected = -1;
    state.selection = [];
    state.pinned = [];
  }
  state.limits = limitsFor(state.scene.bounds, state.viewport);
  relight(state);
}

/**
 * New positions for the nodes already in the frame, from a live simulation. A pair that does
 * not have one entry per node is another graph's drawing and is ignored. The columns the motor
 * handed over are read, never kept: they go into the ones the view already draws.
 */
export function setPositions(state: LoopState, xs: Float64Array, ys: Float64Array): void {
  const count = state.scene.frame.nodeCount;
  if (xs.length !== count || ys.length !== count) return;
  if (state.x.length !== count) {
    state.x = new Float32Array(count);
    state.y = new Float32Array(count);
  }
  state.x.set(xs);
  state.y.set(ys);
  // The scene is the single source of truth: it carries the frame and the grid rebuilt on it.
  state.scene = movedScene(state.scene, { x: state.x, y: state.y });
  state.x = state.scene.frame.x;
  state.y = state.scene.frame.y;
  state.limits = limitsFor(state.scene.bounds, state.viewport);
  markMoved(state);
}

/** -1 while the nodes are moving: the grid holds where they will be, not where they are. */
export function pickAt(state: LoopState, at: Point): number {
  if (state.transitionStart >= 0) return -1;
  const world = screenToWorld(state.camera, at);
  const { scale } = state.camera;
  return pickIn(state.scene, {
    x: world.x, y: world.y, tolerance: PICK_TOLERANCE / scale, floor: MIN_SCREEN_RADIUS / scale,
  });
}
