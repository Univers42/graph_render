/**
 * `tick-step.ts` — one resident tick: the stages over the resident buffers, in graph-core's
 * order, split at the tick's two read-backs.
 *
 * ## The order, which is `particle_mesh.rs:177-207`'s
 *
 * The caller decays `alpha` first. Then **link** reads `x + v` (`link.rs:184-193`) and merges,
 * **charge** reads `x` and merges, the **centre** shifts `x` by the mean, **collide** reads its
 * projection `x + v` (`particle_mesh/motion.rs:136-149`), gravity is skipped at zero (the
 * fixtures carry `gravity = 0.0`, `live_params.rs:183`), and **integrate** merges the collide's
 * push, decays the velocities and moves the positions.
 *
 * ## The two read-backs
 *
 * The host folds what the device cannot fold in the CPU's order. The charge's frame and the
 * centre's mean are both folds over `x` at the start of the tick: link and charge move only
 * velocities, so the mean the CPU folds after them is the same one. The collide's grid is a
 * fold over the projection, which exists only once charge has merged. So a tick reads `x`
 * back, submits link, charge, centre and the projection, reads the projection back, and
 * submits collide and integrate. At 1M each read-back is 8 MB; that the record budgeted one
 * (`gpu-force-tier.md:62-64`) is a measured cost in `docs/measurements/gpu-g1.md`.
 *
 * Caveat: the first tick places the fixture's own frame, graph-core's fold over the `f64`
 * positions; every later tick folds the `f32` read-back, which can land one rung away from the
 * CPU's frame when the span sits on a rung's edge.
 */

import { Refusal } from "./adapter.ts";
import { buildChargeStage } from "./charge.ts";
import type { ChargeStage } from "./charge.ts";
import { gridFor } from "./collide-grid.ts";
import { buildCollideStage } from "./collide-stage.ts";
import type { CollideStage } from "./collide-stage.ts";
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
import type { GPUBuffer, GPUCommandEncoder, GPUDevice } from "./types.ts";

/** The `distance_max` the frame's `dmax` is (`params.rs:68`). */
const DISTANCE_MAX = 520;

/** The harness's two faults: `tick-decay` drops `velocity_decay`, `tick-order` runs charge before link. */
export const TICK_FAULTS: readonly string[] = ["tick-decay", "tick-order"];

/** The stages and the buffers of one graph's tick, built once. */
export interface TickRig {
  readonly device: GPUDevice;
  readonly fixture: Fixture;
  readonly buffers: ResidentBuffers;
  readonly link: LinkStage;
  readonly charge: ChargeStage;
  readonly collide: CollideStage;
  readonly motion: MotionStage;
  readonly linkFirst: boolean;
}

/** Builds every stage over one set of resident buffers; `fault` is `undefined` or a `TICK_FAULTS` name. */
export function buildRig(device: GPUDevice, fixture: Fixture, fault?: string): TickRig {
  if (fault !== undefined && !TICK_FAULTS.includes(fault)) {
    throw new Refusal(`--break ${fault}: the tick faults are ${TICK_FAULTS.join(", ")}`);
  }
  const buffers = createResident(device, fixture.n, fixture.side, gridFor(fixture.posX, fixture.posY));
  const { nodes: positions, velocities, pins } = buffers;
  const link = buildLinkStage(device, fixture, 0, { positions, velocities, delta: buffers.linkDelta, read: buffers.readPositions });
  const charge = buildChargeStage(device, buffers, fixture, 0);
  const collide = buildCollideStage(device, buffers.collide, fixture, 0);
  const decay = fault === "tick-decay" ? 1 : VELOCITY_DECAY;
  const motion = buildMotionStage(device, { positions, velocities, pins, collideDelta: collide.delta, projected: collide.nodes }, fixture.n, decay);
  return { device, fixture, buffers, link, charge, collide, motion, linkFirst: fault !== "tick-order" };
}

/** Destroys the rig's buffers; the pipelines go with the device. */
export function destroyRig(rig: TickRig): void {
  destroyResident(rig.buffers);
}

/** One tick at `alpha`, already decayed. `tick` keys the collide's jiggle and picks tick 0's frame. */
export async function tickOnce(rig: TickRig, step: { readonly tick: number; readonly alpha: number }): Promise<void> {
  const { device, fixture, motion, collide } = rig;
  const x = await readPositions(rig, rig.buffers.nodes);
  const columns = widen(x, fixture.n);
  const frame = step.tick === 0 ? fixture : frameOf(columns.posX, columns.posY, fixture.side, DISTANCE_MAX);
  const first = device.createCommandEncoder();
  encodeForces(rig, first, frame, step.alpha);
  motion.centre(first, centreShift(x, fixture.n));
  motion.project(first);
  device.queue.submit([first.finish()]);
  const projected = widen(await readPositions(rig, collide.nodes), fixture.n);
  collide.regrid(gridFor(projected.posX, projected.posY), step.tick);
  const second = device.createCommandEncoder();
  collide.encode(second);
  motion.integrate(second);
  device.queue.submit([second.finish()]);
}

/** Copies `source`'s `2n` components into the read-back buffer, maps it, and returns them. */
export async function readPositions(rig: TickRig, source: GPUBuffer): Promise<Float32Array> {
  const { device, buffers } = rig;
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(source, 0, buffers.readPositions, 0, buffers.readPositions.size);
  device.queue.submit([encoder.finish()]);
  await buffers.readPositions.mapAsync(GPUMapMode.READ);
  const out = new Float32Array(buffers.readPositions.getMappedRange().slice(0));
  buffers.readPositions.unmap();
  return out;
}

/**
 * Link and charge, each with its merge, in the mesh's order or, under `tick-order`, swapped.
 * A `null` frame is the CPU's unsolvable span, where `charge::apply` returns before it reads
 * a field (`mesh.rs:164-170`), so the charge is skipped.
 */
function encodeForces(rig: TickRig, encoder: GPUCommandEncoder, frame: Frame | null, alpha: number): void {
  const link = (): void => {
    rig.link.setAlpha(alpha);
    rig.link.encode(encoder);
    rig.motion.merge(encoder, rig.link.delta);
  };
  if (rig.linkFirst) {
    link();
  }
  if (frame !== null) {
    rig.charge.place(frame, alpha);
    rig.charge.encodeDeposit(encoder, false);
    rig.charge.encodeTransform(encoder);
    rig.motion.merge(encoder, rig.charge.delta);
  }
  if (!rig.linkFirst) {
    link();
  }
}

/** The `f32` positions as `f64` columns, for the host's folds; the widening is exact. */
function widen(positions: Float32Array, n: number): { posX: Float64Array; posY: Float64Array } {
  const posX = new Float64Array(n);
  const posY = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    posX[i] = positions[2 * i] ?? 0;
    posY[i] = positions[2 * i + 1] ?? 0;
  }
  return { posX, posY };
}
