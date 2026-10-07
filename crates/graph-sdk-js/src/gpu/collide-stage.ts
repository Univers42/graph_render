/**
 * `collide-stage.ts` — the collide pass's device half: the buffers, the uploads and the seven
 * dispatches, built once per graph and encoded into any command encoder.
 *
 * The seven dispatches run each in its own compute pass, in `build`'s order. The counts are
 * zeroed by a queue write before every run; every other buffer is written whole by its stage.
 * `regrid` uploads a grid and the jiggle's tick: the probe's grid is the fixture's, once; the
 * tick's is derived from the read-back of the positions the collide reads, every tick.
 *
 * The row table's buffer starts at `ROW_SLACK` times the first grid's rows. A layout that grows
 * taller than that gets a new table, `ROW_SLACK` times the new grid's rows, and the seven bind
 * groups are rebuilt over it — a live layout spreads for hundreds of ticks, so a refusal here
 * would end a session mid-run (`docs/decisions/gpu-g1d.md`). The stage owns that grown buffer,
 * and `destroy` frees it.
 */

import { Refusal } from "./adapter.ts";
import { gridFor, scanPlan } from "./collide-grid.ts";
import type { Grid } from "./collide-grid.ts";
import type { Fixture } from "./fixture.ts";
import { FROZEN_LAW, lawOf } from "./law.ts";
import { COLLIDE_HASH_WGSL } from "./kernels/collide-hash.wgsl.ts";
import { COLLIDE_RESOLVE_WGSL } from "./kernels/collide-resolve.wgsl.ts";
import { COLLIDE_SCAN_WGSL } from "./kernels/collide-scan.wgsl.ts";
import { COLLIDE_SCATTER_WGSL } from "./kernels/collide-scatter.wgsl.ts";
import { dispatch, groupsFor } from "./pipelines.ts";
import type { GPUBindGroup, GPUBuffer, GPUCommandEncoder, GPUComputePipeline, GPUDevice } from "./types.ts";
import { GPUBufferUsage } from "./types.ts";

/** The jiggle's seed: the session's `SEED` (`session.rs:77`). */
const SEED = 0;

/** How many times the first grid's rows the row table's buffer holds. */
const ROW_SLACK = 4;

/**
 * The collide stage: built once per graph, encodes its dispatches into a given encoder.
 *
 * The stage reads `nodes` and writes the per-node collide increment. The tick projects
 * `x + v` into `nodes` before it runs and merges the increment into the velocities inside the
 * integrate (`motion.rs:152-154`); the probe reads the increment back for the report.
 */
export interface CollideStage {
  /** The per-node collide increment the resolve writes, `2n` components. */
  readonly delta: GPUBuffer;
  /** The positions the stage reads, `2n` components. */
  readonly nodes: GPUBuffer;
  /** Uploads a grid over the positions in `nodes`, and the tick the jiggle is keyed on. */
  regrid(grid: Grid, tick: number): void;
  encode(encoder: GPUCommandEncoder): void;
  /** Frees the row table a `regrid` grew, if one did; `buffers.rows` stays the caller's. */
  destroy(): void;
}

/** The buffers the collide stage reads and writes, built once per graph. */
export type CollideBuffers = Readonly<Record<
  "frame" | "nodes" | "rows" | "counts" | "cells" | "bucket" | "arrival" | "start" | "sums" |
  "loose" | "order" | "delta" | "readDelta" | "readOrder" | "readStart", GPUBuffer>>;

/**
 * Builds the collide stage: the fixture's positions and grid, the pipelines, the bind groups.
 *
 * The positions uploaded here are the fixture's own, for the probe; the tick overwrites them
 * every tick with its projection.
 */
