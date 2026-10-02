# Job fix-tree-hierarchy (agent build: review-layout-tree L-09, L-10, L-11, L-15..L-19)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-tree.md` (ids `L-NN`;
lines may have moved). The pinned references are on the host at `/home/dlesieur/goinfre/refs` (read
them with the read tool; `scripts/orch/gr` does not mount them). SciGraphs is in `SciGraphs/` in the
worktree.

1. **L-09, MINOR.** `layout/hierarchy.rs` doc says "one counting sort"; the code runs two comparison
   sorts (`notes.sort()`, `cuts.sort()`). Correct the doc to the real cost, or use a counting sort
   if output is unchanged and a `memory.rs` row shows it is cheaper. Ladder: the doc fix is rung 1.
2. **L-10, MINOR.** `depth()` is 1 for real roots under a virtual root. List every caller of
   `depth()`; if any maps depth to a ring/level against a reference that starts at 0 and its
   conformance row disagrees, that is a finding with a RED test. Otherwise document the offset at
   `depth()` and verdict `doc-only`.
3. **L-11, MINOR.** `layout/hierarchy/fixture.rs` accepts `"NaN"`/`"inf"` weights. RED: a fixture
   weight of `"NaN"`. GREEN: refuse a non-finite weight with an error naming the node.
4. **L-15, MINOR.** `layout/treemap/rows.rs` sorts with `total_cmp`; d3's `b.value - a.value` gives
   NaN for two `+inf` aggregates, which `Array.prototype.sort` treats as equal (keep order). RED:
   parent with children summing to `+inf`, `+inf`, `5.0`, compared against what d3 3.1.2's
   `squarify.js` order would give (derive it, do not guess). GREEN: a comparator that treats two
   equal infinities as equal.
5. **L-16, MINOR.** `layout/tidy_tree/walk.rs` `second_walk` iterates a breadth-first order where
   `tree.js` uses preorder; output is equal only because `m[parent]` is final at visit time. Add a
   test on a 2+ root forest that pins the invariant, and a `debug_assert!` or comment at the loop
   naming it; make `skip(1)` check that `order()[0]` is the layout root.
6. **L-17, MINOR.** `tidy_tree/walk/contour.rs` `.expect(..)` panics out of `Walk::layout`. GREEN:
   return a `StageError` naming the violated invariant (no RED input is known: say so and test the
   error path through a unit seam if one exists, else verdict `doc-only` with the reason).
7. **L-18, MINOR.** `layout/hierarchical_3d/disk.rs` returns the origin for `count == 1`. Read
   SciGraphs' `_hierarchical_layout_3d`: if it places a singleton level at the origin, keep the
   branch and state the convention in the module doc (`doc-only`); if not, fix it under the
   conformance row (17) and paste `scripts/scigraphs-conformance.sh`.
8. **L-19, MINOR.** `hierarchical_3d.rs` module doc frames the layout as lifting the 2D Sugiyama
   result; it is an independent port of SciGraphs' BFS-based 3D layout. Fix the doc only.

Output must not move except where an id's GREEN requires it (say which). Paste `hashgate --seeds 8`
(exit 0), its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero), and
`scripts/scigraphs-conformance.sh` (exit 0).

Paths: `crates/graph-core/src/layout/hierarchy.rs`, `crates/graph-core/src/layout/hierarchy/**`,
`crates/graph-core/src/layout/tidy_tree.rs`, `crates/graph-core/src/layout/tidy_tree/**`,
`crates/graph-core/src/layout/treemap.rs`, `crates/graph-core/src/layout/treemap/**`,
`crates/graph-core/src/layout/hierarchical_3d.rs`, `crates/graph-core/src/layout/hierarchical_3d/**`,
`docs/measurements/fix-tree-hierarchy.md` (one row per id: verdict, RED, GREEN).

Done when: fix-common's done-when; every id above has a row.
