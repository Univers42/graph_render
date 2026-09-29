/** What the view says it is drawing, read from its state and not from pixels. */
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