export function buildCollideStage(
  device: GPUDevice,
  buffers: CollideBuffers,
  fixture: Fixture,
  faultCode: number,
): CollideStage {
  const grid = gridFor(fixture.posX, fixture.posY, radiusOf(fixture));
  const xy = new Float32Array(fixture.n * 2);
  for (let k = 0; k < fixture.n; k += 1) {
    xy[k * 2] = Math.fround(fixture.posX[k] ?? 0);
    xy[k * 2 + 1] = Math.fround(fixture.posY[k] ?? 0);
  }
  device.queue.writeBuffer(buffers.nodes, 0, xy);
  let bound = buffers;
  let stages = build(device, bound, fixture.n, grid.buckets);
  const regrid = (next: Grid, tick: number): void => {
    if (next.rows.byteLength > bound.rows.size) {
      if (bound.rows !== buffers.rows) bound.rows.destroy();
      bound = { ...buffers, rows: rowBuffer(device, next.rows.length * ROW_SLACK) };
      stages = build(device, bound, fixture.n, next.buckets);
    }
    uploadGrid(device, bound, next, [fixture.n, tick, faultCode]);
  };
  regrid(grid, 0);
  return {
    delta: buffers.delta,
    nodes: buffers.nodes,
    regrid,
    destroy(): void {
      if (bound.rows !== buffers.rows) bound.rows.destroy();
    },
    encode(encoder: GPUCommandEncoder): void {
      device.queue.writeBuffer(buffers.counts, 0, new Uint32Array(grid.buckets));
      for (const { pipe, bind, groups } of stages) {
        const pass = encoder.beginComputePass();
        dispatch(pass, pipe, bind, groups);
        pass.end();
      }
    },
  };
}

/** Every buffer the pass uses, sized from `n` and the grid; the three read-backs `MAP_READ`. */
export function allocateCollide(device: GPUDevice, n: number, grid: Grid): CollideBuffers {
  const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC;
  const mapped = GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST;
  const make = (label: string, size: number, usage = storage): GPUBuffer =>
    device.createBuffer({ label: `collide-${label}`, size: Math.max(4, size), usage });
  const spans = (grid.buckets + 1) * 4;
  return {
    frame: make("frame", 64, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST),
    nodes: make("nodes", n * 8),
    rows: make("rows", grid.rows.length * 4 * ROW_SLACK),
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

/**
 * The radius a rig over `fixture` sizes its collide grid at (`collide.rs:262`): the law's, or
 * the frozen one when the law's is `0` — the CPU's skip, where the tick never runs the stage
 * and a grid at `0` would saturate every row index (`collide-grid.ts`).
 */
export function radiusOf(fixture: Fixture): number {
  const radius = lawOf(fixture).collide_radius;
  return radius > 0 ? radius : FROZEN_LAW.collide_radius;
}

/** A row table of `rows` words, as `allocateCollide` makes one. */
function rowBuffer(device: GPUDevice, rows: number): GPUBuffer {
  const usage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC;
  return device.createBuffer({ label: "collide-rows", size: Math.max(4, rows * 4), usage });
}

/** The row table and the 64-byte uniform in the prelude's order; `keys` is `n`, the tick, the fault. */
function uploadGrid(device: GPUDevice, buffers: CollideBuffers, grid: Grid, keys: readonly [number, number, number]): void {
  if (grid.rows.byteLength > buffers.rows.size) {
    throw new Refusal(`gpu: the collide row table holds ${buffers.rows.size / 4} rows and the grid has ${grid.rows.length}`);
  }
  const [n, tick, code] = keys;
  device.queue.writeBuffer(buffers.rows, 0, grid.rows);
  const words = new ArrayBuffer(64);
  const { blocks, chunk } = scanPlan(grid.buckets);
  new Uint32Array(words).set([n, grid.buckets, grid.mask, grid.rows.length, blocks, chunk, SEED, tick, code]);
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
function build(device: GPUDevice, buffers: CollideBuffers, n: number, buckets: number): readonly Stage[] {
  const at = (index: number, key: keyof CollideBuffers): readonly [number, GPUBuffer] => [index, buffers[key]];
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
