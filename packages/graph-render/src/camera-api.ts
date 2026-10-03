/**
 * The view's two faces over one controller: the scene (what is drawn) and the camera (where
 * it is drawn from). `view.ts` holds the public `View` type and the events; the bodies are
 * here, because the type is the contract and these are its two halves.
 *
 * The camera half is where the 3D camera lives next to the 2D one. Every method asks the
 * frame which kind of drawing it is and takes that branch, so a host never has to know: a
 * `zoomBy` is a dolly on a 3D frame and a scale on a 2D one, and a `focus` moves whichever
 * camera is drawing.
 */
import {
  type Camera, type FitArea, type Point, type ZoomLimits, centreOn, panBy, resetCamera, zoomAt,
} from "./camera.ts";
import { setSelection } from "./canvas2d/choose.ts";
import {
  type Controller, fit, moveOrbit, moveTo, pickAt, select, setPositions, setSafeArea, showFrame,
} from "./canvas2d/controller.ts";
import { hideNodes, togglePin } from "./canvas2d/keep.ts";
import { invalidate } from "./canvas2d/loop.ts";
import { rebaseLocal, setBaseStyle, showAll, showLocal } from "./canvas2d/local.ts";
import { edgeEndsOf, nodeOpacity, edgeOpacity, labelledNodes } from "./canvas2d/probe.ts";
import type { Frame } from "./frame.ts";
import { centreOf } from "./gesture.ts";
import { type LabelPolicy, newLabelPlan } from "./labels.ts";
import { type Orbit, dollyBy, panBy as panInSpace, resetTo, rotateBy } from "./three/orbit.ts";
import type { LoopState } from "./canvas2d/loop.ts";
import type { Projected } from "./three/projection.ts";
import type { View } from "./view.ts";

export type SceneApi = Pick<View, "setFrame" | "setStyle" | "setTheme" | "setLabels">;
export type CameraApi = Omit<View, keyof SceneApi | "on" | "toPNG" | "stats" | "radii" | "destroy">;

/** True while the frame on screen is 3D, which is what the orbit gestures are for. */
export function inSpace(state: LoopState): boolean {
  return state.orbit !== null;
}

/** The wheel and the double-click, which dolly a 3D frame and zoom a 2D one. */
export function zoomAt3d(controller: Controller, factor: number): void {
  const { state } = controller;
  if (state.orbit === null) {
    moveTo(controller, zoomAt(state.camera, centreOf(state.viewport), factor, state.limits), false);
    return;
  }
  moveOrbit(controller, dollyBy(state.orbit, factor));
}

/** A drag on a 3D frame's background: the right button slides the target, the left turns. */
export function orbitBy(controller: Controller, delta: Point, right: boolean): void {
  const orbit = controller.state.orbit;
  if (orbit === null) return;
  const turned = right ? panInSpace(orbit, delta, controller.state.viewport) : rotateBy(orbit, delta.x, delta.y);
  moveOrbit(controller, turned);
}

/**
 * Where every node is drawn, from the projection the last frame used. A 2D frame has no
 * depth and so no answer here: `position` is the 2D camera's, and it is still right.
 */
export function projectedOf(state: LoopState): readonly Projected[] | null {
  const drawn = state.drawn;
  if (state.orbit === null || drawn === null) return null;
  const out: Projected[] = [];
  for (let at = 0; at < drawn.drawn; at += 1) {
    const node = drawn.order[at] ?? 0;
    out.push({ node, x: drawn.x[node] ?? 0, y: drawn.y[node] ?? 0, depth: drawn.depth[node] ?? 0 });
  }
  return out;
}

/** The same orbit looking at one node: the target moves to it, the angles stay. */
function lookAt(orbit: Orbit, frame: Frame, node: number): Orbit {
  return { ...orbit, target: { x: frame.x[node] ?? 0, y: frame.y[node] ?? 0, z: frame.z?.[node] ?? 0 } };
}

export function sceneApi(controller: Controller): SceneApi {
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
    setLabels: (policy: LabelPolicy) => {
      state.policy = policy;
      if (policy.budget > state.plan.node.length) state.plan = newLabelPlan(policy.budget);
      invalidate(state);
    },
  };
}

