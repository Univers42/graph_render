/**
 * The renderer's public face: a view bound to a canvas the host owns.
 *
 *   const view = createView(canvas);
 *   view.setFrame(frameFrom(decodeSnapshot(bytes)));
 *
 * A scene above BULK_THRESHOLD nodes draws through a WebGL2 layer (`webgl2/`) when the browser
 * has one, and through Canvas2D otherwise or once the GL context is lost.
 *
 * It does not: run a layout, fetch, read CSS, or keep a frame loop alive while parked.
 * Not done yet: WebGPU, pinch with two pointers, keyboard navigation of nodes.
 */
import { type Camera, type Point, type Viewport, type ZoomLimits, panBy, zoomAt } from "./camera.ts";
import { cameraApi, inSpace, orbitBy, sceneApi, zoomAt3d } from "./camera-api.ts";
import { clickAt, contextAt, pressAt } from "./canvas2d/choose.ts";
import { type Controller, fit, hover, measure, moveTo, newState, pickAt } from "./canvas2d/controller.ts";
import { invalidate } from "./canvas2d/loop.ts";
import { type EdgeEnds } from "./canvas2d/probe.ts";
import type { Frame } from "./frame.ts";
import { DOUBLE_CLICK_ZOOM } from "./gesture.ts";
import type { LabelPolicy } from "./labels.ts";
import type { LiveDrag } from "./drag.ts";
import { type LocalOptions, newLocalLayer } from "./local.ts";
import { bindPointer } from "./pointer.ts";
import { statsOf } from "./view-stats.ts";
import type { Style } from "./style.ts";
import type { Theme } from "./theme.ts";
import type { Orbit } from "./three/orbit.ts";
import type { Projected } from "./three/projection.ts";

import type { BackendChoice } from "./webgl2/plan.ts";
export type { EdgeEnds } from "./canvas2d/probe.ts";
export type { Orbit } from "./three/orbit.ts";
export type { Projected } from "./three/projection.ts";
export type { CameraApi, SceneApi } from "./camera-api.ts";
export type { BackendChoice } from "./webgl2/plan.ts";
export { BACKEND_CHOICES, backendOf } from "./webgl2/plan.ts";
export interface ViewOptions {
  readonly theme?: Theme;
  readonly labels?: LabelPolicy;
  /** A live force session: a drag pins the node in it while it is enabled. */
  readonly live?: LiveDrag;
  /**
   * Who draws a 2D scene's edges and nodes: `auto` (the default) hands a large one to a
   * WebGL2 layer when the browser has it, `canvas2d` never does, `webgl2` always does.
   * Labels, rings and the lit neighbourhood stay on the 2D context either way.
   */
  readonly backend?: BackendChoice;
}

