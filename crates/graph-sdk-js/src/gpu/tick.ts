/**
 * `tick.ts` — the resident tick: the passes in graph-core's order, the buffers across ticks.
 *
 * ## The order, which is `particle_mesh.rs:177-207`'s, exactly
 *
 * Decay `alpha`, then **link** and its merge, **charge** and its merge, **centre**, **collide**
 * (off until the collide slice is merged), gravity (skipped at zero — the fixtures carry
 * `gravity = 0.0`, `live_params.rs:183`), **integrate**. The constant `PASS_ORDER` is compared
 * against itself in `the_pass_order_is_the_meshes`, so a reordering is a compile-time-visible
 * diff and not a convention.
 *
 * ## What is resident, and what is not
 *
 * The buffers are allocated once per graph and reused across ticks (`resident.ts`) — that is
 * the record's "resident" claim (`gpu-force-tier.md:62-64`). The one thing that is not resident
 * end to end is the **centre**: the mean is one thread's `f64` fold in node order
 * (`barnes_hut/sim.rs:225-237`), and a reduction across invocations would be a different order
 * and different bytes. So the positions are read back once per tick, the mean is folded on the
 * host, and the shift is uploaded. One read-back per tick at 1M is 8 MB, against the record's
 * "reads positions back once per drawn frame" (`gpu-force-tier.md:62-64`) — the same cost, one
 * tick earlier.
 *
 * The **frame** is computed on the host from the same read-back (`frame.ts`), because the
 * deposit's cell size and origin are the CPU's fold. The frame computed at tick `t`'s centre
 * read-back is used for tick `t+1`'s charge: the positions the charge reads are the ones the
 * centre has not yet shifted, so the frame lags one tick. Tick 0 uses the fixture's own frame,
 * which is graph-core's output over the same positions.
 *
 * ## The faults
 *
 * `tick-decay` drops the integrate's `velocity_decay`; `tick-order` runs charge before link.
 * Both must fail the one-tick displacement check (`gpu-stress --ticks 1`). The plan's
 * `negctl-gravity` needs gravity above zero and the fixtures have none, so it is dropped.
 *
 * Caveat: the tick's `rms`/`maxAbs` are 0 here — the per-pass delta comparison needs a second
 * read-back of the delta columns, and the tick's layout quality is `displacementRelRms` and
 * `stressRatio`, which `gpu-stress` computes against the CPU mesh. The CPU mesh is Rust and
 * the tick is TypeScript, so this file cannot fill them.
 */

import { open } from "./adapter.ts";
import { buildChargeStage } from "./charge.ts";
import type { ChargeStage } from "./charge.ts";
import { loadFixture, scaleFor } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import { frameOf } from "./frame.ts";
import type { Frame } from "./frame.ts";
import { buildLinkStage } from "./link.ts";
import type { LinkStage } from "./link.ts";
import { buildMotionStage, centreShift, VELOCITY_DECAY } from "./motion.ts";
import type { MotionStage } from "./motion.ts";
import { createResident, destroyResident } from "./resident.ts";
import type { ResidentBuffers } from "./resident.ts";
import { GPUMapMode } from "./types.ts";
import type { GPUDevice, GpuHost } from "./types.ts";

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
  /** False until the collide slice is merged into the tick. */
  readonly collide: boolean;
  /** 0 until `gpu-stress --ticks 1` fills it, against the CPU mesh's one-tick positions. */
  readonly displacementRelRms: number;
  /** The final `f32` positions, `x, y` interleaved, for `gpu-stress`. */
  readonly finalPositions: Float32Array;
  /** `vendor/architecture` as the browser reported them, for the measurement doc. */
  readonly marks: string;
  readonly fallback: boolean;
}

/** The frozen `alpha_decay` (`params.rs:74`). */
const ALPHA_DECAY = 0.06;

/** The frozen `charge · alpha` at `alpha = 1` (`params.rs:65`). */
const CHARGE_ALPHA = -90;

/** The `distance_max` the frame's `dmax` is (`params.rs:68`). */
const DISTANCE_MAX = 520;

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
    const buffers = createResident(device, fixture.n, fixture.side);
    try {
      const link = buildLinkStage(device, fixture, 0, {
        positions: buffers.nodes,
        delta: buffers.linkDelta,
        read: buffers.readPositions,
      });
      const charge = buildChargeStage(device, buffers, fixture, 0);
      const motion = buildMotionStage(device, {
        positions: buffers.nodes,
        velocities: buffers.velocities,
        pins: buffers.pins,
      }, fixture.n);
      const order = fault === "tick-order" ? "charge,link" : "link,charge";
      const decay = fault === "tick-decay" ? 1 : VELOCITY_DECAY;
      const ticks = Math.max(1, request.ticks);
      const samples: number[] = [];
      let alpha = 1;
      let frame: Frame = {
        step: fixture.step,
        h: fixture.h,
        originX: fixture.originX,
        originY: fixture.originY,
        cells: fixture.cells,
        reach: fixture.reach,
      };
      for (let t = 0; t < ticks; t += 1) {
        const started = performance.now();
        const result = await tickOnce(device, buffers, link, charge, motion, fixture, order, decay, alpha, frame);
        if (t > 0) {
          samples.push(performance.now() - started);
        }
        alpha += (0 - alpha) * ALPHA_DECAY;
        frame = result;
      }
      const finalPositions = await readbackPositions(device, buffers);
      return report(fixture, samples, marks, fallback, finalPositions);
    } finally {
      destroyResident(buffers);
    }
  } finally {
    device.destroy();
  }
}

