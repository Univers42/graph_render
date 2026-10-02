# Job fix-tree-packing (agent build: review-layout-tree L-04..L-07, L-20..L-24)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-tree.md` (ids `L-NN`;
lines may have moved; the branch is cut after sg-fix-spring-temp and sg-spring-seed, which edit
`circle_packing/fallback*`). SciGraphs is in `SciGraphs/` in the worktree
(`core/scigraphs_core/mesh/layouts/circle_packing.py`).

1. **L-05 + L-06, MAJOR.** `radii.rs` `worst_free_error` and `placement.rs` `worst = worst.max(error)`
   use `f64::max`, which drops a NaN, so a NaN error reads as converged and an exact packing ships
   certified. RED: a unit test feeding a NaN error into each fold. GREEN: a non-finite error makes
   the result `f64::INFINITY`, so `is_embedded`'s finiteness check refuses it. Fix the doc at
   `circle_packing.rs` that says this hazard was closed.
2. **L-07, MAJOR.** `refine_tangency`'s non-convergence is discarded: return the final `worst` and OR
   `worst >= 1e-9` into `approximate`, so note code 3 is emitted. RED: a planar graph with
   `iterations = 1` must come back approximate.
3. **L-04, MAJOR.** `fallback/seed.rs` `vec![0.0; n * n]` with no cap: 80 GB at n = 100k, and `n * n`
   overflows `usize` on wasm32 above 65536 but not natively (a native/wasm32 divergence). GREEN: use
   the CSR the caller has (or a sparse adjacency) for the seed; if a dense matrix stays, guard
   `n * n` with `checked_mul` and refuse above the cap with a `StageError`. Positions must not move
   on the 20 gate models: paste `scripts/scigraphs-conformance.sh` (row 5) before and after.
4. **L-20, MINOR.** `params.scale <= 0` collapses every circle to the origin. RED: `scale: 0.0`.
   GREEN: refuse it where the other params are validated.
5. **L-21, MINOR.** `fallback/relax.rs` doc says a self-loop contributes `+` then `-`; the branch
   tests one endpoint. Self-loops are filtered upstream (`simple_pairs`), so fix the doc or the
   branch, whichever keeps the gather bit-identical to the scatter (pinned by its test).
6. **L-22, MINOR.** `fallback.rs` builds `Packed` with no finiteness check. GREEN: non-finite final
   positions -> `StageError` (or note code 3 if the row's contract says degrade, not fail).
7. **L-23, MINOR.** `placement.rs` `refine_tangency` reallocates `pulls` and `gradients` each
   iteration. Hoist both out of the loop; output byte-identical.
8. **L-24, MINOR.** `circle_packing.rs` `simple_pairs` uses a `BTreeSet<u64>`. Replace with a sorted,
   deduplicated `Vec<u64>` if output order is unchanged; paste a `memory.rs` row before and after.

Paste `hashgate --seeds 8` (exit 0), its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero), and
`scripts/scigraphs-conformance.sh` (exit 0).

Paths: `crates/graph-core/src/layout/circle_packing.rs`, `crates/graph-core/src/layout/circle_packing/**`,
`docs/measurements/fix-tree-packing.md` (one row per id: verdict, RED, GREEN).

Done when: fix-common's done-when; every id above has a row.
