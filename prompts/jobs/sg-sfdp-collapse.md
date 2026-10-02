# Job sg-sfdp-collapse (agent build, SciGraphs conformance: GRAPHVIZ_SFDP)

Read `prompts/jobs/sg-common.md` first. Runs BEFORE `sg-sfdp-step` (that job's coarsening and
smoothing work is judged on a solver that does not collapse). Row: `GRAPHVIZ_SFDP`.

Why: on `lesmis` the motor's sfdp layout is one-dimensional and has coincident points. Shape
measure = small/large eigenvalue of the 2-D covariance of the 77 points (1 = round, 0 = a line):
Graphviz 16.1.0 (`target/scigraphs-conformance/ref/GRAPHVIZ_SFDP.f64`, first 77 points) 0.414,
the motor on develop 0.006. Instrumented per level (seed 1): the coarsest level (n=8) is a
healthy 2-D layout (ratio 0.62); the first refine (n=11) drops to 0.026 and every later level
stretches it further (final 4.7e-3, 16 coincident pairs; seed 2: 40 pairs). The p13
`oracle-graphviz` sfdp ceiling (1e3 pt, worst 388, `docs/measurements/p13-gv2-sfdp.md`) does not
see this: it is vacuous for shape.

The defects, port against reference (`$GM_SCRATCH/refs/graphviz-16.1.0/graphviz-16.1.0/lib/sfdpgen/`):
a. `sfdp/solve.rs` `relax` moves by `step / length`, the argument, every iteration; the cooled
   `current` only decides when to stop. Reference: `x += step*f` with the cooled `step`
   (`spring_electrical.c:638`, updated at `:649`).
b. Every level runs adaptive cooling from 0.1. Reference: after the coarsest level the driver sets
   `adaptive_cooling = false` and `step = .1` (`:1160-1161`), so fine levels cool by `0.9*step`
   (`update_step`, `:171-184`).
c. Stop test is `current > TOL / K`; reference is `step > tol` with `tol = 0.001` absolute
   (`:47`, `:650`).
d. `sfdp/multilevel.rs:64-94` `prolongate` copies the coarse position and adds `rng::jiggle`
   (±5e-7 absolute, `rng.rs:54-60`) to every node. Reference (`:814-851`): P multiply, then
   `interpolate_coord` over the fine graph (alpha 0.5, Gauss-Seidel, self skipped), then
   `K*0.001*(drand()-0.5)` on every R-row member after the first, with `K` before the 0.75 decay.
e. `sfdp/quadtree.rs:132-165` `push` has no `MINDIST` crop; reference crops supernode distance
   (`:615`). Below `quadtree_size = 45` nodes (`:39`, `:543`) the reference sums all pairs
   exactly with `distance_cropped`; the port always walks the Barnes-Hut tree.
f. The module doc of `solve.rs` claims gather form (D10). The loop updates `self.x[i]` in place and
   later nodes read it, which is the reference's sequential order. Keep the sequential order
   (it is deterministic and it is what Graphviz does); correct the doc, and record the D10
   deviation in `docs/decisions/` with the reason (Graphviz output is the oracle, user 2026-09-30).

Probe evidence (throwaway instrumented tree, lesmis, final ratio / coincident pairs):

| change | seed 1 | seed 2 | seed 981798123 |
|---|---|---|---|
| develop | 0.0047 / 16 | — / 40 | 0.006 (row) |
| a | 0.062 / 0 | 0.27 / 0 | 0.021 / 0 |
| a+b+c | 0.28 / 0 | 0.22 / 0 | 0.19 / 0 |
| a+b+c+d (probe's own d) | 0.13 / 0 | 0.044 / 0 | 0.039 / 0 |

a+b+c removes the collapse. The probe's d made the shape worse; it used edge-list neighbour order
and a counter-based jitter, not the reference's CSR order and `drand()` stream. Port d from the
reference text and judge it by the row; if it still hurts, stop and report rather than drop it.

Do:
1. Measure the row (sg-common step 1). Also paste the shape ratio for both arms on lesmis.
2. RED, graph-core, one test per defect where it can fail on its own: a relax test showing the
   per-iteration move shrinks by 0.9 on a fine level (a, b); the loop count on a fine level is the
   reference's `ceil(log(0.001/0.1)/log(0.9)) = 44` when `maxiter` allows (c); prolongate on a
   matched pair leaves the second member `K*0.001`-scale away and runs `interpolate_coord` (d);
   no two output points closer than `1e-6 * span` and shape ratio > 0.15 on lesmis and on a 10x10
   grid (the collapse itself).
3. GREEN a, b, c first, then d, then e, measuring the row after each. Determinism rules (§6) hold.
4. `run` (the registered entry) changes: this is a bug in a Graphviz port, so the registered
   default moves. Re-run `oracle-graphviz` for sfdp and paste before/after; tighten the p13 sfdp
   ceiling only if the new worst case supports it, and write the ceiling's blind spot (shape) in
   the p13 doc either way.
5. Re-pin the row (sg-common step 4). `YIFAN_HU` has its own solver (`layout/force/yifan_hu.rs`)
   and must not move.

Done when: sg-common's done-when, plus the RED tests green, lesmis shape ratio within 0.15 of
Graphviz's 0.414 or the remaining gap explained by a named reference step not yet ported (coarsening
order and smoothing belong to `sg-sfdp-step`), and `docs/measurements/sg-sfdp-collapse.md` with
the per-defect table above re-measured on the real code.
