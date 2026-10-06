/**
 * `buffers.ts` — the buffers the charge pass uses, and their sizes.
 *
 * Everything is derived from three numbers the fixture header carries: `n`, `P` and `m`. No
 * length here is written out, because a length written out is a length that is wrong at the
 * next `n`. The sizes below are what the pass touches at 1M (`P = 1024`, `n = 10⁶`), and they
 * are all inside the software arm's 1 GiB `maxStorageBufferBindingSize`
 * (`gpu-adapter.md:118`).
 *
 * | buffer | size at 1M | what it is |
 * |---|---:|---|
 * | `nodes` | 8 MB | the positions, `f32`, narrowed from the fixture's `f64` once |
 * | `density` | 4 MB | the fixed-point density, `i32`, `P·P` |
 * | `field` | 8 MB | the `f32` complex working column, `P·P` |
 * | `scratch` | 8 MB | the transpose target, `P·P` |
 * | `gain` | 8 MB | the kernel spectrum, `f32`, narrowed once, `P·P` |
 * | `twiddle` | 8 KB | the plan's forward table, `f32`, narrowed once, `P` |
 * | `delta` | 8 MB | the arm's own per-node increment, `f32`, `n` |
 * | `boxes` | 64 KB | the bounds fold's per-block boxes, `⌈n/256⌉` |
 * | `extent` | 16 B | the folded extent, four `f32` |
 *
 * **Every narrowing happens once, on upload.** The positions, the twiddle table and the
 * spectrum are each narrowed a single time by `Math.fround` before they reach the device, and
 * the transforms then consume `f32` that has been through exactly one rounding. Narrowing a
 * twiddle inside the butterfly instead would apply `log₂P` roundings per line instead of one
 * per table.
 *
 * `field` and `scratch` are two buffers rather than one because the four passes alternate:
 * rows writes `a→b`, columns writes `b→a`, and a single buffer would need every pass to be
 * two-phase — a barrier and a copy per pass — for no saving worth the hazard.
 */

import { GPUBufferUsage } from "./types.ts";
import type { GPUBuffer, GPUDevice } from "./types.ts";

/** The eight buffers, named for the kernels that write them. */
export interface Buffers {
  readonly nodes: GPUBuffer;
  readonly density: GPUBuffer;
  readonly field: GPUBuffer;
  readonly scratch: GPUBuffer;
  readonly gain: GPUBuffer;
  readonly twiddle: GPUBuffer;
  readonly delta: GPUBuffer;
  readonly boxes: GPUBuffer;
  readonly extent: GPUBuffer;
  readonly frame: GPUBuffer;
  readonly framePass: readonly GPUBuffer[];
  readonly read: { readonly density: GPUBuffer; readonly delta: GPUBuffer; readonly extent: GPUBuffer };
}

/** `P·P` complex `f32` samples: the field, the scratch and the gain are each this size. */
function planeBytes(side: number): number {
  return side * side * 8;
}

/** The per-node columns: the positions in, the increments out. */
function nodeBytes(n: number): number {
  return n * 8;
}

/** `⌈n/256⌉` boxes of four `f32`. */
function boxBytes(n: number): number {
  return Math.ceil(n / 256) * 16;
}

/**
 * Creates every buffer the pass needs, storage-only except the read-backs.
 *
 * The `read` buffers are `MAP_READ` and therefore `COPY_DST` and nothing else, which is the
 * rule that keeps a mapped buffer from also being a kernel's output.
 */
export function create(device: GPUDevice, n: number, side: number, passes: number): Buffers {
  const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC;
  const plane = planeBytes(side);
  const mapped = GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST;
  const uniform = GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST;
  return {
    nodes: device.createBuffer({ label: "nodes", size: nodeBytes(n), usage: storage }),
    density: device.createBuffer({ label: "density", size: side * side * 4, usage: storage }),
    field: device.createBuffer({ label: "field", size: plane, usage: storage }),
    scratch: device.createBuffer({ label: "scratch", size: plane, usage: storage }),
    gain: device.createBuffer({ label: "gain", size: plane, usage: storage }),
    twiddle: device.createBuffer({ label: "twiddle", size: side * 8, usage: storage }),
    delta: device.createBuffer({ label: "delta", size: nodeBytes(n), usage: storage }),
    boxes: device.createBuffer({ label: "boxes", size: boxBytes(n), usage: storage }),
    extent: device.createBuffer({ label: "extent", size: 16, usage: storage }),
    frame: device.createBuffer({ label: "frame", size: 64, usage: uniform }),
    framePass: Array.from({ length: passes }, (_, index) =>
      device.createBuffer({ label: `frame-pass-${index}`, size: 64, usage: uniform })),
    read: {
      density: device.createBuffer({ label: "read-density", size: side * side * 4, usage: mapped }),
      delta: device.createBuffer({ label: "read-delta", size: nodeBytes(n), usage: mapped }),
      extent: device.createBuffer({ label: "read-extent", size: 16, usage: mapped }),
    },
  };
}

/** Destroys every buffer, in one pass, for the `finally` of a run. */
export function destroy(buffers: Buffers): void {
  const all: GPUBuffer[] = [
    buffers.nodes, buffers.density, buffers.field, buffers.scratch, buffers.gain,
    buffers.twiddle, buffers.delta, buffers.boxes, buffers.extent, buffers.frame,
    ...buffers.framePass, buffers.read.density, buffers.read.delta, buffers.read.extent,
  ];
  for (const buffer of all) {
    buffer.destroy();
  }
}