/**
 * What one layout request runs on a graph of a given size, and which engine its live session
 * ticks on.
 *
 * WHY a force layout on a large graph is not run frozen: the live loop starts on every force
 * run, and the session it steps seeds its own positions, so the frozen picture is replaced on
 * the first frame. At 400k nodes that frozen run blocked the worker for minutes and was then
 * thrown away. Past `LIVE_NODES` the graph is scattered in O(n) instead and settles on screen.
 */
import { type ForceEngine, settlesLive } from "./live.ts";

/** The force layout a large graph settles on: O(n + P² log P) a tick (`particle_mesh.rs`). */
export const PARTICLE_MESH = "layout.force.particle_mesh";

/** The layout that throws the nodes back to random positions: O(n), for "Animate" too. */
export const SCATTER = "layout.random";

/**
 * Caveat: taken from the wasm frame-budget crossover (`phase09-crossover.md`: 4 000 nodes fit
 * 16 ms a Barnes-Hut tick, 10 000 take 35.9 ms, so a 112-tick frozen run takes 4 s), not from
 * a browser trace of this rule. A graph just under it still waits for its frozen run; a
 * faster tick moves the crossover and this number does not follow it.
 */
export const LIVE_NODES = 5_000;

export interface RunPlan {
  /** The layout the motor runs now. */
  readonly run: string;
  /** The layout the run is reported as, which is the one that settles live. */
  readonly report: string;
  readonly engine: ForceEngine;
}

/** `canSettle` is false on a motor without a live session: there the frozen run is all there is. */
export function planRun(layoutId: string, nodes: number, canSettle: boolean): RunPlan {
  const large = nodes >= LIVE_NODES;
  const engine: ForceEngine = large || layoutId === PARTICLE_MESH ? "particle_mesh" : "barnes_hut";
  if (large && canSettle && settlesLive(layoutId)) return { run: SCATTER, report: PARTICLE_MESH, engine };
  return { run: layoutId, report: layoutId, engine };
}