export interface ViewStats {
  /** Who drew the last frame's edges and nodes. */
  readonly backend: "canvas2d" | "webgl2";
  /** Why the WebGL2 layer could not be used, or "". */
  readonly backendFailure: string;
  readonly nodes: number;
  readonly edges: number;
  readonly drawnNodes: number;
  readonly drawnEdges: number;
  readonly drawnLabels: number;
  /** Arrow heads in the last frame, and the length of one in CSS pixels. */
  readonly drawnArrows: number;
  readonly arrowSize: number;
  /** Edges drawn with a control point. */
  readonly curvedEdges: number;
  /** The stroke width of an edge in CSS pixels in the last frame. */
  readonly strokeWidth: number;
  /** Path fills and strokes in the last frame. */
  readonly draws: number;
  /** `stroke()` calls on edges in the last frame, and the edge styles that drew (base, lit). */
  readonly strokeCalls: number;
  readonly edgeStyles: number;
  /** Edges whose ends wore two colours in the last frame, and the strokes each took of its own. */
  readonly mixedEdges: number;
  readonly gradientStrokes: number;
  /** Fills for arrow heads and for glow discs in the last frame. */
  readonly arrowFills: number;
  readonly glowFills: number;
  /** Label sprites rasterised in the last frame: 0 when every one was already baked. */
  readonly spritesRasterised: number;
  /** Label layouts run since the view was made: it grows on a camera change, not on a redraw. */
  readonly layoutRuns: number;
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
  /**
   * The frame the last `setFrame` gave the view, as the scene holds it. Read-only: it does
   * not invalidate or repaint, and before the first frame it is the empty frame. The host
   * reads it to ask what is on screen without asking the painter.
   */
  frame(): Frame;
  /** The style the last `setStyle` gave the view, under the same rules as `frame`. */
  style(): Style;
  /** The canvas box in CSS pixels, which is what a camera's offsets are measured against. */
  viewport(): Viewport;
  setCamera(camera: Camera): void;
  camera(): Camera;
  fit(): void;
  /** 1:1 with the world origin in the middle: what the key `0` and the reset button mean. */
  reset(): void;
  /**
   * The 3D camera, or `null` when the frame is 2D. Its presence is the whole answer to
   * "is this drawing 3D", and the studio's badge reads it rather than a second flag.
   */
  orbit(): Orbit | null;
  /**
   * Where every node is drawn right now, in CSS pixels: its screen point and its depth from
   * the eye. `null` on a 2D frame, where a node has no depth and the 2D camera's own
   * `position` answers where it is. Read-only: asking does not project, invalidate or
   * repaint, and a host reads it to say what is on screen without asking the painter.
   */
  projected(): readonly Projected[] | null;
  /**
   * Puts the 3D camera where a host wants it, the way `setCamera` does for the 2D one. A
   * no-op on a 2D frame, since there is no orbit to set.
   */
  setOrbit(orbit: Orbit): void;
  /**
   * The drawing head on again: the same nodes, the same distance, the camera's angles at
   * zero. A no-op on a 2D frame, where `reset()` is the reset.
   */
  resetOrbit(): void;
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
  /** The ends of an edge as drawn, in world units, with the nodes they join (null past the last edge). */
  edgeEnds(edge: number): EdgeEnds | null;
  /** What the view draws a node or an edge at right now (1 in the focus, faded outside it). */
  opacity(kind: "node" | "edge", index: number): number;
  /**
   * New positions for the nodes already in the frame, from a live simulation. `xs`/`ys`
   * are one entry per node in dense order; a length that does not match the frame's node
   * count is ignored (the drawing is of another graph). The columns the motor handed over
   * are read, never kept: the next frame replaces them.
   */
  setPositions(xs: Float64Array, ys: Float64Array): void;
  /** The nodes whose labels the last frame placed. */
  labelled(): readonly number[];
  /** The node under a canvas-relative point, or -1. */
  pick(at: Point): number;
  on<Name extends keyof ViewEvents>(name: Name, handler: (payload: ViewEvents[Name]) => void): () => void;
  toPNG(): Promise<Blob>;
  stats(): ViewStats;
  /** World radius of every node by dense index: what the style drew, after the min and max clamp. */
  radii(): Float32Array;
  destroy(): void;
}

type Handlers = { [Name in keyof ViewEvents]: Set<(payload: ViewEvents[Name]) => void> };

function toBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob === null) reject(new Error("graph-render: the canvas could not be encoded as PNG"));
      else resolve(blob);
    }, "image/png");
  });
}

/** Pointer, wheel and resize; returns what undoes all three. */
function bindInputs(controller: Controller): () => void {
  const { canvas, state } = controller;
  const unbind = bindPointer(canvas, {
    zoom: (at, factor) => {
      if (state.orbit !== null) zoomAt3d(controller, factor);
      else moveTo(controller, zoomAt(state.camera, at, factor, state.limits), false);
    },
    pan: (delta) => moveTo(controller, panBy(state.camera, delta), false),
    orbit: (delta, right) => orbitBy(controller, delta, right),
    hover: (at) => hover(controller, at === null ? -1 : pickAt(state, at)),
    click: (at, shift) => clickAt(controller, at, shift),
    press: (at, shift) => pressAt(controller, at, shift),
    context: (at) => contextAt(controller, at),
    doubleClick: (at) => {
      // A double-click on a node is the node's own gesture (S2); on the background it zooms.
      if (pickAt(state, at) >= 0) return;
      if (state.orbit !== null) zoomAt3d(controller, DOUBLE_CLICK_ZOOM);
      else moveTo(controller, zoomAt(state.camera, at, DOUBLE_CLICK_ZOOM, state.limits), false);
    },
  }, globalThis.window, () => inSpace(state));
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
    theme: options.theme, policy: options.labels, backend: options.backend, onFrame: () => emit("frame", statsOf(state)),
  });
  const notify = {
    hover: (node: number): void => emit("hover", node),
    select: (node: number): void => emit("select", node),
    selection: (nodes: readonly number[]): void => emit("selection", nodes),
    context: (node: number, at: Point): void => emit("context", { node, at }),
    camera: (camera: Camera): void => emit("camera", camera),
  };
  const controller: Controller = { canvas, state, notify, fitted: true, local: newLocalLayer(), ...(options.live === undefined ? {} : { live: options.live }) };
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
    radii: () => state.scene.style.radius,
    destroy: () => {
      state.destroyed = true;
      if (state.scheduled !== 0) cancelAnimationFrame(state.scheduled);
      if (state.settleTimer !== null) clearTimeout(state.settleTimer);
      unbind();
    },
  };
}
