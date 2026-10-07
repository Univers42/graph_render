/**
 * `charge-kernel.ts` — the kernel refresh, for a frame that leaves the fixture's rung.
 *
 * The fixture's spectrum is the CPU's kernel for the fixture's own frame (`kernel.rs:42-63`):
 * the law `G(r) = -r / l` sampled at every cell offset within the frame's reach, then forward
 * transformed. Its samples sit `h` apart, so it is the right kernel only while the frame keeps
 * the fixture's rung and reach. The CPU rebuilds it whenever either changes, and so does the
 * tick: the sampling is `kernel.rs:66-92` on the host in `f64`, narrowed once on upload; the
 * transform is the stage's own two forward passes at `live = side`, as `Kernel::refresh` runs
 * `fft.forward(.., side)`; the result is copied into `gain`.
 *
 * Caveat: the device transforms `f32` samples, where the CPU transforms `f64` ones and narrows
 * the spectrum once, so a refreshed spectrum carries about `log₂P` roundings instead of one. A
 * tick that stays on the fixture's rung never refreshes and keeps the fixture's spectrum.
 */

import { Refusal } from "./adapter.ts";
import type { Buffers } from "./buffers.ts";
import { frameWords } from "./charge-upload.ts";
import type { Placement } from "./charge-upload.ts";
import type { Fixture } from "./fixture.ts";
import { dispatch } from "./pipelines.ts";
import type { Rig } from "./pipelines.ts";
import type { GPUDevice } from "./types.ts";

/** The law's squared cutoffs at the frozen parameters: `distance_min = 1`, `distance_max = 520`. */
const DMIN2 = 1;
const DMAX2 = 520 * 520;

/** The frame fields the kernel depends on. */
export interface KernelFrame {
  readonly h: number;
  readonly reach: number;
}

/**
 * `kernel::sample`: zeroes, then `G` at every offset within `reach`, wrapped modulo `side` and
 * scaled by `1/side²`, as interleaved `f64` pairs — the real part is `Gx`, the imaginary part
 * `Gy`. The upload narrows them, once.
 */
export function sampleKernel(side: number, frame: KernelFrame): Float64Array {
  const g = new Float64Array(side * side * 2);
  const scale = 1 / (side * side);
  for (let dy = -frame.reach; dy <= frame.reach; dy += 1) {
    const row = wrap(dy, side) * side;
    for (let dx = -frame.reach; dx <= frame.reach; dx += 1) {
      const at = (row + wrap(dx, side)) * 2;
      const [re, im] = green(dx * frame.h, dy * frame.h, scale);
      g[at] = re;
      g[at + 1] = im;
    }
  }
  return g;
}

/**
 * Samples the kernel for `at`, transforms it with the stage's two forward passes, and copies
 * it into `gain`, in a submit of its own. The caller rewrites the pass uniforms afterwards.
 */
export function refreshKernel(device: GPUDevice, buffers: Buffers, rig: Rig, at: { fixture: Fixture; place: Placement & KernelFrame }): void {
  const { fixture, place } = at;
  const { side } = fixture;
  device.queue.writeBuffer(buffers.field, 0, new Float32Array(sampleKernel(side, place)));
  const encoder = device.createCommandEncoder();
  const passes: readonly (readonly [number, number])[] = [[0, 0], [1, 2]];
  for (const [index, mode] of passes) {
    const uniform = buffers.framePass[index];
    const bind = rig.fftBind[index];
    if (uniform === undefined || bind === undefined) {
      throw new Refusal(`gpu: no transform pass ${index} for the kernel refresh`);
    }
    device.queue.writeBuffer(uniform, 0, frameWords(fixture, place, { live: side, mode, inverse: 0 }));
    const pass = encoder.beginComputePass();
    dispatch(pass, rig.fft, bind, side);
    pass.end();
  }
  encoder.copyBufferToBuffer(buffers.field, 0, buffers.gain, 0, side * side * 8);
  device.queue.submit([encoder.finish()]);
}

/** `kernel::green`: `G(r) · scale`, zero at the origin and past `dmax`, softened under `dmin`. */
function green(rx: number, ry: number, scale: number): [number, number] {
  let l = rx * rx + ry * ry;
  if (l === 0 || l >= DMAX2) {
    return [0, 0];
  }
  if (l < DMIN2) {
    l = Math.sqrt(DMIN2 * l);
  }
  return [(-rx / l) * scale, (-ry / l) * scale];
}

/** `rem_euclid`: `v mod side` in `0..side`. */
function wrap(v: number, side: number): number {
  return ((v % side) + side) % side;
}
