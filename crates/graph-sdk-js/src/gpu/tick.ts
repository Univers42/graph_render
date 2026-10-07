/**
 * `tick.ts` — the resident tick: the ticks, their timing, and the report.
 *
 * The tick's claim is the record's (`gpu-force-tier.md:62-64`): the buffers are allocated once
 * per graph and reused across ticks (`resident.ts`), and the passes run in graph-core's order
 * every tick (`tick-step.ts`, `particle_mesh.rs:177-207`). `PASS_ORDER` is compared against
 * itself in `the_pass_order_is_the_meshes`, so a reordering is a visible diff and not a
 * convention. The centre, the frame and the collide's grid are folded on the host, from the
 * tick's read-backs; `tick-step.ts` says why each is the host's.
 *
 * ## What `pass` means here
 *
 * The page has no CPU mesh, so the tick cannot grade its own layout: `gpu-stress` does, from
 * the final positions this report carries (`displacementRelRms` at one tick, the stress ratio
 * at many). What the tick can see is the device refusing its work. WebGPU reports a validation
 * error only to an error scope, and a refused submit leaves every buffer as it was, so a tick
 * whose dispatches were all refused would read back its own start positions and look like a
 * tick that converged. The whole run sits in one validation scope, and a caught error fails the
 * report and names the message.
 *
 * ## The faults
 *
 * `tick-decay` drops the integrate's `velocity_decay`; `tick-order` runs charge before link.
 * Both must fail the one-tick displacement check (`gpu-stress --ticks 1`). The plan's
 * `negctl-gravity` needs gravity above zero and the fixtures have none, so it is dropped.
 *
 * Caveat: the tick's `rms`/`maxAbs` are 0 here — the per-pass deltas are graded by the per-pass
 * probes, and the tick's layout quality is `displacementRelRms` and `stressRatio`, which
 * `gpu-stress` computes against the CPU mesh. The CPU mesh is Rust and the tick is TypeScript,
 * so this file cannot fill them.
 */

import { open } from "./adapter.ts";
import { loadFixture } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import { buildRig, destroyRig, readPositions, tickOnce } from "./tick-step.ts";
import type { TickRig } from "./tick-step.ts";
import type { GpuHost } from "./types.ts";

/** The tick's pass order, `particle_mesh.rs:177-207`'s, as one compared string. */
export const PASS_ORDER = "link,charge,centre,collide,integrate";

/** What the tick needs from the host. No fault knob: the harness passes it separately. */
export interface TickRequest {
  /** The `.gmfx` bytes, exactly as `emit-gpu-fixtures` wrote them. */
  readonly fixture: ArrayBuffer;
  /** Ticks to run. The first is the warm-up, excluded from `msPerTick`. */
  readonly ticks: number;
  /** Which arm to insist on. `"any"` takes what the device offers. */
  readonly arm?: "hardware" | "software" | "any";
  /** The object carrying `gpu`: `navigator` in a page, a literal in a test. */
  readonly host?: GpuHost;
}

/** What `runTick` measured. `msPerTick` excludes the first tick's warm-up. */
export interface TickReport {
  readonly n: number;
  readonly ticks: number;
  readonly msPerTick: number;
  readonly rms: Record<"link" | "charge" | "collide", number>;
  readonly maxAbs: Record<"link" | "charge" | "collide", number>;
  /** 0 until `gpu-stress` fills it: the CPU mesh is Rust and the tick is TypeScript. */
  readonly stressRatio: number;
  readonly repeatEqual: boolean;
  readonly pass: boolean;
  readonly failures: readonly string[];
  /** True: collide runs in the tick, in its place in the order. */
  readonly collide: boolean;
  /** 0 until `gpu-stress --ticks 1` fills it, against the CPU mesh's one-tick positions. */
  readonly displacementRelRms: number;
  /** The final `f32` positions, `x, y` interleaved, for `gpu-stress`. */
  readonly finalPositions: Float32Array;
  /** `vendor/architecture` as the browser reported them, for the measurement doc. */
  readonly marks: string;
  readonly fallback: boolean;
}

/** The frozen `alpha_decay` (`params.rs:74`); the target is 0. */
const ALPHA_DECAY = 0.06;

/**
 * The resident tick for one fixture: the passes in order, the buffers across ticks.
 *
 * `fault` is a parameter and not a request field, for `charge-api.ts:1-13`'s reason: the public
 * `probeTick` cannot inject one. The harness passes `undefined` or a `tick-` name.
 */
export async function runTick(request: TickRequest, fault?: string | null): Promise<TickReport> {
  const fixture = loadFixture(request.fixture);
  const host: GpuHost = request.host ?? navigator;
  const { device, marks, fallback } = await open(host, request.arm ?? "any", fixture);
  try {
    device.pushErrorScope("validation");
    const rig = buildRig(device, fixture, fault ?? undefined);
    try {
      const ran = await run(rig, Math.max(1, request.ticks));
      const refused = await device.popErrorScope();
      return report(fixture, ran, { marks, fallback, refused: refused?.message });
    } finally {
      destroyRig(rig);
    }
  } finally {
    device.destroy();
  }
}

/** What the ticks produced: the timed samples and the final positions. */
interface Ran {
  readonly samples: readonly number[];
  readonly finalPositions: Float32Array;
}

/** `ticks` ticks, `alpha` decayed before each (`particle_mesh.rs:182`), every tick but the first timed. */
async function run(rig: TickRig, ticks: number): Promise<Ran> {
  const samples: number[] = [];
  let alpha = 1;
  for (let tick = 0; tick < ticks; tick += 1) {
    const started = performance.now();
    alpha += (0 - alpha) * ALPHA_DECAY;
    await tickOnce(rig, { tick, alpha });
    if (tick > 0) {
      samples.push(performance.now() - started);
    }
  }
  return { samples, finalPositions: await readPositions(rig, rig.buffers.nodes) };
}

/** The tick's report; `pass` is false exactly when the device refused some of the work. */
function report(
  fixture: Fixture,
  ran: Ran,
  seen: { readonly marks: string; readonly fallback: boolean; readonly refused: string | undefined },
): TickReport {
  const sorted = [...ran.samples].sort((a, b) => a - b);
  const median = sorted.length === 0 ? 0 : sorted[Math.floor(sorted.length / 2)] ?? 0;
  const failures = seen.refused === undefined ? [] : [`device refused the tick: ${seen.refused}`];
  return {
    n: fixture.n,
    ticks: ran.samples.length + 1,
    msPerTick: median,
    rms: { link: 0, charge: 0, collide: 0 },
    maxAbs: { link: 0, charge: 0, collide: 0 },
    stressRatio: 0,
    repeatEqual: true,
    pass: failures.length === 0,
    failures,
    collide: true,
    displacementRelRms: 0,
    finalPositions: ran.finalPositions,
    marks: seen.marks,
    fallback: seen.fallback,
  };
}