/** The 3D camera's half of the face, where a host can see and set it. */
function orbitApi(controller: Controller, state: LoopState): Pick<CameraApi, "orbit" | "projected" | "setOrbit" | "resetOrbit"> {
  return {
    orbit: () => state.orbit,
    projected: () => projectedOf(state),
    setOrbit: (orbit) => moveOrbit(controller, orbit),
    resetOrbit: () => {
      if (state.orbit !== null) moveOrbit(controller, resetTo(state.orbit));
    },
  };
}

/** Centre a node, on whichever camera is drawing: the orbit's target, or the 2D offset. */
function focusApi(controller: Controller, state: LoopState): Pick<CameraApi, "focus"> {
  return {
    focus: (node) => {
      if (node < 0 || node >= state.scene.frame.nodeCount) return;
      select(controller, node);
      if (state.orbit !== null) {
        // A 3D frame's "centre this node" is the orbit's target, not the 2D camera: moving a
        // camera nothing is drawn through would be a focus that looks broken.
        moveOrbit(controller, lookAt(state.orbit, state.scene.frame, node));
        return;
      }
      // Mid-tween this aims at the frame's column, the position the node *settles* at, and not at
// the eased pose the tween is passing through. A camera placed there stays there — `moveTo`
// cuts, and no camera tween exists to override it — so the node walks into the middle of the
// frame instead of the middle chasing it for 600 ms, and the focus the user asked for is the
// focus they end up with. Aiming at the eased pose instead would mean re-aiming on every frame
// of the tween to land anywhere at all.
const world = { x: state.scene.frame.x[node] ?? 0, y: state.scene.frame.y[node] ?? 0 };
      const near = { ...state.camera, scale: Math.max(state.camera.scale, 1.2) };
      moveTo(controller, centreOn(near, world, state.viewport), false);
    },
  };
}

export function cameraApi(controller: Controller): CameraApi {
  const { state } = controller;
  return {
    setCamera: (camera: Camera) => moveTo(controller, camera, false),
    camera: () => state.camera,
    frame: () => state.scene.frame,
    style: () => state.scene.style,
    viewport: () => state.viewport,
    safeArea: () => state.safe,
    setSafeArea: (area: FitArea | null) => setSafeArea(controller, area),
    fit: () => fit(controller),
    reset: () => moveTo(controller, resetCamera(state.viewport), false),
    ...orbitApi(controller, state),
    ...focusApi(controller, state),
    zoomBy: (factor) => zoomAt3d(controller, factor),
    panBy: (delta) => moveTo(controller, panBy(state.camera, delta), false),
    limits: (): ZoomLimits => state.limits,
    select: (node) => select(controller, node),
    local: (node, options) => showLocal(controller, node, options),
    showAll: () => showAll(controller),
    selectMany: (nodes) => setSelection(controller, nodes),
    selection: () => state.selection,
    hide: (nodes) => hideNodes(controller, nodes),
    togglePin: (node) => togglePin(controller, node),
    pinned: () => state.pinned,
    setPositions: (xs, ys) => {
      // A live frame is a frame. A force layout is re-run from fresh positions on screen
      // (`liveLoop.ts` shuffles it), so the box the camera was fitted to is a box about a
      // drawing that is no longer the one being drawn: without this the whole graph walks off
      // the screen on every layout switch to a `layout.force…` and never comes back, because
      // nothing else in the live path moves the camera. The fit stops the moment the user's
      // own pan, zoom or node drag takes the camera (the `fitted` flag), so a hand's view is
      // never yanked back by a frame still arriving from the worker.
      if (setPositions(state, xs, ys) && controller.fitted) fit(controller);
    },
    position: (node) => ({ x: state.x[node] ?? 0, y: state.y[node] ?? 0 }),
    edgeEnds: (edge) => edgeEndsOf(state, edge),
    opacity: (kind, index) => (kind === "node" ? nodeOpacity : edgeOpacity)(state, index, performance.now()),
    labelled: () => labelledNodes(state),
    pick: (at) => pickAt(state, at),
  };
}
