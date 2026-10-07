/**
 * `live-probe.ts` — the browser probe's `--pass live`: a GPU mesh against its CPU twin, through
 * the public SDK (`docs/decisions/gpu-g1d.md` condition 10).
 *
 * Per fixture the graph is the fixture's own simple graph, built as a columns document; the
 * positions are the session's own spiral, not the fixture's. Two particle-mesh sessions start
 * identical: A ticks once through `gpuMesh()`, B ticks once on the CPU, and the one-tick
 * `displacementRelRms` is `gpu_stress.rs`'s, `rms(A − B) / rms(B − start)` over the `2n`
 * components. It runs at the frozen law and again at `CUSTOM`, a set that moves every
 * parameter the device reads — gravity on, collide smaller, the law's cutoff shorter — so a
 * `setParams` that never reached the device (`--break params`) fails the second number. With
 * `ticks > 1`, A then runs `ticks` more ticks alone, and `msPerTick` is their mean.
 *
 * Not exported from `index.ts`: it is a gate's page, compiled into `target/gpu-js` with the
 * rest of `src`, and it takes the module bytes from the page rather than fetching them, so the
 * SDK keeps its one `fetch`.
 */

import { encodeColumns } from "../columns.ts";
import type { ColumnsDocument } from "../columns.ts";
import { createMotor } from "../motor.ts";
import type { Motor } from "../motor.ts";
import type { ForceParams, GpuTier, Handle } from "../types.ts";
import { loadFixture } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import { liveFault } from "./live-driver.ts";
import type { GpuHost } from "./types.ts";

/** What the page hands the pass. */
export interface LiveRequest {
  readonly fixture: ArrayBuffer;
  readonly wasm: ArrayBuffer;
  readonly arm?: "hardware" | "software" | "any";
  readonly ticks: number;
  readonly host?: GpuHost;
}

/** One fixture's verdict. */
export interface LiveReport {
  readonly kind: "live";
  readonly n: number;
  readonly ticks: number;
  readonly tier: GpuTier;
  readonly reason: string;
  readonly marks: string;
  readonly relDefault: number;
  readonly relCustom: number;
  readonly msPerTick: number;
  readonly ceiling: number;
  readonly pass: boolean;
  readonly failures: readonly string[];
}

/**
 * The one-tick ceiling, from the measurement in `docs/measurements/gpu-g1.md` ("G1d live
 * twin"), with its margin stated there.
 */
export const LIVE_CEILING = 3e-5;

/** Every parameter the device reads, moved off its default and inside its range. */
const CUSTOM: Partial<ForceParams> = {
  charge: -150,
  gravity: 0.1,
  link_distance: 40,
  link_strength_scale: 0.3,
  collide_radius: 8,
  center_strength: 0.8,
  velocity_decay: 0.5,
  distance_max: 400,
};

/** One motor per page: the SDK loads one module per session, and each fixture hands the bytes over afresh. */
let pageMotor: Promise<Motor> | null = null;

export async function runLive(request: LiveRequest, fault?: string | null): Promise<LiveReport> {
  const fixture = loadFixture(request.fixture);
  pageMotor ??= createMotor(request.wasm);
  const motor = await pageMotor;
  const graph = motor.buildColumns(encodeColumns(documentOf(fixture)));
  const host: GpuHost = request.host ?? navigator;
  const arm = request.arm === "hardware" ? "hardware" : "any";
  liveFault.name = fault ?? undefined;
  try {
    const plain = await twin(motor, graph, { host, arm, ticks: 0, params: {} });
    const custom = await twin(motor, graph, { host, arm, ticks: request.ticks - 1, params: CUSTOM });
    const failures = [
      plain.tier === "gpu" ? "" : `tier=${plain.tier}`,
      plain.rel <= LIVE_CEILING ? "" : `relDefault=${plain.rel}>${LIVE_CEILING}`,
      custom.rel <= LIVE_CEILING ? "" : `relCustom=${custom.rel}>${LIVE_CEILING}`,
    ].filter((failure) => failure !== "");
    return {
      kind: "live", n: fixture.n, ticks: request.ticks, tier: custom.tier, reason: custom.reason, marks: custom.marks,
      relDefault: plain.rel, relCustom: custom.rel, msPerTick: custom.ms, ceiling: LIVE_CEILING,
      pass: failures.length === 0, failures,
    };
  } finally {
    liveFault.name = undefined;
    motor.release(graph);
  }
}

interface Twin {
  readonly host: GpuHost;
  readonly arm: "hardware" | "any";
  readonly ticks: number;
  readonly params: Partial<ForceParams>;
}

/** A against B for one tick at `params`, then `ticks` more of A alone, timed. */
async function twin(motor: Motor, graph: Handle, how: Twin): Promise<{ rel: number; ms: number; tier: GpuTier; reason: string; marks: string }> {
  const a = motor.forceSession(graph, how.params, "particle_mesh");
  const b = motor.forceSession(graph, how.params, "particle_mesh");
  try {
    const start = copy(b.positions());
    const mesh = await a.gpuMesh({ host: how.host, arm: how.arm });
    await mesh.tick(1);
    b.tick(1);
    const rel = displacementRelRms(copy(a.positions()), copy(b.positions()), start);
    const began = performance.now();
    if (how.ticks > 0) await mesh.tick(how.ticks);
    const ms = how.ticks > 0 ? (performance.now() - began) / how.ticks : 0;
    const { tier, reason, marks } = mesh;
    await mesh.release();
    return { rel, ms, tier, reason, marks };
  } finally {
    a.release();
    b.release();
  }
}

/** `gpu_stress.rs`'s metric: `rms(gpu − cpu) / rms(cpu − start)` over every component. */
export function displacementRelRms(gpu: Float64Array, cpu: Float64Array, start: Float64Array): number {
  let off = 0;
  let moved = 0;
  for (let k = 0; k < cpu.length; k += 1) {
    const c = cpu[k] ?? 0;
    off += ((gpu[k] ?? 0) - c) ** 2;
    moved += (c - (start[k] ?? 0)) ** 2;
  }
  return Math.sqrt(off / moved);
}

function copy(at: { readonly xs: Float64Array; readonly ys: Float64Array }): Float64Array {
  const out = new Float64Array(at.xs.length * 2);
  out.set(at.xs);
  out.set(at.ys, at.xs.length);
  return out;
}

/** The fixture's simple graph as a columns document: node `k` is row `k`. */
function documentOf(fixture: Fixture): ColumnsDocument {
  const nodes = Array.from({ length: fixture.n }, (_, k) => ({
    id: `n${k}`, kind: "record", database_id: null, source: "gmfx", label: "", group: null,
    weight: 0.5, version: 0, has_note: false, icon: null,
  }));
  const edges = Array.from({ length: fixture.m }, (_, k) => ({
    id: `e${k}`, source: `n${fixture.edgeLo[k] ?? 0}`, target: `n${fixture.edgeHi[k] ?? 0}`, kind: "relation",
    label: "", strength: fixture.edgeStrength[k] ?? 0, directed: false, record_id: null, child_first: false,
  }));
  return { version: 1, nodes, edges };
}
