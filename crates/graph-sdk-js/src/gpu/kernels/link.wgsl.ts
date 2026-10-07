/**
 * `link.wgsl.ts` — the link pass as one per-node gather, the mesh's own link column.
 *
 * The CPU pass is `link::pass_with` (`barnes_hut/link.rs:91-100`) merged by `motion::merge`
 * (`particle_mesh.rs:69-79`): one force per simple edge (`link.rs:114-137`), split between
 * its two endpoints by the edge's bias (`link.rs:141-147`), and each node's share summed over
 * its own CSR row in ascending edge order (`step.rs:177-189`). From rest the merge is
 * `0 + delta`, so the column the fixture stores is that sum and nothing else.
 *
 * **Gather, not scatter (D10).** An edge has two endpoints, and adding its force into both is
 * a float scatter. So one invocation per node walks its own row — built once on the host,
 * `link.ts`'s `linkCsr` — and recomputes each edge's force from its end. Both ends read the
 * same `nodes[hi] - nodes[lo]`, so they compute the same force bit for bit; the edge's square
 * root and division run twice, once per end, which is the price of having no atomics.
 *
 * **The same expressions, in the same order.** `p = x + v` per end (`link.rs:184-193`; the
 * probe's velocities are zero, so its bytes are the at-rest ones), `d = p[hi] - p[lo]`, `l = sqrt(d·d)`,
 * `factor = (l - distance) / l · strength`, `f = d · factor`, then `+f·(1 - b)` at the lower
 * end and `-f·b` at the higher one. The CPU's `(-f)·b` and the kernel's `f·(-b)` are the same
 * IEEE754 number, so negating the weight instead of the force moves no bit. `alpha` is 1 at
 * the fixture's state, so the uploaded `strength` is `alpha · strength` already.
 *
 * Caveat: a pair whose two `f32` positions coincide on both axes gets no force here, where
 * the CPU's `jiggle` (`link.rs:117-122`) gives it one of `O(distance)`. The 1k to 50k
 * fixtures have none (a one-off count of `f32` differences, `docs/measurements/gpu-g1.md`),
 * and one that did would fail `guard` rather than pass: an `O(60)` error at two nodes is
 * orders of magnitude over `k · 5 · 2⁻²³` in the rms. A compiler may also contract `a + b·c` into a
 * fused multiply-add, which WGSL permits: that is one rounding fewer per term, inside the
 * guard's five ULP, and it is why the arm is held to a bound and not to the CPU's bytes.
 */

/** The `--break` selector the uniform carries: `link-bias` swaps the two ends' weights. */
export const LINK_FAULT_BIAS = 1;

/** The uniform's size: `n`, `fault`, a pad and `alpha`, one 16-byte block. */
export const LINK_FRAME_BYTES = 16;

/** The WGSL for the per-node link gather. */
export const LINK_WGSL = `
struct LinkFrame {
  n: u32,
  fault: u32,
  pad0: u32,
  alpha: f32,
};

@group(0) @binding(0) var<uniform> frame: LinkFrame;
@group(0) @binding(1) var<storage, read> nodes: array<vec2<f32>>;
// The host's CSR: node i's incident edges are row_edge[row_start[i] .. row_start[i + 1]],
// ascending, which is the order graph-core's row_csr files them in.
@group(0) @binding(2) var<storage, read> row_start: array<u32>;
@group(0) @binding(3) var<storage, read> row_edge: array<u32>;
// Per edge: (lo, hi), and (distance, strength, b, 1 - b), each narrowed once from f64.
@group(0) @binding(4) var<storage, read> ends: array<vec2<u32>>;
@group(0) @binding(5) var<storage, read> geometry: array<vec4<f32>>;
@group(0) @binding(6) var<storage, read_write> delta: array<vec2<f32>>;
@group(0) @binding(7) var<storage, read> velocities: array<vec2<f32>>;

const FAULT_BIAS: u32 = ${LINK_FAULT_BIAS}u;

// Node i's own weight of edge e's force: 1 - b at the lower end, -b at the higher one.
// Under link-bias the two are swapped: -(1 - b) at the higher end and b at the lower.
fn weight(g: vec4<f32>, is_hi: bool) -> f32 {
  if (frame.fault == FAULT_BIAS) {
    return select(g.z, -g.w, is_hi);
  }
  return select(g.w, -g.z, is_hi);
}

// Node i's share of every edge in its row, summed in the row's order.
@compute @workgroup_size(256)
fn link_gather(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= frame.n) {
    return;
  }
  var sum = vec2<f32>(0.0, 0.0);
  for (var at = row_start[i]; at < row_start[i + 1u]; at = at + 1u) {
    let e = row_edge[at];
    let pair = ends[e];
    let g = geometry[e];
    let d = (nodes[pair.y] + velocities[pair.y]) - (nodes[pair.x] + velocities[pair.x]);
    if (d.x == 0.0 && d.y == 0.0) {
      continue;
    }
    let l = sqrt(d.x * d.x + d.y * d.y);
    // alpha is the per-tick uniform: the link's strength is alpha * strength
    // (barnes_hut/link.rs:134), and the probe folded alpha = 1 into its upload. The tick
    // carries the decaying alpha here, so the same kernel serves both.
    let factor = (l - g.x) / l * g.y * frame.alpha;
    let f = d * factor;
    sum = sum + f * weight(g, pair.y == i);
  }
  delta[i] = sum;
}
`;
