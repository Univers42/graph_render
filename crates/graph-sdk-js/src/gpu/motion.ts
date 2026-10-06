/**
 * `motion.ts` — the tick's host half: the frozen motion constants and the centre's fold.
 *
 * The constants are the engine's own (`params.rs:72-73`): `center_strength = 1` and
 * `velocity_decay = 1 - 0.42 = 0.58`, written as the expressions the engine writes them as,
 * so a later change is a diff against a real number rather than a guess.
 *
 * `centreShift` is `Sim::center_shift` (`barnes_hut/sim.rs:225-237`) over the arm's f32
 * positions: a f64 left fold in node order — the f32 values widen to f64 exactly — then
 * `(mean) * strength`. The fold is one thread's, because the mean is one thread's fold on
 * the CPU too (D3): a reduction across invocations would be a different order and different
 * bytes. The kernel only subtracts the shift this returns (`motion.rs:109-113`).
 *
 * Caveat: `n = 0` divides by zero and returns `NaN`, where the CPU's `center_shift` returns
 * `None` and `center` skips the shift (`sim.rs:227-229`). No caller folds an empty mesh —
 * the fixtures' `n` is 1k and 50k — and one that did would want the `None`, not the `NaN`.
 * Caveat: the fold is over the positions as uploaded, f32 narrowed once; the fixture's own
 * f64 positions fold to different bytes, which is why `centreShiftF64` exists, for the
 * test's reference and the fixture's own arithmetic.
 */

import { MOTION_WGSL } from "./kernels/motion.wgsl.ts";
import { dispatch, groupsFor } from "./pipelines.ts";
import { GPUBufferUsage } from "./types.ts";
import type {
  GPUBindGroup,
  GPUBuffer,
  GPUCommandEncoder,
  GPUComputePipeline,
  GPUDevice,
  GPUShaderModule,
} from "./types.ts";

/** The frozen `velocity_decay` (`params.rs:73`): `1 - d3's velocityDecay`, in d3's terms. */
export const VELOCITY_DECAY = 1 - 0.42;

/** The frozen `center_strength` (`params.rs:72`). */
export const CENTER_STRENGTH = 1.0;

/**
 * The centre's shift over the arm's f32 positions, bit-for-bit the CPU's `center_shift`.
 *
 * `positions` is `x, y` interleaved, `2n` components; the f32 values widen to f64 exactly,
 * so the fold is over the same numbers the CPU folds.
 */
export function centreShift(positions: Float32Array, n: number, strength: number = CENTER_STRENGTH): { dx: number; dy: number } {
  let sx = 0;
  let sy = 0;
  for (let i = 0; i < n; i += 1) {
    sx += positions[2 * i] ?? 0;
    sy += positions[2 * i + 1] ?? 0;
  }
  return { dx: (sx / n) * strength, dy: (sy / n) * strength };
}

/** The same fold over f64 position columns, for the test's reference and the fixture's own arithmetic. */
export function centreShiftF64(posX: Float64Array, posY: Float64Array, n: number, strength: number = CENTER_STRENGTH): { dx: number; dy: number } {
  let sx = 0;
  let sy = 0;
  for (let i = 0; i < n; i += 1) {
    sx += posX[i] ?? 0;
    sy += posY[i] ?? 0;
  }
  return { dx: (sx / n) * strength, dy: (sy / n) * strength };
}

/**
 * The motion stage: the velocity merge, the centre shift and the integrate, built once per
 * graph over the resident buffers.
 *
 * `merge` adds a pass's delta onto the velocities (`motion.rs:51-63`). `centre` subtracts the
 * host's mean from the positions (`motion.rs:109-113`). `integrate` decays the velocities and
 * moves the positions, with pins overwriting (`motion.rs:64-66`, `:86-93`). Each encodes its
 * dispatch into a given encoder, so the tick runs them in the mesh's order.
 *
 * Caveat: the merge's bind group is cached per delta buffer, so a caller that reuses one delta
 * buffer across ticks pays one bind group creation and not one per tick. A caller that alternates
 * between two buffers pays two. The tick uses two (link, charge), so it pays two.
 */
export interface MotionStage {
  merge(encoder: GPUCommandEncoder, delta: GPUBuffer): void;
  centre(encoder: GPUCommandEncoder, by: { dx: number; dy: number }): void;
  integrate(encoder: GPUCommandEncoder): void;
}

