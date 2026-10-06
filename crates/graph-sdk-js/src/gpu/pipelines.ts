/**
 * `pipelines.ts` — the compiled kernels and their bind groups.
 *
 * It is a separate file from `charge.ts` because the two have different failure modes and
 * different lifetimes: a pipeline is compiled once per device and reused by both runs, and a
 * bind group is a *claim* about which buffers a shader may read. Keeping the claims next to the
 * code that makes them is what keeps a wrong binding from being a wrong answer.
 *
 * ## The binding numbers are the shader's, not the argument list's
 *
 * `layout: "auto"` derives a pipeline's bind group layout from the bindings its entry point
 * *uses*, so a group must carry exactly those: an entry the entry point does not use is a
 * validation error, not a harmless extra, and a gap in the numbering is a different error again.
 * `fold_extent` and `widen_density` skip `nodes`, so their groups are written out with the
 * numbers the WGSL gives them rather than filled in from position.
 *
 * The four transform bind groups differ only in which of `field` and `scratch` is `src` and
 * which is `dst`, because the CPU's two transforms alternate: rows writes `a→b`, columns
 * writes `b→a` (`fft/pass.rs:112-125`). Getting one of the four backwards is a transposed
 * answer that still looks like an answer, which is why they are written out and not looped.
 */

import { Refusal } from "./adapter.ts";
import type { Buffers } from "./buffers.ts";
import { BOUNDS_WGSL } from "./kernels/bounds.wgsl.ts";
import { DEPOSIT_WGSL } from "./kernels/deposit.wgsl.ts";
import { FFT_WGSL } from "./kernels/fft.wgsl.ts";
import { READ_WGSL } from "./kernels/read.wgsl.ts";
import { ZERO_WGSL } from "./kernels/zero.wgsl.ts";
import type {
  GPUBindGroup,
  GPUBuffer,
  GPUComputePassEncoder,
  GPUComputePipeline,
  GPUDevice,
} from "./types.ts";

/** The compiled kernels and every bind group, built once per device. */
export interface Rig {
  readonly zero: GPUComputePipeline;
  readonly block: GPUComputePipeline;
  readonly extent: GPUComputePipeline;
  readonly deposit: GPUComputePipeline;
  readonly widen: GPUComputePipeline;
  readonly fft: GPUComputePipeline;
  readonly read: GPUComputePipeline;
  readonly zeroBind: GPUBindGroup;
  readonly blockBind: GPUBindGroup;
  readonly extentBind: GPUBindGroup;
  readonly depositBind: GPUBindGroup;
  readonly widenBind: GPUBindGroup;
  /** One per transform pass: `a→b`, `b→a`, `a→b`, `b→a`. */
  readonly fftBind: readonly GPUBindGroup[];
  readonly readBind: GPUBindGroup;
}

/** Compiles every kernel and binds every group, once. */
export function build(device: GPUDevice, buffers: Buffers): Rig {
  const zero = pipeline(device, ZERO_WGSL, "zero_density", "zero");
  const bounds = pipeline(device, BOUNDS_WGSL, "fold_block", "block");
  const extent = pipeline(device, BOUNDS_WGSL, "fold_extent", "extent");
  const deposit = pipeline(device, DEPOSIT_WGSL, "deposit", "deposit");
  const widen = pipeline(device, DEPOSIT_WGSL, "widen_density", "widen");
  const fft = pipeline(device, FFT_WGSL, "fft_line", "fft");
  const read = pipeline(device, READ_WGSL, "read_field", "read");
  const frame = buffers.frame;
  const nodes = buffers.nodes;
  const binding = (
    pipe: GPUComputePipeline,
    entries: readonly (readonly [number, GPUBuffer])[],
  ): GPUBindGroup =>
    device.createBindGroup({
      label: pipe.label ?? "bind",
      layout: pipe.getBindGroupLayout(0),
      entries: entries.map(([index, buffer]) => ({ binding: index, resource: { buffer } })),
    });
  const passUniform = (index: number): GPUBuffer => {
    const buffer = buffers.framePass[index];
    if (buffer === undefined) {
      throw new Refusal(`gpu: no uniform for transform pass ${index}`);
    }
    return buffer;
  };
  return {
    zero,
    block: bounds,
    extent,
    deposit,
    widen,
    fft,
    read,
    zeroBind: binding(zero, [[0, frame], [1, buffers.density]]),
    blockBind: binding(bounds, [[0, frame], [1, nodes], [2, buffers.boxes]]),
    extentBind: binding(extent, [[0, frame], [2, buffers.boxes], [3, buffers.extent]]),
    depositBind: binding(deposit, [[0, frame], [1, nodes], [2, buffers.density]]),
    widenBind: binding(widen, [[0, frame], [2, buffers.density], [3, buffers.field]]),
    fftBind: [
      binding(fft, [[0, passUniform(0)], [1, buffers.gain], [2, buffers.field], [3, buffers.scratch], [4, buffers.twiddle]]),
      binding(fft, [[0, passUniform(1)], [1, buffers.gain], [2, buffers.scratch], [3, buffers.field], [4, buffers.twiddle]]),
      binding(fft, [[0, passUniform(2)], [1, buffers.gain], [2, buffers.field], [3, buffers.scratch], [4, buffers.twiddle]]),
      binding(fft, [[0, passUniform(3)], [1, buffers.gain], [2, buffers.scratch], [3, buffers.field], [4, buffers.twiddle]]),
    ],
    readBind: binding(read, [[0, frame], [1, nodes], [2, buffers.field], [3, buffers.delta]]),
  };
}

/** One compute pipeline from one WGSL module and one entry point. */
function pipeline(device: GPUDevice, code: string, entryPoint: string, label: string): GPUComputePipeline {
  return device.createComputePipeline({
    label,
    layout: "auto",
    compute: { module: device.createShaderModule({ label, code }), entryPoint },
  });
}

/** One dispatch: set the pipeline and the group, then dispatch `groups` workgroups of 256. */
export function dispatch(
  pass: GPUComputePassEncoder,
  pipe: GPUComputePipeline,
  bind: GPUBindGroup,
  groups: number,
): void {
  pass.setPipeline(pipe);
  pass.setBindGroup(0, bind);
  pass.dispatchWorkgroups(groups, 1, 1);
}

/** `⌈count/256⌉` workgroups, the unit every stage's dispatch is counted in. */
export function groupsFor(count: number): number {
  return Math.ceil(count / 256);
}