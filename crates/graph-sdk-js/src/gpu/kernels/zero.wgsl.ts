/**
 * `zero.wgsl.ts` — the density clear, and the frame's bounds fold.
 *
 * Both live in one module because they are the two stages that write a whole buffer rather
 * than one entry per node, and both are one dispatch over a `P·P` or `n`-sized index space.
 * Splitting them would give two files each with a binding block and a header, and nothing
 * else.
 *
 * **The clear is its own dispatch, never the deposit reading what was there.** The CPU's
 * deposit writes only rows `0..cells` of the density (`deposit.rs:120`) and the runner clears
 * the rest before the kernel runs (`mesh.rs:205-210`). A GPU deposit that only *added* into
 * whatever the previous tick left would be a different kernel from the one it is transcribed
 * from, and the difference is one tick old — the plan names this stage as "the bug that reads
 * last year's density" (`plan :1101-1102`).
 *
 * **`bounds` folds in f32 and is exact anyway.** min and max are order-free and associative,
 * so the GPU's block size and fold order need not match the CPU's `BLOCK = 4096`
 * (`frame.rs:21`); that is exactly why this stage is the first one. What is *not* free is the
 * **narrowing**: the arm reads positions as `f32`, and the report's `boundsExact` is the check
 * that its min/max equal, bit for bit, the host's min/max over `Math.fround` of the same
 * positions. One bit of `f32` lost here is a bit of `f32` lost everywhere downstream, and at
 * 1M the seed spiral reaches 12 000 units where `f32`'s spacing is 2⁻¹⁰.
 *
 * Two dispatches, and two is enough: the block pass runs `⌈n/256⌉` workgroups, 3 907 at 1M,
 * against a `maxComputeWorkgroupsPerDimension` of 65 535 on both arms
 * (`gpu-adapter.md:106-107`, `:126`). A third would only be needed past `n ≈ 1.6·10⁷`.
 * Caveat: this fold's block count is `⌈n/256⌉` and the arm refuses a device whose
 * `maxComputeWorkgroupsPerDimension` is under it (`adapter.ts`), so there is no third dispatch
 * to fall back on — a device that passes the limits check can always run this in two.
 */

/** The WGSL for `zero.wgsl.ts`, as the module's single export. */
export const ZERO_WGSL = `
// Every stage here is one invocation per element of a whole buffer.
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read_write> density: array<i32>;
@group(0) @binding(2) var<storage, read_write> boxes: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> extent: array<vec4<f32>>;

// The fixed-point density, zeroed. P*P stores at 4 bytes is 4 MB at P = 1024.
@compute @workgroup_size(256)
fn zero_density(@builtin(global_invocation_id) id: vec3<u32>) {
  let at = id.x;
  if (at < frame.side * frame.side) {
    density[at] = 0;
  }
}

// One box per block of 256 nodes: (min x, min y, max x, max y).
// A non-finite node is skipped, as frame.rs:124-129 skips it; the seed spiral has none, but
// the CPU's fold does not and this is a transcription of it.
@compute @workgroup_size(256)
fn fold_block(@builtin(workgroup_id) block: vec3<u32>,
              @builtin(local_invocation_id) lane: vec3<u32>) {
  let first = block.x * 256u;
  let at = first + lane.x;
  var box = vec4<f32>(F32_MAX, F32_MAX, -F32_MAX, -F32_MAX);
  if (at < frame.n) {
    let p = frame.nodes[at];
    if (p.x == p.x && p.y == p.y && abs(p.x) < F32_MAX && abs(p.y) < F32_MAX) {
      box = vec4<f32>(p.x, p.y, p.x, p.y);
    }
  }
  boxes[block.x] = box;
}

// The blocks, folded into one. One invocation, so the order is the block order and the result
// is a plain left fold — which min/max makes irrelevant, but which costs nothing to keep.
@compute @workgroup_size(256)
fn fold_extent(@builtin(local_invocation_id) lane: vec3<u32>) {
  if (lane.x != 0u) {
    return;
  }
  var acc = vec4<f32>(F32_MAX, F32_MAX, -F32_MAX, -F32_MAX);
  var at = 0u;
  loop {
    if (at >= frame.blocks) {
      break;
    }
    // The fault under --break bounds drops the last block, which is the fault the
    // boundsExact check exists to catch: an extent that is not the host's is not a number the
    // rest of the pass can be trusted to have been computed over.
    if (frame.fault == 3u && at + 1u == frame.blocks) {
      at = at + 1u;
      continue;
    }
    acc = min(acc, boxes[at]);
    acc = max(acc, vec4<f32>(acc.z, acc.w, acc.x, acc.y));
    at = at + 1u;
  }
  extent[0] = acc;
}
`;

/**
 * `F32_MAX` as a WGSL literal.
 *
 * WGSL has no `f32::MAX` constant, and `3.4028235e38` is the same value only if it rounds to
 * it — it does, being the nearest `f32` to the true `f32::MAX` — but writing it as an explicit
 * bit pattern says what it is and cannot drift. The empty box is the frame's own `EMPTY`
 * (`frame.rs:27-30`), with the axes ordered so `min` and `max` are both the right way round.
 */
export const F32_MAX_WGSL = "const F32_MAX: f32 = bitcast<f32>(0x7f7fffffu);\n";