/** The buffers the motion stage reads and writes. */
export interface MotionBuffers {
  readonly positions: GPUBuffer;
  readonly velocities: GPUBuffer;
  readonly pins: GPUBuffer;
}

/**
 * Builds the motion stage: compiles the kernels, uploads the uniforms, caches the bind groups.
 *
 * The `pins` buffer is `NaN` everywhere — the fixtures carry no pins — so the integrate's pin
 * branch never fires; the kernel is still written for the pinned case, because the CPU's
 * integrate is (`motion.rs:64-66`).
 */
export function buildMotionStage(
  device: GPUDevice,
  buffers: MotionBuffers,
  n: number,
): MotionStage {
  const module = device.createShaderModule({ label: "motion", code: MOTION_WGSL });
  const merge = pipeline(device, module, "velocity_merge", "motion-merge");
  const integrate = pipeline(device, module, "integrate", "motion-integrate");
  const centre = pipeline(device, module, "centre_shift", "motion-centre");
  // The prelude Frame's `n`, for the kernels' guard. The motion kernels read `frame.n`, so the
  // stage carries a 64-byte uniform with it; the other fields are unused by these three.
  const frame = uniform(device, "motion-frame", 64, (ints) => {
    ints[0] = n;
  });
  // The motion uniform: `n` and the decay, narrowed once.
  const motion = uniform(device, "motion-uniform", 16, (ints, floats) => {
    ints[0] = n;
    floats[1] = VELOCITY_DECAY;
  });
  const shift = device.createBuffer({ label: "motion-shift", size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const mergeBinds = new Map<GPUBuffer, GPUBindGroup>();
  return {
    merge(encoder: GPUCommandEncoder, delta: GPUBuffer): void {
      let bind = mergeBinds.get(delta);
      if (bind === undefined) {
        bind = device.createBindGroup({
          label: "motion-merge",
          layout: merge.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: frame } },
            { binding: 3, resource: { buffer: buffers.velocities } },
            { binding: 4, resource: { buffer: delta } },
          ],
        });
        mergeBinds.set(delta, bind);
      }
      const pass = encoder.beginComputePass();
      dispatch(pass, merge, bind, groupsFor(n));
      pass.end();
    },
    centre(encoder: GPUCommandEncoder, by: { dx: number; dy: number }): void {
      const words = new ArrayBuffer(16);
      const floats = new Float32Array(words);
      floats[0] = by.dx;
      floats[1] = by.dy;
      device.queue.writeBuffer(shift, 0, words);
      const bind = device.createBindGroup({
        label: "motion-centre",
        layout: centre.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 5, resource: { buffer: buffers.positions } },
          { binding: 7, resource: { buffer: shift } },
        ],
      });
      const pass = encoder.beginComputePass();
      dispatch(pass, centre, bind, groupsFor(n));
      pass.end();
    },
    integrate(encoder: GPUCommandEncoder): void {
      const bind = device.createBindGroup({
        label: "motion-integrate",
        layout: integrate.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 2, resource: { buffer: motion } },
          { binding: 3, resource: { buffer: buffers.velocities } },
          { binding: 5, resource: { buffer: buffers.positions } },
          { binding: 6, resource: { buffer: buffers.pins } },
        ],
      });
      const pass = encoder.beginComputePass();
      dispatch(pass, integrate, bind, groupsFor(n));
      pass.end();
    },
  };
}

/** One compute pipeline from one module and one entry point. */
function pipeline(device: GPUDevice, module: GPUShaderModule, entryPoint: string, label: string): GPUComputePipeline {
  return device.createComputePipeline({ label, layout: "auto", compute: { module, entryPoint } });
}

/** One uniform buffer, filled by `fill` with the ints and floats it wants. */
function uniform(
  device: GPUDevice,
  label: string,
  size: number,
  fill: (ints: Uint32Array, floats: Float32Array) => void,
): GPUBuffer {
  const words = new ArrayBuffer(size);
  const ints = new Uint32Array(words);
  const floats = new Float32Array(words);
  fill(ints, floats);
  const buffer = device.createBuffer({ label, size, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(buffer, 0, words);
  return buffer;
}
