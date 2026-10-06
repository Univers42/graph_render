/**
 * `motion.wgsl.ts` — the tick's three motion passes as per-node gathers.
 *
 * The CPU pass is `Velocity::step_range` (`motion.rs:51-69`) for the merge and the decay,
 * `Position::step_range` (`motion.rs:86-93`) for the integrate's position half, and
 * `Shift::step_range` (`motion.rs:109-113`) for the centre's shift. Each kernel is one
 * node's expression, evaluated in the same order, so the bytes are the CPU's — modulo the
 * arm's `f32`, which is the tier's whole point: the arm is held to a bound and not to the
 * CPU's bytes, as `link.wgsl.ts` says.
 *
 * **The decay is `f32` on the device.** The CPU's `velocity_decay` is the `f64` `0.58`
 * (`params.rs:73`); the uniform carries it narrowed once, by the host's `Float32Array`
 * store, as every per-node constant is. A pinned axis's velocity is zeroed and its position
 * placed at the pin (`motion.rs:64-66`, `motion.rs:88-91`); the pin is `NaN` for "no pin",
 * the CPU's `None`, and finite for a placed axis.
 *
 * **The centre's mean stays on the host.** It is one thread's `f64` fold in node order
 * (`sim.rs:225-237`), and the kernel only subtracts the `f32` shift the host uploads
 * (`motion.rs:109-113`). A `f64` reduction across invocations would be a different kernel
 * from the one being transcribed, and a different order is a different sum.
 *
 * **The bindings.** `frame` (0) and `nodes` (1) come from the prelude; the motion passes add
 * `motion` (2), `velocities` (3), `delta` (4), `positions` (5), `pins` (6) and `shift` (7).
 * The two writing kernels go through `positions`, never `nodes`: a buffer bound read-only and
 * read-write in the same pass is a validation error, so a kernel that writes positions cannot
 * also read the read-only `nodes` binding over the same buffer. With `layout: "auto"` an
 * unused binding is left out of the pipeline's bind group layout, so the host binds the
 * positions buffer once, to `positions`, for those two; `nodes` stays for the read-only
 * consumers and is unused by these three.
 *
 * Caveat: a node pinned to a `NaN` coordinate is indistinguishable from an unpinned one —
 * the sentinel *is* the `NaN` bit pattern, so a pin at `NaN` reads as "no pin" and the node
 * integrates freely. A fixture that pinned a `NaN` coordinate would be a data error, not a
 * kernel error. Caveat: the shift the kernel subtracts is the host's `f64` mean narrowed to
 * `f32`, where the CPU subtracts the `f64` mean itself (`sim.rs:236`) — the two differ by
 * one rounding of `dx`/`dy`, inside the arm's bound. Caveat: a compiler may contract
 * `v * decay` and `p + v` into fused multiply-adds, which WGSL permits: one rounding fewer
 * per term, inside the bound, and why the arm is held to a bound and not to the CPU's bytes.
 */

import { NODES_WGSL, PRELUDE_WGSL } from "./prelude.wgsl.ts";

/** The WGSL for the three motion passes. */
export const MOTION_WGSL = `${PRELUDE_WGSL}
${NODES_WGSL}

// The motion passes' own uniform: the node count and the integrate's decay, narrowed once
// from the f64 0.58 (params.rs:73) by the host's Float32Array store. Separate from the
// charge Frame because the motion passes are a different dispatch with a different uniform
// surface; the guard reads frame.n, the charge Frame's own, and motion.n mirrors it.
struct MotionFrame {
  n: u32,
  decay: f32,
  pad0: f32,
  pad1: f32,
};

// The centre's shift, as two f32s: the host's f64 mean (sim.rs:236) narrowed once. Two
// f32s rather than a vec2<f32> because a uniform binding is a 16-byte block in this pass.
struct ShiftFrame {
  x: f32,
  y: f32,
  pad0: f32,
  pad1: f32,
};

@group(0) @binding(2) var<uniform> motion: MotionFrame;
@group(0) @binding(3) var<storage, read_write> velocities: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read> delta: array<vec2<f32>>;
@group(0) @binding(5) var<storage, read_write> positions: array<vec2<f32>>;
@group(0) @binding(6) var<storage, read> pins: array<vec2<f32>>;
@group(0) @binding(7) var<uniform> shift: ShiftFrame;

// step::merge's velocity half (motion.rs:51-63): v = vx[i], then v += delta[i]. One
// invocation per node, each writing only its own output.
@compute @workgroup_size(256)
fn velocity_merge(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= frame.n) {
    return;
  }
  velocities[i] = velocities[i] + delta[i];
}

// Sim::integrate (motion.rs:64-66 and :86-93), both halves in one node's expression: the
// velocity is decayed, or zeroed when the axis is pinned; the position is placed at the pin,
// or moved by the velocity. The pin is NaN for "no pin" — the CPU's None — per axis.
@compute @workgroup_size(256)
fn integrate(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= frame.n) {
    return;
  }
  let p = positions[i];
  let v = velocities[i];
  let pin = pins[i];
  let vx = isnan(pin.x) ? v.x * motion.decay : 0.0;
  let vy = isnan(pin.y) ? v.y * motion.decay : 0.0;
  velocities[i] = vec2<f32>(vx, vy);
  positions[i] = vec2<f32>(isnan(pin.x) ? p.x + vx : pin.x, isnan(pin.y) ? p.y + vy : pin.y);
}

// Sim::center's shift half (motion.rs:109-113): x[i] = x[i] - by, with the host's mean.
@compute @workgroup_size(256)
fn centre_shift(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= frame.n) {
    return;
  }
  positions[i] = positions[i] - vec2<f32>(shift.x, shift.y);
}
`;
