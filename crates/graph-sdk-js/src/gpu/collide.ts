/**
 * `collide.ts` — the collide pass on a WebGPU device: the grid the host derives, the
 * dispatches, the read-back.
 *
 * The CPU pass is `collide.rs`: a counting sort of the nodes into a hashed cell list one
 * diameter wide, then a gather over each node's nine neighbour cells. The device runs the
 * same shape in four stages, `collide_hash`, `collide_scan`, `collide_scatter` and
 * `collide_resolve`, one module each under `kernels/`.
 *
 * ## What the host derives, and why it is the host's
 *
 * The fixture carries positions and no collide parameters, so the grid comes from the frozen
 * set the emitter used (`ForceParams::default`, `gpu_fixtures/settle.rs:87-92`): radius 16, so
 * cells 32 wide. The origin is the finite positions' minimum, as `frame::bounds` folds it
 * over the at-rest projection `x + 0` (`collide.rs:146`); `f32` rounding is monotonic, so the
 * narrowed minimum is the minimum of the narrowed positions. The row table is `hash.rs:30`'s
 * 64-bit product per row, evaluated in `BigInt` (`kernels/collide.wgsl.ts` says why).
 *
 * The seven dispatches run each in its own compute pass, in `build`'s order. The counts are
 * zeroed by a queue write before every run; every other buffer is written whole by its stage.
 * The pass runs twice; the delta, the member lists and their spans come back.
 */

import { Refusal, open } from "./adapter.ts";
import { collideReport } from "./bounds-collide.ts";
import type { CollideRan } from "./bounds-collide.ts";
import type { Arm } from "./bounds.ts";
import { loadFixture } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import { COLLIDE_FAULTS } from "./kernels/collide.wgsl.ts";
import { COLLIDE_HASH_WGSL } from "./kernels/collide-hash.wgsl.ts";
import { COLLIDE_RESOLVE_WGSL } from "./kernels/collide-resolve.wgsl.ts";
import { COLLIDE_SCAN_WGSL } from "./kernels/collide-scan.wgsl.ts";
import { COLLIDE_SCATTER_WGSL } from "./kernels/collide-scatter.wgsl.ts";
import type { PassReport } from "./pass-report.ts";
import { dispatch, groupsFor } from "./pipelines.ts";
import type { GPUBindGroup, GPUBuffer, GPUComputePipeline, GPUDevice, GpuHost } from "./types.ts";
import { GPUBufferUsage, GPUMapMode } from "./types.ts";

/** What the collide arm needs from the host; no fault knob, as `ChargeRequest` has none. */
export interface CollideRequest {
  /** The `.gmfx` bytes, exactly as `emit-gpu-fixtures` wrote them. */
  readonly fixture: ArrayBuffer;
  /** Which arm to insist on. `"any"` takes what the device offers. */
  readonly arm?: Arm | "any";
  /** The object carrying `gpu`: `navigator` when omitted, a literal in a node test. */
  readonly host?: GpuHost;
}

/** The jiggle's keys: the session's `SEED` (`session.rs:77`) and the probe's tick 0. */
const [SEED, TICK] = [0, 0];

/** The collide radius at the frozen parameters (`params.rs:71`); the cell is its diameter. */
const COLLIDE_RADIUS = 16;

/** `hash.rs:30`'s multiplier. */
const ROW_HASH = 0x9e3779b97f4a7c15n;

/** The grid every collide stage reads: the uniform's numbers and the row table. */
export interface Grid {
  /** The finite positions' minimum, narrowed to `f32`. */
  readonly originX: number;
  readonly originY: number;
  /** `1 / diameter`, narrowed to `f32`. */
  readonly invSize: number;
  /** The diameter and its square (`collide.rs:262-273`). */
  readonly reach: number;
  readonly d2: number;
  readonly buckets: number;
  readonly mask: number;
  /** `rows[cy + 1]` is row `cy`'s bucket for cell 0, for `cy` in `-1..=rowMax + 1`. */
  readonly rows: Uint32Array;
}

/** The scan's workgroups: `blocks` for its first and third dispatch, `chunk` sums each in its second. */
export function scanPlan(buckets: number): { readonly blocks: number; readonly chunk: number } {
  const blocks = Math.ceil(buckets / 256);
  return { blocks, chunk: Math.ceil(blocks / 256) };
}

