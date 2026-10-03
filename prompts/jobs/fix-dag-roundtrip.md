# Job fix-dag-roundtrip (agent build: layout.dag.sugiyama breaks its own roundtrip invariant)

Read `prompts/jobs/fix-common.md` first (the RED/GREEN loop and the constraints; there is no review id:
this job's single finding is below, call it D-01).

Facts (measured 2026-10-03 on fix-tree-sugiyama 7730dad, also red on develop since 0bed9cb,
2026-10-02 20:54, "sugiyama acyclic/layering"):
- `scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 100` exits 1:
  `layout.dag.sugiyama` 94/100. The six failing seeds:
  ```
  FAILED seed 66: edge 44 (33 -> 1): y 3 -> 1 through 0 dummies, reversed=true
  FAILED seed 75: edge 26 (19 -> 1): y 5 -> 1 through 0 dummies, reversed=true
  FAILED seed 77: edge 26 (19 -> 0): y 1 -> 0 through 2 dummies, reversed=true
  FAILED seed 78: edge 92 (62 -> 0): y 3 -> 0 through 0 dummies, reversed=true
  FAILED seed 96: edge 137 (92 -> 1): y 5 -> 1 through 0 dummies, reversed=true
  FAILED seed 97: edge 136 (93 -> 1): y 4 -> 1 through 0 dummies, reversed=true
  ```
- The invariant is `crates/graph-cli/src/snapshot_cmd/dag.rs` `invariants`: a reversed edge (note
  `EdgeReversed`) spans `-(dummies + 1)` layers, one layer per hop. Every failure is a reversed edge
  whose dummy count does not match its span (0 dummies over 2-4 layers, or 2 dummies over 1 layer).
- `roundtrip-100` is not in `quick.rows`, which is why the landing gate did not catch it.

Do, in order:
1. Reproduce seed 66 alone and name the line in `crates/graph-core/src/layout/sugiyama/**` that gives a
   reversed edge the wrong dummy chain (suspects: 0bed9cb's `acyclic.rs` and `acyclic/feedback.rs`,
   a reversed edge that shares endpoints with a forward one, `routing.rs`'s dummy attribution).
2. RED: a unit test in `sugiyama/` on the smallest graph that reproduces it (shrink seed 66's graph).
3. GREEN: fix the motor at the root. The invariant in `dag.rs` does not move, unless you prove it
   wrong against `docs/decisions/sugiyama-heuristics.md` and SciGraphs (`SciGraphs/core/scigraphs_core/
   mesh/layouts/hierarchical.py`); then say so under "deviations" and keep the old check as a test.
4. Output moves only on the broken edges: paste `hashgate --seeds 8` (exit 0), its
   `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero), `scripts/scigraphs-conformance.sh` (exit 0) and
   the dagre crossing counts in `docs/decisions/sugiyama-heuristics.md` (unchanged or explained).

Paths: `crates/graph-core/src/layout/sugiyama/**`, `crates/graph-cli/src/snapshot_cmd/dag.rs` (only
per step 3), `docs/measurements/fix-dag-roundtrip.md` (cause, RED, GREEN, numbers).

Done when: fix-common's done-when, and `scripts/orch/rows/quick-roundtrip.rows` is green (quick.rows
plus `roundtrip --seeds 100`).
