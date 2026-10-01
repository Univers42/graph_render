# Job sg-igraph-dims (agent build, SciGraphs conformance: IGRAPH_FR, IGRAPH_KK, IGRAPH_DRL, IGRAPH_DRL_2D, IGRAPH_LGL)

Read `prompts/jobs/sg-common.md` first. Rows: the five `IGRAPH_*` rows.

Why, corrections first:
- `G_IGRAPH_SEED` (`gaps.rs:57`) and doc finding 4 are wrong. python-igraph installs Python's
  `random` as igraph's RNG (`src/_igraph/random.c:55-57`, `:296-325` in the python-igraph
  source), so `_reset_layout_rng` seeding `random` DOES seed igraph. The reference is
  reproducible; prove it by running each row twice and diffing the bytes.
- Bitwise is still out of reach by licence: `docs/decisions/layouts-igraph.md` rule 4 forbids
  reproducing igraph's RNG stream or its C code. The target is shape (disparity), not bytes.
- Dimension. SciGraphs calls FR (`igraph_layouts.py:74`), KK (`:99`) and DRL (`:342`) with
  `dim=3`, DRL_2D (`:406`) with `dim=2`. The motor's FR, KK and DRL are 2-D. Then
  `_igraph_fit_positions` (`:24-42`): subtract the mean, divide by `abs().max()`, multiply by
  scale.

Do:
1. Measure all five rows; run the reference twice and paste the byte diff (expected: none).
   Rewrite `G_IGRAPH_SEED` as the licence gap it really is (note cites `random.c` and
   layouts-igraph.md rule 4) and fix finding 4 in the doc.
2. Apply `_igraph_fit_positions` to the motor arm in the conformance code (one function, its
   own test). Re-measure: this separates scale from shape.
3. 3-D variants, written from networkx 3.6 (BSD; `spring_layout(dim=3)`,
   `kamada_kawai_layout(dim=3)`) and the papers, never from igraph's C:
   `layout.force.fruchterman_reingold_3d` and `layout.force.kamada_kawai_3d`, registered with
   full Metadata, oracle = networkx 3.6 at `dim=3` (reuse the Python oracle harness pattern of
   `harness/oracle-fa2.py`). The 2-D ids stay byte-identical (`hashgate --seeds 8`).
   DRL 3-D: only if the DrL paper defines the 3-D step without igraph's code; otherwise record
   a `Gap` and leave the row.
4. Map FR and KK rows to the 3-D ids in `motor.rs`. Re-pin the rows whose disparity moved.

Paths: `layout/force/**` (new 3-D modules), `registry.rs` (additive), a harness script for the
networkx 3-D oracle, the conformance `motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs`,
`docs/measurements/sg-igraph-dims.md`, `docs/decisions/layouts-igraph.md` (one paragraph on the
RNG correction).

Done when: the reproducibility diff, a before/after table for the five rows, and the new
networkx differentials within their stated ceilings are pasted, and the sg-common done-when
holds.
