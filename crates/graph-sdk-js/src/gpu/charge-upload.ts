/**
 * `charge-upload.ts` — the charge pass's uploads: the constants once, the five uniforms per frame.
 *
 * Every narrowing in the pass happens here, once. The five uniforms are the `frame` and one per
 * transform pass. They share every field but `live`, `mode` and `inverse`, and two of the four
 * `live` counts are the frame's `cells` (`fft/pass.rs:112-125`), so a frame that moves rewrites
 * all five: the tick does it every tick, the probe once.
 */

import { Refusal } from "./adapter.ts";
import type { Buffers } from "./buffers.ts";
import { scaleFor } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import type { GPUDevice } from "./types.ts";

/** Where the deposit lands, the charge it carries (`charge · alpha`, `charge.rs:44`), the fault. */
export interface Placement {
  readonly cells: number;
  readonly h: number;
  readonly originX: number;
  readonly originY: number;
  readonly charge: number;
  readonly fault: number;
}

/** One transform pass's own fields. */
export interface PassFields {
  readonly live: number;
  readonly mode: number;
  readonly inverse: number;
}

/** The positions, the twiddle table and the kernel spectrum, each narrowed to `f32` once. */
export function uploadConstants(device: GPUDevice, buffers: Buffers, fixture: Fixture): void {
  const { side } = fixture;
  device.queue.writeBuffer(buffers.nodes, 0, pairs(fixture.posX, fixture.posY));
  device.queue.writeBuffer(buffers.twiddle, 0, pairs(fixture.twiddleRe, fixture.twiddleIm));
  const gain = new Float32Array(side * side * 2);
  for (let k = 0; k < side * side; k += 1) {
    gain[k * 2] = Math.fround(fixture.spectrumRe[k] ?? 0);
    gain[k * 2 + 1] = Math.fround(fixture.spectrumIm[k] ?? 0);
  }
  device.queue.writeBuffer(buffers.gain, 0, gain);
}

/**
 * The stage uniforms for one placement. The four transforms each get their own buffer, because
 * the CPU's two transforms take a different `live` count for each of their two passes: `cells`
 * for the forward rows and the inverse columns, `side` for the other two.
 */
export function writeFrames(device: GPUDevice, buffers: Buffers, fixture: Fixture, at: Placement): void {
  device.queue.writeBuffer(buffers.frame, 0, frameWords(fixture, at, { live: at.cells, mode: 0, inverse: 0 }));
  const plan: readonly (readonly [number, number])[] = [
    [0, at.cells],
    [2, fixture.side],
    [1, fixture.side],
    [2, at.cells],
  ];
  plan.forEach(([mode, live], index) => {
    const buffer = buffers.framePass[index];
    if (buffer === undefined) {
      throw new Refusal(`gpu: no uniform for transform pass ${index}`);
    }
    // inverse is 1 for the two passes of Fft::inverse, which are plan[2] and plan[3].
    device.queue.writeBuffer(buffer, 0, frameWords(fixture, at, { live, mode, inverse: index < 2 ? 0 : 1 }));
  });
}

/** Two `Float64Array`s interleaved and narrowed to `f32`, `x` then `y`. */
function pairs(x: Float64Array, y: Float64Array): Float32Array {
  const out = new Float32Array(x.length * 2);
  for (let k = 0; k < x.length; k += 1) {
    out[k * 2] = Math.fround(x[k] ?? 0);
    out[k * 2 + 1] = Math.fround(y[k] ?? 0);
  }
  return out;
}

/**
 * One 64-byte uniform, sixteen 4-byte members, in the prelude's field order.
 *
 * `inv_scale` is `1/scale` and is exact: `scaleFor` returns a power of two, so the widening in
 * `widen_density` is a single exact multiply rather than a division per sample.
 */
export function frameWords(fixture: Fixture, at: Placement, pass: PassFields): ArrayBuffer {
  const words = new ArrayBuffer(64);
  const ints = new Uint32Array(words);
  const floats = new Float32Array(words);
  const scale = scaleFor(fixture.n) ?? 1;
  ints.set([fixture.n, fixture.side, Math.log2(fixture.side), at.cells, scale, Math.ceil(fixture.n / 256)]);
  ints.set([pass.live, pass.mode, pass.inverse, at.fault, 0], 6);
  floats.set([at.h, at.originX, at.originY, at.charge, 1 / scale], 11);
  return words;
}
