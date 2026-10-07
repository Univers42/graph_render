/**
 * `live-session.ts` — what a `GpuMesh` (`live.ts`) reads from its session and writes back into
 * it: the state a rig is built from, and a read-back widened into the session's columns. Split
 * from `live.ts` for the house line cap; the mesh's ordering stays there.
 */

import type { ForceParams, ForceTick, Handle } from "../types.ts";
import type { MeshState } from "./live-driver.ts";

/** What a GPU mesh needs from its session: `force-gpu.ts` builds it over the session's privates. */
export interface MeshSession {
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array };
  velocities(): { readonly vxs: Float64Array; readonly vys: Float64Array };
  edges(): { readonly lo: Uint32Array; readonly hi: Uint32Array; readonly strength: Float64Array };
  params(): ForceParams;
  alpha(): number;
  reheat(alpha: number): void;
  pin(row: number, x: number, y: number): void;
  unpin(row: number): void;
  unpinAll(): void;
  setParams(params: Partial<ForceParams>): void;
  /** The session's own grow and tick, past the guard that refuses them to everyone else. */
  grow(handle: Handle): void;
  tick(ticks: number): ForceTick;
  /** Hands the session back: its CPU verbs work again. */
  detach(): void;
}

/** The rig's input: the session's columns and simple graph, its velocities and law, and `pins`. */
export function stateOf(session: MeshSession, held: ReadonlyMap<number, readonly [number, number]>): MeshState {
  const { xs, ys } = session.positions();
  const { vxs, vys } = session.velocities();
  const pins = new Float32Array(xs.length * 2).fill(NaN);
  for (const [row, at] of held) {
    if (row < xs.length) pins.set(at, row * 2);
  }
  const graph = { posX: xs, posY: ys, ...session.edges() };
  return { graph, vx: vxs, vy: vys, pins, law: session.params() };
}

/** `f32` read-backs into the session's `f64` columns, and its alpha set to the mesh's; a
 *  non-finite value is a failed tick, thrown before anything is written. */
export function writeBack(session: MeshSession, back: { readonly x: Float32Array; readonly v: Float32Array | null }, alpha: number): void {
  const { x, v } = back;
  if (!x.every(Number.isFinite) || (v !== null && !v.every(Number.isFinite))) {
    throw new Error("gpu: the device produced a non-finite position or velocity");
  }
  const { xs, ys } = session.positions();
  split(x, xs, ys);
  if (v !== null) {
    const { vxs, vys } = session.velocities();
    split(v, vxs, vys);
  }
  session.reheat(alpha);
}

/** `x, y` interleaved into two columns, widened exactly. */
function split(from: Float32Array, xs: Float64Array, ys: Float64Array): void {
  for (let i = 0; i < xs.length; i += 1) {
    xs[i] = from[2 * i] ?? 0;
    ys[i] = from[2 * i + 1] ?? 0;
  }
}

export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