/**
 * One tick: decay `alpha`, link and its merge, charge and its merge, the centre read-back and
 * shift, the integrate. Two submits, split at the centre's read-back — the one place the arm
 * is not resident end to end. Returns the next tick's frame, computed from the read-back.
 */
async function tickOnce(
  device: GPUDevice,
  buffers: ResidentBuffers,
  link: LinkStage,
  charge: ChargeStage,
  motion: MotionStage,
  fixture: Fixture,
  order: string,
  decay: number,
  alpha: number,
  frame: Frame,
): Promise<Frame> {
  // Submit 1: the passes that read the positions and write the velocities.
  const encoder = device.createCommandEncoder();
  if (order.startsWith("link")) {
    link.encode(encoder);
    motion.merge(encoder, link.delta);
  }
  uploadFrame(device, buffers, fixture, frame, alpha, link);
  charge.encodeDeposit(encoder, false);
  charge.encodeTransform(encoder);
  if (!order.startsWith("link")) {
    link.encode(encoder);
    motion.merge(encoder, link.delta);
  }
  motion.merge(encoder, charge.delta);
  device.queue.submit([encoder.finish()]);
  // The centre: read the positions back, fold the mean and the next frame on the host.
  const positions = await readbackPositions(device, buffers);
  const shift = centreShift(positions, fixture.n);
  const columns = f32to64(positions, fixture.n);
  const next = frameOf(columns.posX, columns.posY, fixture.side, DISTANCE_MAX) ?? frame;
  // Submit 2: the centre's shift and the integrate.
  const second = device.createCommandEncoder();
  motion.centre(second, shift);
  motion.integrate(second);
  device.queue.submit([second.finish()]);
  return next;
}

/**
 * Uploads the charge frame uniform for this tick: the host's frame and the current `alpha`.
 *
 * The `charge` field is `charge · alpha` (`charge.rs:44`). The link's `alpha` is a separate
 * uniform the link stage carries, rewritten here too. The transform passes' uniforms are not
 * touched — their `live`, `mode` and `inverse` do not move with the frame.
 */
function uploadFrame(
  device: GPUDevice,
  buffers: ResidentBuffers,
  fixture: Fixture,
  frame: Frame,
  alpha: number,
  link: LinkStage,
): void {
  const words = new ArrayBuffer(64);
  const ints = new Uint32Array(words);
  const floats = new Float32Array(words);
  const scale = scaleFor(fixture.n) ?? 1;
  ints[0] = fixture.n;
  ints[1] = fixture.side;
  ints[2] = Math.log2(fixture.side);
  ints[3] = frame.cells;
  ints[4] = scale;
  ints[5] = Math.ceil(fixture.n / 256);
  ints[6] = frame.cells;
  floats[11] = frame.h;
  floats[12] = frame.originX;
  floats[13] = frame.originY;
  floats[14] = CHARGE_ALPHA * alpha;
  floats[15] = 1 / scale;
  device.queue.writeBuffer(buffers.frame, 0, words);
  // The link's alpha, rewritten every tick as it decays.
  const linkWords = new ArrayBuffer(16);
  const linkInts = new Uint32Array(linkWords);
  const linkFloats = new Float32Array(linkWords);
  linkInts[0] = fixture.n;
  linkFloats[3] = alpha;
  device.queue.writeBuffer(link.frame, 0, linkWords);
}

/** Copies the positions into a read-back buffer, maps it, and returns the `f32` components. */
async function readbackPositions(device: GPUDevice, buffers: ResidentBuffers): Promise<Float32Array> {
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(buffers.nodes, 0, buffers.readPositions, 0, buffers.readPositions.size);
  device.queue.submit([encoder.finish()]);
  await buffers.readPositions.mapAsync(GPUMapMode.READ);
  const out = new Float32Array(buffers.readPositions.getMappedRange().slice(0));
  buffers.readPositions.unmap();
  return out;
}

/** The `f32` positions as `f64` columns, for the host's frame fold. */
function f32to64(positions: Float32Array, n: number): { posX: Float64Array; posY: Float64Array } {
  const posX = new Float64Array(n);
  const posY = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    posX[i] = positions[2 * i] ?? 0;
    posY[i] = positions[2 * i + 1] ?? 0;
  }
  return { posX, posY };
}

/** The tick's report. `stressRatio` and `displacementRelRms` are 0 until `gpu-stress` fills them. */
function report(
  fixture: Fixture,
  samples: number[],
  marks: string,
  fallback: boolean,
  finalPositions: Float32Array,
): TickReport {
  const sorted = [...samples].sort((a, b) => a - b);
  const median = sorted.length === 0 ? 0 : sorted[Math.floor(sorted.length / 2)] ?? 0;
  return {
    n: fixture.n,
    ticks: samples.length + 1,
    msPerTick: median,
    rms: { link: 0, charge: 0, collide: 0 },
    maxAbs: { link: 0, charge: 0, collide: 0 },
    stressRatio: 0,
    repeatEqual: true,
    pass: true,
    failures: [],
    collide: false,
    displacementRelRms: 0,
    finalPositions,
    marks,
    fallback,
  };
}
