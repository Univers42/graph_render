/** What is selected: one node, or several, with the last one chosen as the primary. */
import { screenToWorld, type Point } from "../camera.ts";
import { liveGesture, movedScene } from "../drag.ts";
import type { Gesture } from "../pointer.ts";
import { addTo, boxOf, nodesInBox } from "../selection.ts";
import type { Controller } from "./controller.ts";
import { pickAt } from "./controller.ts";
import { invalidate, markMoved, relight } from "./loop.ts";

function sameNodes(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((node, at) => node === b[at]);
}

/** Nodes outside the frame are dropped; the primary is the last one left. */
export function setSelection(controller: Controller, nodes: readonly number[]): void {
  const { state } = controller;
  const count = state.scene.frame.nodeCount;
  const kept = nodes.filter((node) => node >= 0 && node < count);
  const primary = kept.at(-1) ?? -1;
  const changed = primary !== state.selected;
  if (!changed && sameNodes(kept, state.selection)) return;
  state.selection = kept;
  state.selected = primary;
  relight(state);
  invalidate(state);
  if (changed) controller.notify.select(primary);
  controller.notify.selection(kept);
}

export function clickAt(controller: Controller, at: Point, shift: boolean): void {
  const node = pickAt(controller.state, at);
  if (shift) setSelection(controller, addTo(controller.state.selection, node));
  else setSelection(controller, node >= 0 ? [node] : []);
}

function boxGesture(controller: Controller, from: Point): Gesture {
  const { state } = controller;
  const nodesUnder = (to: Point): number[] => {
    const box = boxOf(screenToWorld(state.camera, from), screenToWorld(state.camera, to));
    return nodesInBox({ x: state.x, y: state.y }, state.scene.style.hidden, box);
  };
  return {
    move: (to) => {
      state.marquee = boxOf(from, to);
      invalidate(state);
    },
    end: (to) => {
      state.marquee = null;
      setSelection(controller, nodesUnder(to));
    },
  };
}

/** The node follows the pointer exactly: its centre is put under it, not offset from it. */
function nodeGesture(controller: Controller, node: number): Gesture {
  const { state } = controller;
  return {
    move: (to) => {
      if (state.x === state.scene.frame.x) {
        state.x = state.x.slice();
        state.y = state.y.slice();
      }
      const world = screenToWorld(state.camera, to);
      state.x[node] = world.x;
      state.y[node] = world.y;
      markMoved(state);
    },
    end: () => {
      state.scene = movedScene(state.scene, { x: state.x, y: state.y });
      invalidate(state);
    },
  };
}

/**
 * A node under a moving pointer takes the camera's claim as well as the drawing's: a live
 * settle keeps arriving while the node is held, and a camera that follows it would slide the
 * held node out from under the pointer. The claim goes on the first move and not on the press,
 * so a plain click to select a node leaves the view still framing its own drawing.
 */
function held(controller: Controller, gesture: Gesture): Gesture {
  return {
    move: (at) => {
      controller.fitted = false;
      gesture.move(at);
    },
    end: (at) => gesture.end(at),
    cancel: () => gesture.cancel?.(),
  };
}

/** Shift takes the drag for a box; otherwise a press on a node takes it for that node. */
export function pressAt(controller: Controller, at: Point, shift: boolean): Gesture | null {
  if (shift) return boxGesture(controller, at);
  const node = pickAt(controller.state, at);
  if (node < 0) return null;
  const { camera } = controller.state;
  const live = controller.live === undefined ? null : liveGesture(controller.live, node, (to) => screenToWorld(camera, to), at);
  return held(controller, live ?? nodeGesture(controller, node));
}

/** A secondary click acts on the node under it: it joins the selection if it is not in it. */
export function contextAt(controller: Controller, at: Point): void {
  const { state } = controller;
  const node = pickAt(state, at);
  if (node >= 0 && !state.selection.includes(node)) setSelection(controller, [node]);
  controller.notify.context(node, at);
}
