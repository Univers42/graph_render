/**
 * `bounds.wgsl.ts` — the finite positions' bounding box, the min/max fold.
 *
 * **`bounds` folds in `f32` and is exact anyway.** min and max are order-free, so the GPU's
 * block size and fold order need not match the CPU's `BLOCK = 4096` (`frame.rs:21`) — that is
 * why this is the first stage of the pass. What is *not* free is the **narrowing**: the arm
 * reads positions as `f32`, and `boundsExact` is the check that its min/max equal, bit for
 * bit, the host's min/max over `Math.fround` of the same positions. One bit of `f32` lost here
 * is a bit lost everywhere downstream, and at 1M the seed spiral reaches 12 000 units where
 * `f32`'s spacing is 2⁻¹⁰ (`gpu-force-tier.md:91-93`).
 *
 * ## Two dispatches, and two is enough
 *
 * The block pass runs `⌈n/256⌉` workgroups, 3 907 at 1M, against
 * `maxComputeWorkgroupsPerDimension` of 65 535 on both arms (`gpu-adapter.md:106-107`, `:126`).
 * A third dispatch would only be needed past `n ≈ 1.6·10⁷`, and `adapter.ts` refuses a device
 * whose limit is under the count this stage dispatches, so there is no third dispatch here to
 * fall back on: a device that passes the limits check can always run this in two.
 *
 * **The final fold is one invocation.** 256 lanes over 3 907 blocks is 16 iterations of a
 * serial loop on lane 0 and 255 idle lanes — deliberately, because a serial fold is the CPU's
 * own order (`frame.rs:101` folds the blocks in order) and min/max makes the order irrelevant
 * anyway. A workgroup-parallel fold would save nothing at 16 iterations and would put a second
 * reduction tree in a pass whose first stage exists to be trivially right.
 *
 * Caveat: the block pass reads `nodes[at]` **before** the `at < frame.n` guard, because WGSL
 * evaluates a `let` where it is written. That read is inside the buffer — the last block's
 * `at` reaches `⌈n/256⌉·256 − 1`, and the host allocates the node buffer rounded up to that —
 * and the guard is applied to the result, so the values are discarded. Failing input: none, but
 * the ordering is a trap for anyone who moves the `let` below the guard expecting it to be a
 * bounds check.
 */

import { NODES_WGSL, PRELUDE_WGSL } from "./prelude.wgsl.ts";

/** The WGSL for the bounds fold. */
export const BOUNDS_WGSL = `${PRELUDE_WGSL}
${NODES_WGSL}
@group(0) @binding(2) var<storage, read_write> boxes: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> extent: array<vec4<f32>>;

// One box per block of 256 nodes: (min x, min y, max x, max y). The empty box is the frame's
// own EMPTY (frame.rs:27-30), so a block of non-finite nodes folds to it exactly.
@compute @workgroup_size(256)
fn fold_block(@builtin(workgroup_id) block: vec3<u32>,
              @builtin(local_invocation_id) lane: vec3<u32>) {
  let at = block.x * 256u + lane.x;
  let p = nodes[at];
  var box = vec4<f32>(F32_MAX, F32_MAX, F32_MIN, F32_MIN);
  // A non-finite node is skipped, as frame.rs:124-129 skips it. The seed spiral has none; the
  // CPU's fold checks anyway, and this is a transcription of that fold.
  if (at < frame.n && p.x == p.x && p.y == p.y) {
    box = vec4<f32>(p.x, p.y, p.x, p.y);
  }
  boxes[block.x] = box;
}

// The blocks, folded into one, in block order. One invocation: see the module's note on why.
@compute @workgroup_size(256)
fn fold_extent(@builtin(local_invocation_id) lane: vec3<u32>) {
  if (lane.x != 0u) {
    return;
  }
  var acc = vec4<f32>(F32_MAX, F32_MAX, F32_MIN, F32_MIN);
  var at = 0u;
  loop {
    if (at >= frame.blocks) {
      break;
    }
    // The fault under --break bounds drops the last block, which is the fault boundsExact
    // exists to catch: an extent that is not the host's is an extent the rest of the pass was
    // not computed over.
    if (frame.fault == 3u && at + 1u == frame.blocks) {
      at = at + 1u;
      continue;
    }
    acc = widen(acc, boxes[at]);
    at = at + 1u;
  }
  extent[0] = acc;
}

// acc grown to cover b. The comparisons are strict, so of two equal ends the earlier one stays
// — the tie-break frame.rs:138-148 makes, and the reason the CPU's bounds do not depend on the
// worker count.
fn widen(acc: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
  return vec4<f32>(
    select(acc.x, b.x, b.x < acc.x),
    select(acc.y, b.y, b.y < acc.y),
    select(acc.z, b.z, b.z > acc.z),
    select(acc.w, b.w, b.w > acc.w),
  );
}
`;