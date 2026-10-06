/**
 * `link.ts` — the link pass on the device: the host's CSR, the uploads, two runs, the report.
 *
 * The kernel is `kernels/link.wgsl.ts`, one invocation per node summing its own row. This file
 * builds that row once on the host, narrows every per-edge constant once, dispatches the
 * gather twice, and hands both read-backs to `bounds-link.ts`.
 *
 * ## The CSR is graph-core's, rebuilt from the fixture's edges
 *
 * `row_csr` (`layout/force/mod.rs:177-180`) files every simple edge under both ends in
 * ascending edge index, and `LinkPass::node_share` sums a node's terms in that order
 * (`barnes_hut/step.rs:177-189`). `linkCsr` is the same counting sort: a count per node, a
 * prefix sum, then the edges in index order — so each row comes out ascending without a sort.
 *
 * ## The per-edge constants, narrowed once
 *
 * `edge_geometry` (`barnes_hut/link.rs:47-63`) under the frozen parameters
 * (`params.rs:69-70`): `distance = 60 / max(0.4, s)`, `strength = min(0.7, 0.15 · s)`, and
 * the bias `b = deg(lo) / (deg(lo) + deg(hi))` over the simple graph's degrees, which are the
 * CSR's row lengths. All four — with `1 - b` — are computed in `f64` and narrowed once, by
 * the `Float32Array` store on upload (the same rounding as `Math.fround`), as the positions
 * are; the device never rounds a constant the CPU computed exactly.
 * `alpha` is 1 at the fixture's state (`fixtures/gpu/README.md`), so `strength` carries it.
 *
 * ## The whole pass runs twice
 *
 * `repeatEqual` compares the two runs' `delta` columns as bytes. The gather has no atomics and
 * no reduction across invocations, so two runs that differ are a defect in the dispatch or the
 * read-back, not a rounding.
 */

import { Refusal, open } from "./adapter.ts";
import type { Arm } from "./bounds.ts";
import { linkReport } from "./bounds-link.ts";
import { loadFixture } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import { LINK_FAULT_BIAS, LINK_FRAME_BYTES, LINK_WGSL } from "./kernels/link.wgsl.ts";
import type { PassReport } from "./pass-report.ts";
import { dispatch, groupsFor } from "./pipelines.ts";
import { GPUBufferUsage, GPUMapMode } from "./types.ts";
import type { GPUBindGroup, GPUBuffer, GPUComputePipeline, GPUDevice, GpuHost } from "./types.ts";

/** What the link arm needs from the host. No fault knob: the harness passes it separately. */
export interface LinkRequest {
  /** The `.gmfx` bytes, exactly as `emit-gpu-fixtures` wrote them. */
  readonly fixture: ArrayBuffer;
  /** Which arm to insist on. `"any"` takes what the device offers. */
  readonly arm?: Arm | "any";
  /** The object carrying `gpu`: `navigator` in a page when omitted, a literal in a test. */
  readonly host?: GpuHost;
}

/** Each node's incident simple edges: row `i` is `edges[start[i] .. start[i + 1]]`. */
export interface LinkCsr {
  readonly start: Uint32Array;
  readonly edges: Uint32Array;
}

/** The frozen `link_distance` and `link_strength_scale` (`params.rs:69-70`). */
const LINK_DISTANCE = 60;
const LINK_STRENGTH_SCALE = 0.15;

/** The `--break` faults this pass knows, by the uniform code each sets. */
const FAULTS: Readonly<Record<string, number>> = { "link-bias": LINK_FAULT_BIAS };

/** The device's buffers and the one pipeline, built once and used by both runs. */
interface Rig {
  readonly buffers: readonly GPUBuffer[];
  readonly delta: GPUBuffer;
  readonly read: GPUBuffer;
  readonly pipeline: GPUComputePipeline;
  readonly bind: GPUBindGroup;
}

/**
 * The link pass for one fixture: the gather twice, then the comparison against `delta_link`.
 *
 * `fault` is a parameter and not a request field, for `charge-api.ts:1-13`'s reason: the
 * public `probeLink` cannot inject one. The harness passes `undefined` or a `link-` name.
 */
export async function runLink(request: LinkRequest, fault?: string): Promise<PassReport> {
  const fixture = loadFixture(request.fixture);
  const code = linkFaultCode(fault);
  const host: GpuHost = request.host ?? navigator;
  const { device, marks, fallback } = await open(host, request.arm ?? "any", fixture);
  try {
    const rig = build(device, fixture, code);
    try {
      const first = await once(device, rig, fixture.n);
      const second = await once(device, rig, fixture.n);
      const arm = armOf(request.arm ?? "any", fallback);
      return linkReport(fixture, { first, second, arm, marks, fallback });
    } finally {
      rig.buffers.forEach((buffer) => buffer.destroy());
    }
  } finally {
    device.destroy();
  }
}

/** The fault's uniform code, `0` for none, or a refusal naming the ones that exist. */
function linkFaultCode(fault?: string | null): number {
  if (fault === undefined || fault === null) {
    return 0;
  }
  const code = FAULTS[fault];
  if (code === undefined) {
    throw new Refusal(`--break ${fault}: the link faults are ${Object.keys(FAULTS).sort().join(", ")}`);
  }
  return code;
}

