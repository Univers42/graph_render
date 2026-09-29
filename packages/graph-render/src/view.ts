/**
 * The renderer's public face: a view bound to a canvas the host owns.
 *
 *   const view = createView(canvas);
 *   view.setFrame(frameFrom(decodeSnapshot(bytes)));
 *
 * It does not: run a layout, fetch, read CSS, or keep a frame loop alive while parked.
 * Not done yet: a WebGL2 backend, pinch with two pointers, keyboard navigation of nodes.
 */
import {
  type Camera, type Point, type ZoomLimits, centreOn, panBy, resetCamera, zoomAt,
} from "./camera.ts";
import { clickAt, contextAt, pressAt, setSelection } from "./canvas2d/choose.ts";
import {
  type Controller, fit, hover, measure, moveTo, newState, pickAt, select, showFrame,
} from "./canvas2d/controller.ts";
import { hideNodes, togglePin } from "./canvas2d/keep.ts";
import { type LoopState, invalidate } from "./canvas2d/loop.ts";
import { rebaseLocal, setBaseStyle, showAll, showLocal } from "./canvas2d/local.ts";
import { edgeOpacity, labelledNodes, nodeOpacity } from "./canvas2d/probe.ts";
import { fpsOf } from "./canvas2d/rate.ts";
import type { Frame } from "./frame.ts";
import { DOUBLE_CLICK_ZOOM, centreOf } from "./gesture.ts";
import { type LabelPolicy, newLabelPlan } from "./labels.ts";
import { type LocalOptions, newLocalLayer } from "./local.ts";
import { bindPointer } from "./pointer.ts";
import type { Style } from "./style.ts";
import type { Theme } from "./theme.ts";

export interface ViewOptions {
  readonly theme?: Theme;
  readonly labels?: LabelPolicy;
}

export interface ViewStats {
  readonly backend: "canvas2d";
  readonly nodes: number;
  readonly edges: number;
  readonly drawnNodes: number;
  readonly drawnEdges: number;
  readonly drawnLabels: number;
  /** Path fills and strokes in the last frame. */
  readonly draws: number;
  /** Script time of the last frame; the rasteriser's time is not in it. */
  readonly frameMs: number;
  /** Frames painted per second while the view moves; 0 while parked. */
  readonly fps: number;
  readonly frames: number;
}

export interface ViewEvents {
  readonly hover: number;
  readonly select: number;
  /** Every selected node, the primary last. */
  readonly selection: readonly number[];
  /** A secondary click at a canvas point; `node` is what was under it, or -1. */
  readonly context: { readonly node: number; readonly at: Point };
  readonly camera: Camera;
  readonly frame: ViewStats;
}

export interface FrameOptions {
  /** Move the nodes from where they are. Ignored when the node count changed. */
  readonly animate?: boolean;
  /** Default true. */
  readonly fit?: boolean;
}

export interface View {
  setFrame(frame: Frame, options?: FrameOptions): void;
  setStyle(style: Style): void;
  setTheme(theme: Theme): void;
  setLabels(policy: LabelPolicy): void;
  setCamera(camera: Camera): void;
  camera(): Camera;
  fit(): void;
  /** 1:1 with the world origin in the middle: what the key `0` and the reset button mean. */
  reset(): void;
  zoomBy(factor: number): void;
  /** Moves the camera by screen pixels; the world under the cursor goes with it. */
  panBy(delta: Point): void;
  /** The scale this view will not go past, and the one it will not fall under. */
  limits(): ZoomLimits;
  /** Centres the node and selects it. */
  focus(node: number): void;
  select(node: number): void;
  /**
   * Shows only `node` and what a walk of `options` reaches, and fits them. Returns those nodes
   * ascending, or none (and changes nothing) when `node` is not in the frame.
   */
  local(node: number, options: LocalOptions): readonly number[];
  /** Leaves the local graph: every node the style does not hide is shown again, and fitted. */
  showAll(): void;
  selectMany(nodes: readonly number[]): void;
  selection(): readonly number[];
  /** Hides nodes from the drawing and from picking; view-only. */
  hide(nodes: readonly number[]): void;
  /** View-only pin flag, drawn as a dashed ring. */
  togglePin(node: number): void;
  pinned(): readonly number[];
  /** Where a node is drawn, in world units: a dragged node is where the pointer put it. */
  position(node: number): Point;
  /** What the view draws a node or an edge at right now (1 in the focus, faded outside it). */
  opacity(kind: "node" | "edge", index: number): number;
  /** The nodes whose labels the last frame placed. */
  labelled(): readonly number[];
  /** The node under a canvas-relative point, or -1. */
  pick(at: Point): number;
  on<Name extends keyof ViewEvents>(name: Name, handler: (payload: ViewEvents[Name]) => void): () => void;
  toPNG(): Promise<Blob>;
  stats(): ViewStats;
  destroy(): void;
}

type Handlers = { [Name in keyof ViewEvents]: Set<(payload: ViewEvents[Name]) => void> };
type SceneApi = Pick<View, "setFrame" | "setStyle" | "setTheme" | "setLabels">;
type CameraApi = Pick<
  View,
  | "setCamera" | "camera" | "fit" | "reset" | "zoomBy" | "panBy" | "limits" | "focus" | "select" | "selectMany"
  | "selection" | "hide" | "togglePin" | "pinned" | "position" | "opacity" | "labelled" | "pick" | "local" | "showAll"
>;

