/**
 * `live-driver.ts` — one device and the rig over a live session's state, behind the small
 * surface `live.ts` drives.
 *
 * `GpuMesh` (`live.ts`) owns the ordering, the fallbacks and the session; this file owns the
 * device. The seam is `GpuDriver`, so the orchestration is tested in node against a fake driver
 * and only this file needs a browser.
 *
 * Every rig build and every tick runs inside a validation error scope: WebGPU reports a
 * validation error to nobody unless a scope catches it, and an uncaught one leaves the buffers
 * holding whatever the rejected dispatch did not write — a silent wrong layout. A caught one is
 * a throw, which `GpuMesh` turns into the CPU fallback.
 *
 * Caveat: the software check reads the adapter's own words — `isFallbackAdapter` and the names
 * SwiftShader, llvmpipe, lavapipe in its info, as the probe does (`deploy/perf/gpu-mesh.py`). A
 * software adapter that names none of them and does not call itself a fallback passes as
 * hardware; the escape hatch is `arm: "any"`, which takes it knowingly.
 */

import type { ForceParams } from "../types.ts";
import { Refusal, fallbackOf, open } from "./adapter.ts";
import type { LiveGraph } from "./live-fixture.ts";
import { liveFixture, sideFor } from "./live-fixture.ts";
import { buildRig, destroyRig, readPositions, tickOnce } from "./tick-step.ts";
import type { TickRig } from "./tick-step.ts";
import type { GPUAdapter, GPUDevice, GpuHost } from "./types.ts";

/** Which adapter the live arm takes: a hardware one only, or whatever the browser offers. */
export type GpuArm = "hardware" | "any";

/** The state a rig is (re)built from: the session's columns, its velocities, pins and law. */
export interface MeshState {
  readonly graph: LiveGraph;
  readonly vx: Float64Array;
  readonly vy: Float64Array;
  /** Per row `x, y` as `f32`: `NaN` is "not pinned", the CPU's `None`. */
  readonly pins: Float32Array;
  readonly law: Readonly<ForceParams>;
}

/** The device half of a GPU mesh. Every method but `tick`/`read`/`load` is a queue write. */
export interface GpuDriver {
  /** `vendor/architecture`, for the tier line. */
  readonly marks: string;
  /** Settles with the browser's reason when the device is lost. */
  readonly lost: Promise<string>;
  load(state: MeshState): Promise<void>;
  pin(row: number, at: readonly [number, number] | null): void;
  unpinAll(): void;
  tick(step: { readonly tick: number; readonly alpha: number }): Promise<void>;
  /** The positions, and the velocities when asked, `x, y` interleaved, `f32`. */
  read(velocities: boolean): Promise<{ readonly x: Float32Array; readonly v: Float32Array | null }>;
  destroy(): void;
}

/** Opens a driver for `n` nodes, or throws a `Refusal` naming why there is none. */
export type DriverOpener = (n: number, arm: GpuArm) => Promise<GpuDriver>;

/**
 * The probe's fault seam: a `TICK_FAULTS` name every opener built after it is set passes to
 * its rigs. Never exported from `index.ts`; the `gpu-mesh` probe page imports this module by
 * path and sets it, which is how `--break params` reaches a live session without a public knob.
 */
export const liveFault: { name: string | undefined } = { name: undefined };

/** The names a software adapter goes by, lower-case, as the probe's refusal lists them. */
const SOFTWARE_MARKS: readonly string[] = ["swiftshader", "llvmpipe", "lavapipe"];

/** The opener over `host`'s WebGPU; `fault` is a `TICK_FAULTS` name for the probe, else undefined. */
export function deviceOpener(host: GpuHost, fault?: string): DriverOpener {
  return async (n, arm) => {
    const opened = await open(host, arm, { n, side: sideFor(n) });
    const software = softwareName(opened.adapter);
    if (arm === "hardware" && software !== null) {
      opened.device.destroy();
      throw new Refusal(`asked for a hardware adapter and the browser offered a software one: ${software}`);
    }
    return createDriver(opened.device, opened.marks, fault);
  };
}

/** The adapter's software name, or `null` when it reads as hardware. */
export function softwareName(adapter: GPUAdapter): string | null {
  const info = adapter.info ?? {};
  const words = [info.vendor, info.architecture, info.device, info.description].filter((w) => !!w).join(" ");
  const named = SOFTWARE_MARKS.find((mark) => words.toLowerCase().includes(mark));
  if (named !== undefined) return `${named} (${words})`;
  return fallbackOf(adapter) ? `a fallback adapter (${words || "no info"})` : null;
}

/** The driver over one opened device. */
function createDriver(device: GPUDevice, marks: string, fault?: string): GpuDriver {
  let rig: TickRig | null = null;
  let placed = true;
  const live = (): TickRig => {
    if (rig === null) throw new Refusal("gpu: no rig is loaded");
    return rig;
  };
  return {
    marks,
    lost: device.lost.then((info) => info.message || "the device was lost"),
    async load(state: MeshState): Promise<void> {
      if (rig !== null) destroyRig(rig);
      rig = null;
      rig = await scoped(device, () => buildRig(device, liveFixture(state.graph, state.law), fault));
      device.queue.writeBuffer(rig.buffers.velocities, 0, interleave(state.vx, state.vy));
      device.queue.writeBuffer(rig.buffers.pins, 0, state.pins);
      placed = true;
    },
    pin(row: number, at: readonly [number, number] | null): void {
      device.queue.writeBuffer(live().buffers.pins, row * 8, Float32Array.from(at ?? [NaN, NaN]));
    },
    unpinAll(): void {
      const rows = live().fixture.n;
      device.queue.writeBuffer(live().buffers.pins, 0, new Float32Array(rows * 2).fill(NaN));
    },
    async tick(step): Promise<void> {
      const at = { ...step, placed };
      placed = false;
      await scoped(device, () => tickOnce(live(), at));
    },
    async read(velocities: boolean): Promise<{ readonly x: Float32Array; readonly v: Float32Array | null }> {
      const x = await readPositions(live(), live().buffers.nodes);
      const v = velocities ? await readPositions(live(), live().buffers.velocities) : null;
      return { x, v };
    },
    destroy(): void {
      if (rig !== null) destroyRig(rig);
      rig = null;
      device.destroy();
    },
  };
}

/** `run` inside a validation scope: a caught error is a throw naming it. */
async function scoped<T>(device: GPUDevice, run: () => T | Promise<T>): Promise<T> {
  device.pushErrorScope("validation");
  const settled = await Promise.resolve()
    .then(run)
    .then(
      (value) => ({ value }),
      (error: unknown) => ({ error }),
    );
  const caught = await device.popErrorScope();
  if (caught !== null) throw new Error(`gpu: validation error: ${caught.message}`);
  if ("error" in settled) throw settled.error;
  return settled.value;
}

/** Two `f64` columns as one `f32` `x, y` interleaved buffer, narrowed once. */
function interleave(xs: Float64Array, ys: Float64Array): Float32Array {
  const out = new Float32Array(xs.length * 2);
  for (let i = 0; i < xs.length; i += 1) {
    out[2 * i] = xs[i] ?? 0;
    out[2 * i + 1] = ys[i] ?? 0;
  }
  return out;
}