/** The ceiling row an arm is held to: the one asked for, else what the adapter said it was. */
function armOf(asked: Arm | "any", fallback: boolean): Arm {
  if (asked !== "any") {
    return asked;
  }
  return fallback ? "software" : "hardware";
}

/** `row_csr`'s counting sort: every edge under both ends, each row in ascending edge index. */
export function linkCsr(lo: Uint32Array, hi: Uint32Array, n: number): LinkCsr {
  const start = new Uint32Array(n + 1);
  for (const ends of [lo, hi]) {
    for (const node of ends) {
      start[node + 1] = (start[node + 1] ?? 0) + 1;
    }
  }
  for (let node = 0; node < n; node += 1) {
    start[node + 1] = (start[node + 1] ?? 0) + (start[node] ?? 0);
  }
  const next = start.slice(0, n);
  const edges = new Uint32Array(start[n] ?? 0);
  for (let edge = 0; edge < lo.length; edge += 1) {
    for (const node of [lo[edge] ?? 0, hi[edge] ?? 0]) {
      edges[next[node] ?? 0] = edge;
      next[node] = (next[node] ?? 0) + 1;
    }
  }
  return { start, edges };
}

/** Per edge `(lo, hi)` and `(distance, strength, b, 1 - b)`, computed in `f64`, narrowed once. */
function edgeTables(fixture: Fixture, csr: LinkCsr): { ends: Uint32Array; geometry: Float32Array } {
  const { m, edgeLo, edgeHi, edgeStrength } = fixture;
  const degree = (node: number): number => (csr.start[node + 1] ?? 0) - (csr.start[node] ?? 0);
  const ends = new Uint32Array(m * 2);
  const geometry = new Float32Array(m * 4);
  for (let edge = 0; edge < m; edge += 1) {
    const lo = edgeLo[edge] ?? 0;
    const hi = edgeHi[edge] ?? 0;
    const s = edgeStrength[edge] ?? 0;
    const b = degree(lo) / (degree(lo) + degree(hi));
    ends.set([lo, hi], edge * 2);
    geometry.set([LINK_DISTANCE / Math.max(0.4, s), Math.min(0.7, LINK_STRENGTH_SCALE * s), b, 1 - b], edge * 4);
  }
  return { ends, geometry };
}

/** Every buffer filled and the gather bound, once per device. */
function build(device: GPUDevice, fixture: Fixture, faultCode: number): Rig {
  const csr = linkCsr(fixture.edgeLo, fixture.edgeHi, fixture.n);
  const { ends, geometry } = edgeTables(fixture, csr);
  const positions = new Float32Array(fixture.n * 2);
  for (let node = 0; node < fixture.n; node += 1) {
    positions.set([fixture.posX[node] ?? 0, fixture.posY[node] ?? 0], node * 2);
  }
  const frame = new Uint32Array([fixture.n, faultCode, 0, 0]);
  const inputs = [frame, positions, csr.start, csr.edges, ends, geometry].map((data, binding) =>
    uploaded(device, data, binding === 0 ? GPUBufferUsage.UNIFORM : GPUBufferUsage.STORAGE));
  const bytes = fixture.n * 8;
  const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC;
  const delta = device.createBuffer({ label: "link-delta", size: bytes, usage: storage });
  const mapped = GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST;
  const read = device.createBuffer({ label: "link-read", size: bytes, usage: mapped });
  const module = device.createShaderModule({ label: "link", code: LINK_WGSL });
  const pipeline = device.createComputePipeline({
    label: "link",
    layout: "auto",
    compute: { module, entryPoint: "link_gather" },
  });
  const entries = [...inputs, delta].map((buffer, binding) => ({ binding, resource: { buffer } }));
  const bind = device.createBindGroup({ label: "link", layout: pipeline.getBindGroupLayout(0), entries });
  return { buffers: [...inputs, delta, read], delta, read, pipeline, bind };
}

/**
 * One buffer holding `data`, `COPY_DST` added for the upload.
 *
 * At least the uniform's 16 bytes, because a zero-length binding is a validation error and a
 * fixture with no edges would otherwise ask for one; the gather never reads past `n`'s rows.
 */
function uploaded(device: GPUDevice, data: Uint32Array | Float32Array, usage: number): GPUBuffer {
  const size = Math.max(LINK_FRAME_BYTES, data.byteLength);
  const buffer = device.createBuffer({ label: "link-input", size, usage: usage | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(buffer, 0, data);
  return buffer;
}

/** One run: the gather, the copy, the mapped read-back of `2n` components. */
async function once(device: GPUDevice, rig: Rig, n: number): Promise<Float32Array> {
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  dispatch(pass, rig.pipeline, rig.bind, groupsFor(n));
  pass.end();
  encoder.copyBufferToBuffer(rig.delta, 0, rig.read, 0, n * 8);
  device.queue.submit([encoder.finish()]);
  await rig.read.mapAsync(GPUMapMode.READ);
  const out = new Float32Array(rig.read.getMappedRange().slice(0, n * 8));
  rig.read.unmap();
  return out;
}
