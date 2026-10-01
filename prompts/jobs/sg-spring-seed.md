# Job sg-spring-seed (agent build, SciGraphs conformance: SPRING, SPRING_3D, CIRCLE_PACKING)

Read `prompts/jobs/sg-common.md` first. Needs `sg-fix-spring-temp`, `sg-dedupe` and `sg-mt19937`
landed (merge develop into this branch first; stop if any is missing). Rows: `SPRING`,
`SPRING_3D` (cause `rng`) and `CIRCLE_PACKING` (cause `algorithm`, exact on the 20 gate models,
0.517 on `lesmis`).

Why: one gap explains all three. `SpringParams` (`layout/force/spring.rs:120`) has no seed and
`start<D>` (`:191`) draws the start from Mulberry32. The reference passes
`seed=get_layout_seed()` (981798123) to `nx.spring_layout` (`networkx_layouts.py:16-34`), and
networkx turns an int seed into `np.random.RandomState(seed)` (`utils/misc.py:290-291`, under
`$GM_SCRATCH/refs/networkx-3.6`). The dense path (n < 500, every fixture) draws
`seed.rand(nnodes, dim)` as float64, row-major (`drawing/layout.py:675`). The sparse path
(n >= 500, `:629-637`) builds `A` with dtype `"f"`, so its start is float32: out of scope, and it
gets a `Caveat:` line where the seed is drawn.
`CIRCLE_PACKING` on `lesmis` is the non-planar fallback (`circle_packing.py:281`; graph-core
`layout/circle_packing/fallback*`). It calls `nx.spring_layout(G, dim=2, scale=frame,
iterations=seed_iterations, seed=get_layout_seed())` with `frame = scale * 0.45` and
`seed_iterations = max(10, min(50, 20000 // n))` (`circle_packing.py:424-429`). Doc repair 3
("port that fallback") is stale: the fallback exists, and its gap is this seed. Check that
before you trust it.

Do:
1. Measure all three rows (sg-common step 1).
2. RED: a graph-core test. A 2-node `SpringParams { seed: Some(981798123), iterations: 0, .. }`
   start equals `RandomState(981798123).rand(2, D)`: the `Mt19937` vectors from sg-mt19937,
   bit for bit, before the rescale.
3. GREEN: add `seed: Option<u32>` to `SpringParams`. `None` keeps today's Mulberry32 start
   byte for byte, so the registered layouts and their hashgate records stay valid. `Some(s)`
   draws `Mt19937::new(s)`, `next_f64()` per value, row-major. Update every `SpringParams`
   constructor in every crate (CLAUDE.md "a new struct field needs every constructor"). The
   conformance `motor.rs` passes `Some(LAYOUT_SEED)` for SPRING and SPRING_3D.
4. The circle-packing fallback: make the fallback's seed-spring call pass the same `Some(seed)`
   and the parameters above. Only on the path the conformance arm takes: the registered
   `layout.packing.circle` keeps its default unless the fallback has no other caller (say which).
5. Re-measure. The expected cause is `arithmetic` (numpy's pairwise sums vs a sequential loop):
   record it with the f32 count. Do not chase the last ulps by reordering sums unless one
   reduction is named in a measured diff.
6. Close `G_SPRING_SEED` (sg-common step 5); rewrite doc repairs 3 and 4 (4's "f64 -> 1020" is
   unreachable, Geometry is f32: `basic_3d.rs:45-51`).

Paths: `layout/force/spring*.rs`, `layout/force/spring3d*`, `layout/circle_packing/fallback*`,
callers of `SpringParams` across crates, the conformance `motor.rs`, `rows.rs`, `gaps.rs`,
`baseline/table.rs` (these three rows only), the doc rows and repairs 3-4,
`docs/measurements/sg-spring-seed.md`.

Done when: the spring oracle gate (the existing networkx spring differential) is still green,
the three rows' new metrics are pasted with their f32 counts (target 1020/1020, or a tolerance
with the measured reason), and the sg-common done-when holds.
