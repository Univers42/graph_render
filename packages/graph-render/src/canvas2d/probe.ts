/** What the view says it is drawing, read from its state and not from pixels. */
import type { Point } from "../camera.ts";
import { dimOpacity, focusOf } from "./loop.ts";

export interface Reading {
  readonly hovered: number;
  readonly selected: number;
  readonly dimStart: number;
  readonly theme: { readonly dimAlpha: number };
  readonly lit: Uint8Array;
  readonly scene: { readonly frame: { readonly source: Uint32Array; readonly target: Uint32Array } };
  readonly plan: { readonly count: number; readonly node: Uint32Array };
}

/** 1 for the focus and its neighbours, the faded opacity for every other node. */
export function nodeOpacity(state: Reading, node: number, now: number): number {
  const focus = focusOf(state);
  if (focus < 0 || state.lit[node] === 1) return 1;
  return dimOpacity(state, now);
}

/** 1 for an edge of the focus, the faded opacity for every other edge. */
export function edgeOpacity(state: Reading, edge: number, now: number): number {
  const focus = focusOf(state);
  const { source, target } = state.scene.frame;
  if (focus < 0 || source[edge] === focus || target[edge] === focus) return 1;
  return dimOpacity(state, now);
}

/** The nodes whose labels the last frame placed, in the planner's order. */
export function labelledNodes(state: Reading): readonly number[] {
  return Array.from(state.plan.node.subarray(0, state.plan.count));
}

export interface EdgeEnds {
  readonly source: number;
  readonly target: number;
  readonly from: Point;
  readonly to: Point;
}

interface Drawn {
  readonly scene: Reading["scene"];
  readonly x: Float32Array;
  readonly y: Float32Array;
}

/** The ends of an edge as drawn, with the nodes they join; null past the last edge. */
export function edgeEndsOf(state: Drawn, edge: number): EdgeEnds | null {
  const { source, target } = state.scene.frame;
  const a = source[edge];
  const b = target[edge];
  if (a === undefined || b === undefined) return null;
  return {
    source: a, target: b,
    from: { x: state.x[a] ?? 0, y: state.y[a] ?? 0 },
    to: { x: state.x[b] ?? 0, y: state.y[b] ?? 0 },
  };
}