/** The grid over one fixture's positions. */
export function gridFor(posX: Float64Array, posY: Float64Array): Grid {
  const n = posX.length;
  const diameter = 2 * COLLIDE_RADIUS;
  const d2 = diameter * diameter;
  const reach = Math.sqrt(d2);
  const [lowX, lowY] = finiteMinimum(posX, posY);
  const originX = Math.fround(lowX);
  const originY = Math.fround(lowY);
  const invSize = Math.fround(1 / reach);
  // collide.rs:68: (2n).next_power_of_two().max(4).
  const buckets = Math.max(4, 2 ** (32 - Math.clz32(Math.max(2, 2 * n) - 1)));
  let rowMax = 0;
  for (let k = 0; k < n; k += 1) {
    const y = posY[k] ?? 0;
    if (Number.isFinite(y) && Number.isFinite(posX[k] ?? 0)) {
      rowMax = Math.max(rowMax, cellOf(Math.fround(y), originY, invSize));
    }
  }
  const rows = rowTable(rowMax, 64 - Math.log2(buckets));
  return { originX, originY, invSize, reach, d2, buckets, mask: buckets - 1, rows };
}

/**
 * The device's `axis_cell`, op for op: `i32((v - o) * inv)` on `f32` values, each operation
 * correctly rounded as WGSL's `-` and `*` are, truncated and saturated as `i32(f32)` is.
 *
 * Caveat: it agrees with the CPU's `f64` cell except at a cell edge, where the narrowed
 * position can round across it; `the_collide_hash_matches_the_cpu_s_cells` holds on every node
 * of both 1k fixtures, and a crossing elsewhere costs only the order of one sum.
 */
export function cellOf(v: number, o: number, inv: number): number {
  const u = Math.fround(Math.fround(v - o) * inv);
  if (Number.isNaN(u)) {
    return 0;
  }
  return Math.trunc(Math.min(Math.max(u, -(2 ** 31)), 2 ** 31 - 1));
}

/** `(lo x, lo y)` over the nodes whose two coordinates are finite; `(0, 0)` when none is. */
function finiteMinimum(posX: Float64Array, posY: Float64Array): [number, number] {
  let lowX = Number.POSITIVE_INFINITY;
  let lowY = Number.POSITIVE_INFINITY;
  for (let k = 0; k < posX.length; k += 1) {
    const x = posX[k] ?? 0;
    const y = posY[k] ?? 0;
    if (Number.isFinite(x) && Number.isFinite(y)) {
      lowX = x < lowX ? x : lowX;
      lowY = y < lowY ? y : lowY;
    }
  }
  return lowX === Number.POSITIVE_INFINITY ? [0, 0] : [lowX, lowY];
}