function statsOf(state: LoopState): ViewStats {
  return {
    backend: "canvas2d",
    nodes: state.scene.frame.nodeCount,
    edges: state.scene.frame.edgeCount,
    drawnNodes: state.counts.nodes,
    drawnEdges: state.counts.edges,
    drawnLabels: state.counts.labels,
    draws: state.counts.draws,
    frameMs: state.frameMs,
    fps: fpsOf(state.rate, performance.now()),
    frames: state.frames,
  };
}

function toBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob === null) reject(new Error("graph-render: the canvas could not be encoded as PNG"));
      else resolve(blob);
    }, "image/png");
  });
}

function sceneApi(controller: Controller): SceneApi {
  const { state } = controller;
  return {
    setFrame: (frame, options = {}) => {
      showFrame(state, frame, options.animate === true);
      rebaseLocal(controller);
      if (options.fit === false) invalidate(state);
      else fit(controller);
    },
    setStyle: (style) => {
      setBaseStyle(controller, style);
    },
    setTheme: (theme) => {
      state.theme = theme;
      state.sprites.reset(theme, state.dpr);
      invalidate(state);
    },
    setLabels: (policy) => {
      state.policy = policy;
      if (policy.budget > state.plan.node.length) state.plan = newLabelPlan(policy.budget);
      invalidate(state);
    },
  };
}

function cameraApi(controller: Controller): CameraApi {
  const { state } = controller;
  return {
    setCamera: (camera) => moveTo(controller, camera, false),
    camera: () => state.camera,
    fit: () => fit(controller),
    reset: () => moveTo(controller, resetCamera(state.viewport), false),
    zoomBy: (factor) => moveTo(controller, zoomAt(state.camera, centreOf(state.viewport), factor, state.limits), false),
    panBy: (delta) => moveTo(controller, panBy(state.camera, delta), false),
    limits: () => state.limits,
    focus: (node) => {
      if (node < 0 || node >= state.scene.frame.nodeCount) return;
      const world = { x: state.scene.frame.x[node] ?? 0, y: state.scene.frame.y[node] ?? 0 };
      const near = { ...state.camera, scale: Math.max(state.camera.scale, 1.2) };
      select(controller, node);
      moveTo(controller, centreOn(near, world, state.viewport), false);
    },
    select: (node) => select(controller, node),
    local: (node, options) => showLocal(controller, node, options),
    showAll: () => showAll(controller),
    selectMany: (nodes) => setSelection(controller, nodes),
    selection: () => state.selection,
    hide: (nodes) => hideNodes(controller, nodes),
    togglePin: (node) => togglePin(controller, node),
    pinned: () => state.pinned,
    position: (node) => ({ x: state.x[node] ?? 0, y: state.y[node] ?? 0 }),
    opacity: (kind, index) => (kind === "node" ? nodeOpacity : edgeOpacity)(state, index, performance.now()),
    labelled: () => labelledNodes(state),
    pick: (at) => pickAt(state, at),
  };
}

/** Pointer, wheel and resize; returns what undoes all three. */
function bindInputs(controller: Controller): () => void {
  const { canvas, state } = controller;
  const unbind = bindPointer(canvas, {
    zoom: (at, factor) => moveTo(controller, zoomAt(state.camera, at, factor, state.limits), false),
    pan: (delta) => moveTo(controller, panBy(state.camera, delta), false),
    hover: (at) => hover(controller, at === null ? -1 : pickAt(state, at)),
    click: (at, shift) => clickAt(controller, at, shift),
    press: (at, shift) => pressAt(controller, at, shift),
    context: (at) => contextAt(controller, at),
    doubleClick: (at) => {
      // A double-click on a node is the node's own gesture (S2); on the background it zooms.
      if (pickAt(state, at) >= 0) return;
      moveTo(controller, zoomAt(state.camera, at, DOUBLE_CLICK_ZOOM, state.limits), false);
    },
  });
  const observer = new ResizeObserver(() => {
    measure(controller);
    if (controller.fitted) fit(controller);
    else invalidate(state);
  });
  observer.observe(canvas);
  canvas.style.cursor = "grab";
  canvas.style.touchAction = "none";
  return () => {
    observer.disconnect();
    unbind();
  };
}

export function createView(canvas: HTMLCanvasElement, options: ViewOptions = {}): View {
  const handlers: Handlers = { hover: new Set(), select: new Set(), selection: new Set(), context: new Set(), camera: new Set(), frame: new Set() };
  const emit = <Name extends keyof ViewEvents>(name: Name, payload: ViewEvents[Name]): void => {
    for (const handler of handlers[name]) handler(payload);
  };
  const state = newState(canvas, {
    theme: options.theme, policy: options.labels, onFrame: () => emit("frame", statsOf(state)),
  });
  const notify = {
    hover: (node: number): void => emit("hover", node),
    select: (node: number): void => emit("select", node),
    selection: (nodes: readonly number[]): void => emit("selection", nodes),
    context: (node: number, at: Point): void => emit("context", { node, at }),
    camera: (camera: Camera): void => emit("camera", camera),
  };
  const controller: Controller = { canvas, state, notify, fitted: true, local: newLocalLayer() };
  measure(controller);
  const unbind = bindInputs(controller);
  return {
    ...sceneApi(controller),
    ...cameraApi(controller),
    on: (name, handler) => {
      handlers[name].add(handler);
      return () => void handlers[name].delete(handler);
    },
    toPNG: () => toBlob(canvas),
    stats: () => statsOf(state),
    destroy: () => {
      state.destroyed = true;
      if (state.scheduled !== 0) cancelAnimationFrame(state.scheduled);
      if (state.settleTimer !== null) clearTimeout(state.settleTimer);
      unbind();
    },
  };
}
