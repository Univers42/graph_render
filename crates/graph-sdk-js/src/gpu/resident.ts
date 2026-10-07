/**
 * `resident.ts` — the buffers the tick allocates once and reuses across every tick.
 *
 * The tick's claim to be "resident" is these buffers: allocated once per graph, reused across
 * ticks, never reallocated. The charge buffers (`buffers.ts`) are the largest part; the tick
 * adds the velocities, the pins, the link delta, the collide's buffers (`collide-stage.ts`) and
 * the positions read-back.
 *
 * The velocities start at zero — the fixture's state is at rest — and the pins are `NaN`
 * everywhere, the CPU's `None` (`motion.rs:64-66`): the fixtures carry no pins, so the
 * integrate's pin branch never fires, but the buffer is there so a pinned fixture would only
 * need an upload and not a new stage.
 *
 * Caveat: the positions are the charge's `nodes` buffer, shared with the link stage. The link
 * stage uploads the fixture's positions to it at build time; the charge stage does the same.
 * The two uploads carry the same bytes, so the second overwrites the first with no change.
 */

import { create, destroy } from "./buffers.ts";
import type { Buffers } from "./buffers.ts";
import type { Grid } from "./collide-grid.ts";
import { allocateCollide } from "./collide-stage.ts";
import type { CollideBuffers } from "./collide-stage.ts";
import { GPUBufferUsage } from "./types.ts";
import type { GPUBuffer, GPUDevice } from "./types.ts";

/** The resident buffers: the charge buffers plus the tick's own. */
export interface ResidentBuffers extends Buffers {
  /** The velocities, `x, y` interleaved, `f32`, starting at zero. */
  readonly velocities: GPUBuffer;
  /** The pins, `x, y` interleaved, `f32`, all `NaN` — the CPU's `None`. */
  readonly pins: GPUBuffer;
  /** The link pass's per-node increment, `2n` components. */
  readonly linkDelta: GPUBuffer;
  /** The collide stage's buffers; its `nodes` is the projection `x + v` the tick writes. */
  readonly collide: CollideBuffers;
  /** The read-back buffer for the positions, for the centre and the final output. */
  readonly readPositions: GPUBuffer;
}

/**
 * Creates the resident buffers and uploads their initial values.
 *
 * The velocities are zeroed and the pins set to `NaN`; the positions are the charge's `nodes`,
 * uploaded by the charge stage's build. The link delta and the read-back are left for their
 * first dispatch.
 */
export function createResident(device: GPUDevice, n: number, side: number, grid: Grid): ResidentBuffers {
  const charge = create(device, n, side, 4);
  const bytes = n * 8;
  // COPY_DST because the host writes the starting velocities and pins: a write to a buffer
  // without it is a validation error, and WebGPU reports one only to an error scope.
  const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST;
  const mapped = GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST;
  const velocities = device.createBuffer({ label: "velocities", size: bytes, usage: storage });
  device.queue.writeBuffer(velocities, 0, new Float32Array(n * 2));
  const pins = device.createBuffer({ label: "pins", size: bytes, usage: storage });
  device.queue.writeBuffer(pins, 0, new Float32Array(n * 2).fill(NaN));
  return {
    ...charge,
    velocities,
    pins,
    linkDelta: device.createBuffer({ label: "link-delta", size: bytes, usage: storage }),
    collide: allocateCollide(device, n, grid),
    readPositions: device.createBuffer({ label: "read-positions", size: bytes, usage: mapped }),
  };
}

/** Destroys the resident buffers, the charge buffers and the tick's own. */
export function destroyResident(buffers: ResidentBuffers): void {
  destroy(buffers);
  buffers.velocities.destroy();
  buffers.pins.destroy();
  buffers.linkDelta.destroy();
  Object.values(buffers.collide).forEach((buffer) => buffer.destroy());
  buffers.readPositions.destroy();
}