/** Rows `-1..=rowMax + 1`, each `hash.rs:30`'s `(cy as u64).wrapping_mul(K) >> shift`. */
function rowTable(rowMax: number, shift: number): Uint32Array {
  const table = new Uint32Array(rowMax + 3);
  for (let at = 0; at < table.length; at += 1) {
    const cy = BigInt.asUintN(64, BigInt(at - 1));
    table[at] = Number(BigInt.asUintN(64, cy * ROW_HASH) >> BigInt(shift));
  }
  return table;
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
    const buffers = allocate(device, fixture.n, grid);
    try {
      upload(device, buffers, fixture, { grid, code });
      const stages = build(device, buffers, fixture.n, grid.buckets);
      const sizes = { n: fixture.n, buckets: grid.buckets };
      const first = await once(device, buffers, stages, sizes);
      const second = await once(device, buffers, stages, sizes);
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

type Buffers = Readonly<Record<
  "frame" | "nodes" | "rows" | "counts" | "cells" | "bucket" | "arrival" | "start" | "sums" |
  "loose" | "order" | "delta" | "readDelta" | "readOrder" | "readStart", GPUBuffer>>;

/** Every buffer the pass uses, sized from `n` and the grid; the three read-backs `MAP_READ`. */
function allocate(device: GPUDevice, n: number, grid: Grid): Buffers {
  const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC;
  const mapped = GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST;
  const make = (label: string, size: number, usage = storage): GPUBuffer =>
    device.createBuffer({ label: `collide-${label}`, size: Math.max(4, size), usage });
  const spans = (grid.buckets + 1) * 4;
  return {
    frame: make("frame", 64, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST),
    nodes: make("nodes", n * 8),
    rows: make("rows", grid.rows.length * 4),
    counts: make("counts", grid.buckets * 4),
    cells: make("cells", n * 8),
    bucket: make("bucket", n * 4),
    arrival: make("arrival", n * 4),
    start: make("start", spans),
    sums: make("sums", scanPlan(grid.buckets).blocks * 4),
    loose: make("loose", n * 4),
    order: make("order", n * 4),
    delta: make("delta", n * 8),
    readDelta: make("read-delta", n * 8, mapped),
    readOrder: make("read-order", n * 4, mapped),
    readStart: make("read-start", spans, mapped),
  };
}

/** The positions narrowed once, the row table, and the 64-byte uniform in the prelude's order. */
function upload(device: GPUDevice, buffers: Buffers, fixture: Fixture, how: { grid: Grid; code: number }): void {
  const { grid, code } = how;
  const xy = new Float32Array(fixture.n * 2);
  for (let k = 0; k < fixture.n; k += 1) {
    xy[k * 2] = Math.fround(fixture.posX[k] ?? 0);
    xy[k * 2 + 1] = Math.fround(fixture.posY[k] ?? 0);
  }
  device.queue.writeBuffer(buffers.nodes, 0, xy);
  device.queue.writeBuffer(buffers.rows, 0, grid.rows);
  const words = new ArrayBuffer(64);
  const { blocks, chunk } = scanPlan(grid.buckets);
  new Uint32Array(words).set([fixture.n, grid.buckets, grid.mask, grid.rows.length, blocks, chunk, SEED, TICK, code]);
  new Float32Array(words).set([grid.originX, grid.originY, grid.invSize, grid.d2, grid.reach], 11);
  device.queue.writeBuffer(buffers.frame, 0, words);
}

/** One dispatch: a pipeline, its bind group, its workgroup count. */
interface Stage {
  readonly pipe: GPUComputePipeline;
  readonly bind: GPUBindGroup;
  readonly groups: number;
}

/** Every stage in dispatch order, compiled and bound once; each group lists exactly the bindings its entry point uses. */
function build(device: GPUDevice, buffers: Buffers, n: number, buckets: number): readonly Stage[] {
  const at = (index: number, key: keyof Buffers): readonly [number, GPUBuffer] => [index, buffers[key]];
  const stage = (code: string, entryPoint: string, entries: readonly (readonly [number, GPUBuffer])[], groups: number): Stage => {
    const pipe = device.createComputePipeline({
      label: entryPoint,
      layout: "auto",
      compute: { module: device.createShaderModule({ label: entryPoint, code }), entryPoint },
    });
    const layout = pipe.getBindGroupLayout(0);
    const bound = entries.map(([binding, buffer]) => ({ binding, resource: { buffer } }));
    return { pipe, bind: device.createBindGroup({ label: entryPoint, layout, entries: bound }), groups };
  };
  const nodes = groupsFor(n);
  const { blocks } = scanPlan(buckets);
  const frame = at(0, "frame");
  const scan = [frame, at(7, "start"), at(8, "sums")];
  return [
    stage(COLLIDE_HASH_WGSL, "collide_hash", [frame, at(1, "nodes"), at(2, "rows"), at(3, "counts"), at(4, "cells"), at(5, "bucket"), at(6, "arrival")], nodes),
    stage(COLLIDE_SCAN_WGSL, "scan_blocks", [frame, at(3, "counts"), at(7, "start"), at(8, "sums")], blocks),
    stage(COLLIDE_SCAN_WGSL, "scan_sums", scan, 1),
    stage(COLLIDE_SCAN_WGSL, "scan_add", scan, blocks),
    stage(COLLIDE_SCATTER_WGSL, "collide_place", [frame, at(5, "bucket"), at(6, "arrival"), at(7, "start"), at(9, "loose")], nodes),
    stage(COLLIDE_SCATTER_WGSL, "collide_scatter", [frame, at(5, "bucket"), at(7, "start"), at(9, "loose"), at(10, "order")], nodes),
    stage(COLLIDE_RESOLVE_WGSL, "collide_resolve", [frame, at(1, "nodes"), at(2, "rows"), at(4, "cells"), at(7, "start"), at(10, "order"), at(11, "delta")], nodes),
  ];
}

/** One full run: zero the counts, every stage in order, then the three columns read back. */
async function once(device: GPUDevice, buffers: Buffers, stages: readonly Stage[], sizes: { n: number; buckets: number }): Promise<CollideRan> {
  const { n, buckets } = sizes;
  device.queue.writeBuffer(buffers.counts, 0, new Uint32Array(buckets));
  const encoder = device.createCommandEncoder();
  for (const { pipe, bind, groups } of stages) {
    const pass = encoder.beginComputePass();
    dispatch(pass, pipe, bind, groups);
    pass.end();
  }
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
