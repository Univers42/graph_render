/**
 * `zero.wgsl.ts` — the fixed-point density clear.
 *
 * **This is its own dispatch, never the deposit reading what was there.** The CPU's deposit
 * writes only rows `0..cells` of the density (`deposit.rs:120`) and its runner clears the rest
 * before the kernel runs (`mesh.rs:205-210`). A GPU deposit that only *added* into last tick's
 * density would be a different kernel from the one being transcribed, and the difference is one
 * tick old — the plan names this stage as "the bug that reads last year's density"
 * (`plan :1101-1102`). `⌈P²/256⌉` workgroups, 4 096 at `P = 1024`.
 */

import { PRELUDE_WGSL } from "./prelude.wgsl.ts";

/** The WGSL for the clear. */
export const ZERO_WGSL = `${PRELUDE_WGSL}
@group(0) @binding(1) var<storage, read_write> density: array<i32>;

// The fixed-point density, zeroed. P*P stores at 4 bytes is 4 MB at P = 1024.
@compute @workgroup_size(256)
fn zero_density(@builtin(global_invocation_id) id: vec3<u32>) {
  let at = id.x;
  if (at < frame.side * frame.side) {
    density[at] = 0;
  }
}
`;