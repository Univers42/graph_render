# Job fix-tree-sugiyama (agent build: review-layout-tree L-02, L-03, L-12, L-13, L-14)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-tree.md` (ids `L-NN`;
lines may have moved; the branch is cut after sg-sugiyama lands). SciGraphs is in `SciGraphs/` in
the worktree (`core/scigraphs_core/mesh/layouts/hierarchical.py`).

Settled, do not reopen: the review's "no Brandes-Köpf" item is not a defect. SciGraphs' X phase is
the Sugiyama-Tagawa-Toda priority method with the same `_PRIORITY_NODE_BUDGET = 200000`
(`hierarchical.py:8-10,565-580`), and the motor ports it.

1. **L-02, MAJOR.** `sugiyama/ordering.rs` `sort_by_median`'s comparator recomputes
   `median_position` (a `Vec` alloc and a sort) on every comparison. GREEN: compute each vertex's
   median once into `Vec<(median, slot, v)>` and sort that. Same order: ties must break exactly as
   today (check the comparator's tie rule, and SciGraphs' if the motor follows it).
2. **L-03, MAJOR.** `pair_crossings` collects and sorts two `Vec`s per call, four calls per pair,
   inside `transpose`. GREEN: sort each vertex's neighbour positions once per pass into a reused
   buffer. Crossing counts and the final order unchanged.
3. **L-12, MINOR.** `coords.rs`: above `PRIORITY_NODE_BUDGET` the X phase is skipped with no note.
   GREEN: emit a `NoteCode` (an existing one if one fits, else add one through the contract) and add
   the case to the registry row's `degradation` text. Coordinates unchanged.
4. **L-13, MINOR.** `ordering.rs` drops a vertex whose layer is out of range. GREEN: `StageError`.
   RED: if no public input reaches it, test the seam and say so.
5. **L-14, MINOR.** `routing.rs` emits a self-loop as a degenerate polyline (first point = last).
   Read what SciGraphs emits for `n -> n`; match it, or document the convention in the row text.

For 1 and 2, paste a `bench` row on the largest-layer case before and after (n = 10k and 100k),
and show output byte-identical: `hashgate --seeds 8` (exit 0), its `GM_MUTATE_REFERENCE_DEGREE=9`
control (non-zero), the dagre crossing counts in `docs/decisions/sugiyama-heuristics.md` unchanged,
and `scripts/scigraphs-conformance.sh` (exit 0).

Paths: `crates/graph-core/src/layout/sugiyama/**`, `crates/graph-core/src/registry/grid.rs` (the
sugiyama row's `degradation` text only), the contract's note-code list only if a code is added,
`docs/measurements/fix-tree-sugiyama.md` (one row per id: verdict, RED, GREEN, numbers).

Done when: fix-common's done-when; every id above has a row.
