# Job sg-fix-spring-temp (agent build, SciGraphs conformance repair)

Read `prompts/jobs/sg-common.md` first (needs `sg-conformance-split` on develop).

Why: a 2026-10-01 review of every layout against SciGraphs and its oracles found two defects that change
coordinates on inputs the oracle accepts:
1. `crates/graph-core/src/layout/force/spring/forces.rs:95` (`opening`) takes the opening temperature from
   the widest span over all `D` columns. networkx takes it from x and y only, also for `dim=3`:
   `layout.py:687` (dense) and `layout.py:776` (sparse) in `$GM_SCRATCH/refs/networkx-3.6`. A 3D start whose
   z span is the widest opens hotter in the motor than in networkx (`layout.force.spring3d`). The doc
   comment cites `layout.py:705-706`; the lines are 687 and 776.
2. `crates/graph-core/src/layout/circle_packing/fallback.rs:60-66` (`initial_radii`) counts degree over the
   simple graph, with self-loops removed. SciGraphs reads `G.degree(n)` on the graph it was given
   (`SciGraphs/core/scigraphs_core/mesh/layouts/circle_packing.py:420`), where networkx counts a self-loop
   twice. First check how `apply_graph_layout` builds `G` (does it keep self-loops?) and cite the line; if
   it drops them, item 2 is not a defect: say so with the line and change nothing for it.

Do:
1. Write the failing test first for each item (RED), then fix (GREEN). Item 1: a 3D start with z span
   larger than x and y; `opening` equals `0.1 * max(xspan, yspan)`. Item 2: a fixture with one self-loop;
   radii match networkx's `G.degree`.
2. For item 1, add a differential against networkx `spring_layout(dim=3)` on one fixture with a z-dominant
   start, through the existing oracle harness if spring3d has one (find it via the registry `oracle`
   field); record the result in `docs/measurements/sg-fix-spring-temp.md`.
3. Fix the line citations in the doc comment.
4. Both fixes change coordinates, so they can move SciGraphs conformance rows (SPRING_3D,
   CIRCLE_PACKING). Run `scripts/scigraphs-conformance.sh` before the fix (exit 0) and after it.
   Re-pin only the rows this fix moved, per sg-common step 4, and paste each row's before/after
   disparity; a row that got worse is a finding to report, not to re-pin silently. A moved row
   that is neither spring nor circle packing is a regression: stop.

Paths: `crates/graph-core/src/layout/force/spring/**`, `crates/graph-core/src/layout/circle_packing/**`,
the spring3d oracle harness if one exists, `docs/measurements/sg-fix-spring-temp.md`, and the conformance `baseline/` rows step 4 re-pins. Nothing else.

Done when: `scripts/scigraphs-conformance.sh` exits 0 and `--break` exits 1; the new tests fail before the fix and pass after (paste both runs); the merge floor is green;
`hashgate --seeds 8` exits 0 and its `GM_MUTATE_REFERENCE_DEGREE=9` negative control exits non-zero.
