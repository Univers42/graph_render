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
3. 3-D variants, implemented from the spec docs, never from igraph's C (layouts-igraph.md
   rule 1): `docs/layouts/layout.force.fruchterman_reingold.md` already specifies igraph's 3-D
   FR (no grid; random start ±sqrt(n)/2 per axis), `docs/layouts/layout.force.kamada_kawai.md`
   igraph's 3-D KK (deterministic sphere start; the 3-D solve's sign, line 97). Not networkx:
   networkx's FR and KK are different algorithms from the ones SciGraphs calls here.
   Ids `layout.force.fruchterman_reingold_3d` and `layout.force.kamada_kawai_3d`, full Metadata,
   oracle = python-igraph 0.11.9 at `dim=3` through `harness/oracle-igraph.py` (add a dim=3
   pass; its stress metric and ceiling rules stand). Share the kernel across dimensions the way
   `layout/force/spring/forces.rs` does (`const D`); the 2-D ids stay byte-identical
   (`hashgate --seeds 8`). If a spec is silent on a 3-D step, a spec author amends the spec
   first; the implementer never fills the hole from igraph's source.
   DRL 3-D: only if the DrL paper defines the 3-D step without igraph's code; otherwise record
   a `Gap` and leave the row.
4. Map FR and KK rows to the 3-D ids in `motor.rs`. Re-pin the rows whose disparity moved.

Paths: `layout/force/**` (new 3-D modules), `registry.rs` (additive), `harness/oracle-igraph.py`
(dim=3 pass), `docs/layouts/*.md` (spec gaps only), the conformance `motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs`,
`docs/measurements/sg-igraph-dims.md`, `docs/decisions/layouts-igraph.md` (one paragraph on the
RNG correction).

Done when: the reproducibility diff, a before/after table for the five rows, and the new
igraph dim=3 differentials within their stated ceilings are pasted, and the sg-common done-when
holds.
