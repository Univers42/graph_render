/**
 * `tick-step.ts` — one resident tick: the stages over the resident buffers, in graph-core's
 * order, split at the tick's two read-backs.
 *
 * ## The order, which is `particle_mesh.rs:177-207`'s
 *
 * The caller decays `alpha` first. Then **link** reads `x + v` (`link.rs:184-193`) and merges,
 * **charge** reads `x` and merges, the **centre** shifts `x` by the mean, **collide** reads its
 * projection `x + v` (`particle_mesh/motion.rs:136-149`), gravity is skipped at zero (the
 * fixtures carry `gravity = 0.0`, `live_params.rs:183`; a live law's gravity is added inside
 * the integrate), and **integrate** merges the collide's push, decays the velocities and moves
 * the positions. A law with `collide_radius = 0` skips the collide and its read-back, as the
 * CPU skips it at `d2 == 0` (`collide.rs:262-273`).
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
import { buildCollideStage, radiusOf } from "./collide-stage.ts";
import type { CollideStage } from "./collide-stage.ts";
import type { Fixture } from "./fixture.ts";
import { frameOf } from "./frame.ts";
import type { Frame } from "./frame.ts";
import { buildLinkStage } from "./link.ts";
import type { LinkStage } from "./link.ts";
import { lawOf } from "./law.ts";
import { buildMotionStage, centreShift } from "./motion.ts";
import type { MotionStage } from "./motion.ts";
import { createResident, destroyResident } from "./resident.ts";
import type { ResidentBuffers } from "./resident.ts";
import { GPUMapMode } from "./types.ts";
import type { GPUBuffer, GPUCommandEncoder, GPUDevice } from "./types.ts";
import type { ForceParams } from "../types.ts";

/**
 * The harness's faults: `tick-decay` drops `velocity_decay`, `tick-order` runs charge before
 * link, `params` builds the rig at the frozen law whatever law the graph carries — the live
 * arm's "setParams never reached the device".
 */
export const TICK_FAULTS: readonly string[] = ["tick-decay", "tick-order", "params"];

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
  /** The law the stages were built at: the graph's, or the frozen one under `params`. */
  readonly law: Readonly<ForceParams>;
}

/** Builds every stage over one set of resident buffers; `fault` is `undefined` or a `TICK_FAULTS` name. */
export function buildRig(device: GPUDevice, fixture: Fixture, fault?: string): TickRig {
  if (fault !== undefined && !TICK_FAULTS.includes(fault)) {
    throw new Refusal(`--break ${fault}: the tick faults are ${TICK_FAULTS.join(", ")}`);
  }
  const graph = fault === "params" ? { ...fixture, law: undefined } : fixture;
  const law = lawOf(graph);
  const buffers = createResident(device, graph.n, graph.side, gridFor(graph.posX, graph.posY, radiusOf(graph)));
  const { nodes: positions, velocities, pins } = buffers;
  const link = buildLinkStage(device, graph, 0, { positions, velocities, delta: buffers.linkDelta, read: buffers.readPositions });
  const charge = buildChargeStage(device, buffers, graph, 0);
  const collide = buildCollideStage(device, buffers.collide, graph, 0);
  const decay = fault === "tick-decay" ? 1 : law.velocity_decay;
  const into = { positions, velocities, pins, collideDelta: collide.delta, projected: collide.nodes };
  const motion = buildMotionStage(device, into, graph.n, { decay, gravity: law.gravity });
  return { device, fixture: graph, buffers, link, charge, collide, motion, linkFirst: fault !== "tick-order", law };
}

/** Destroys the rig's buffers, the grown row table included; the pipelines go with the device. */
export function destroyRig(rig: TickRig): void {
  rig.collide.destroy();
  destroyResident(rig.buffers);
}

/**
 * One tick at `alpha`, already decayed. `tick` keys the collide's jiggle. The frame is the
 * fixture's own on the rig's first tick (`placed`, which defaults to `tick === 0`), graph-core's
 * fold over the `f64` positions, and the read-back's on every later one.
 */
export async function tickOnce(rig: TickRig, step: { readonly tick: number; readonly alpha: number; readonly placed?: boolean }): Promise<void> {
  const { device, fixture, motion, collide, law } = rig;
  const x = await readPositions(rig, rig.buffers.nodes);
  const columns = widen(x, fixture.n);
  const placed = step.placed ?? step.tick === 0;
  // `Mesh::solve` places no frame for one node or a zero `distance_max` (`mesh.rs:164-166`).
  const solvable = fixture.n > 1 && law.distance_max > 0;
  const fold = placed ? fixture : frameOf(columns.posX, columns.posY, fixture.side, law.distance_max);
  const frame = solvable ? fold : null;
  const collides = law.collide_radius > 0;
  const first = device.createCommandEncoder();
  encodeForces(rig, first, frame, step.alpha);
  motion.centre(first, centreShift(x, fixture.n, law.center_strength));
  if (collides) motion.project(first);
  device.queue.submit([first.finish()]);
  if (collides) {
    const projected = widen(await readPositions(rig, collide.nodes), fixture.n);
    collide.regrid(gridFor(projected.posX, projected.posY, law.collide_radius), step.tick);
  }
  motion.setAlpha(step.alpha);
  const second = device.createCommandEncoder();
  if (collides) collide.encode(second);
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
