/**
 * `collide.ts` — the collide probe on a WebGPU device: the fixture's grid, the stage twice,
 * the read-back, the report.
 *
 * The CPU pass is `collide.rs`: a counting sort of the nodes into a hashed cell list one
 * diameter wide, then a gather over each node's nine neighbour cells. The device runs the
 * same shape in four stages, `collide_hash`, `collide_scan`, `collide_scatter` and
 * `collide_resolve`, one module each under `kernels/`. The host's grid is `collide-grid.ts`,
 * the buffers and dispatches are `collide-stage.ts`, which the resident tick shares; this file
 * is the probe over them, and re-exports the grid so its importers keep one path.
 *
 * The pass runs twice; the delta, the member lists and their spans come back.
 */

import { Refusal, open } from "./adapter.ts";
import { collideReport } from "./bounds-collide.ts";
import type { CollideRan } from "./bounds-collide.ts";
import type { Arm } from "./bounds.ts";
import { gridFor, scanPlan } from "./collide-grid.ts";
import { allocateCollide, buildCollideStage } from "./collide-stage.ts";
import type { CollideBuffers, CollideStage } from "./collide-stage.ts";
import { loadFixture } from "./fixture.ts";
import { COLLIDE_FAULTS } from "./kernels/collide.wgsl.ts";
import type { PassReport } from "./pass-report.ts";
import type { GPUDevice, GpuHost } from "./types.ts";
import { GPUMapMode } from "./types.ts";

export { cellOf, gridFor, scanPlan } from "./collide-grid.ts";
export type { Grid } from "./collide-grid.ts";

/** What the collide arm needs from the host; no fault knob, as `ChargeRequest` has none. */
export interface CollideRequest {
  /** The `.gmfx` bytes, exactly as `emit-gpu-fixtures` wrote them. */
  readonly fixture: ArrayBuffer;
  /** Which arm to insist on. `"any"` takes what the device offers. */
  readonly arm?: Arm | "any";
  /** The object carrying `gpu`: `navigator` when omitted, a literal in a node test. */
  readonly host?: GpuHost;
}

/**
 * The collide pass for one fixture, twice, then the report. `fault` is a parameter and not a
 * request field, for `charge-api.ts:1-13`'s reason: `undefined`, or one of `COLLIDE_FAULTS`.
 */
export async function runCollide(request: CollideRequest, fault?: string | null): Promise<PassReport> {
  const fixture = loadFixture(request.fixture);
  const code = faultCodeFor(fault);
  const grid = gridFor(fixture.posX, fixture.posY);
  const { device, marks, fallback } = await open(request.host ?? navigator, request.arm ?? "any", fixture);
  try {
    checkGrid(device, grid.buckets);
    const buffers = allocateCollide(device, fixture.n, grid);
    try {
      const stage = buildCollideStage(device, buffers, fixture, code);
      const sizes = { n: fixture.n, buckets: grid.buckets };
      const first = await runCollideOnce(device, buffers, stage, sizes);
      const second = await runCollideOnce(device, buffers, stage, sizes);
      const arm: Arm = request.arm === "software" || (request.arm !== "hardware" && fallback) ? "software" : "hardware";
      return collideReport(fixture, [first, second], { arm, marks, fallback });
    } finally {
      Object.values(buffers).forEach((buffer) => buffer.destroy());
    }
  } finally {
    device.destroy();
  }
}

/** The fault's uniform code, or a refusal naming the ones that exist. */
function faultCodeFor(fault?: string | null): number {
  if (fault === undefined || fault === null) {
    return 0;
  }
  const found = Object.entries(COLLIDE_FAULTS).find(([name]) => name === fault);
  if (found === undefined) {
    throw new Refusal(`--break ${fault}: the collide faults are ${Object.keys(COLLIDE_FAULTS).join(", ")}`);
  }
  return found[1];
}

/** The two limits the scan needs beyond `open`'s: its workgroups and its spans' binding. */
function checkGrid(device: GPUDevice, buckets: number): void {
  const { blocks } = scanPlan(buckets);
  if (device.limits.maxComputeWorkgroupsPerDimension < blocks) {
    throw new Refusal(`maxComputeWorkgroupsPerDimension is below the scan's ${blocks} workgroups`);
  }
  if (device.limits.maxStorageBufferBindingSize < (buckets + 1) * 4) {
    throw new Refusal(`maxStorageBufferBindingSize is below the ${buckets + 1} bucket spans`);
  }
}


/** One full run: the stage's dispatches, then the three columns read back. */
async function runCollideOnce(
  device: GPUDevice,
  buffers: CollideBuffers,
  stage: CollideStage,
  sizes: { n: number; buckets: number },
): Promise<CollideRan> {
  const { n, buckets } = sizes;
  const encoder = device.createCommandEncoder();
  stage.encode(encoder);
  encoder.copyBufferToBuffer(buffers.delta, 0, buffers.readDelta, 0, n * 8);
  encoder.copyBufferToBuffer(buffers.order, 0, buffers.readOrder, 0, n * 4);
  encoder.copyBufferToBuffer(buffers.start, 0, buffers.readStart, 0, (buckets + 1) * 4);
  device.queue.submit([encoder.finish()]);
  const read = [buffers.readDelta, buffers.readOrder, buffers.readStart];
  await Promise.all(read.map((buffer) => buffer.mapAsync(GPUMapMode.READ)));
  const [delta, order, start] = read.map((buffer) => buffer.getMappedRange().slice(0));
  read.forEach((buffer) => buffer.unmap());
  if (delta === undefined || order === undefined || start === undefined) {
    throw new Refusal("gpu: a collide read-back mapped nothing");
  }
  return { delta: new Float32Array(delta, 0, n * 2), order: new Uint32Array(order, 0, n), start: new Uint32Array(start) };
}
