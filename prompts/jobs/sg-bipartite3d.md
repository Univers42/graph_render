# Job sg-bipartite3d (agent build, SciGraphs conformance: BIPARTITE_3D)

Read `prompts/jobs/sg-common.md` first. Row: `BIPARTITE_3D` (disparity 0.405, cause
`algorithm`). Doc repair 11 has the picture backwards: the reference is the ring, the motor the
two columns.

Why: SciGraphs' `_bipartite_layout_3d` (`hierarchical.py:213-242`) places the two node sets on
parallel planes, one ring each. Set 0 sits at `z = -scale * 0.5` and set 1 at `+scale * 0.5`,
radius `scale * 0.6`. Node `i` of a set of `count` is at angle `(i / max(1, count)) * 2 * pi`,
evaluated in that order, then `[r cos a, r sin a, z]`. The sets come from `_bipartite_parts`
(`:149`, a two-colouring per component), or from `_greedy_max_cut` (`:183`) when the graph is
not bipartite. graph-core's `layout.bipartite` (`bipartite.rs:30`) is networkx's
`bipartite_layout`, two columns, and has its own oracle: keep it unchanged.

Do:
1. Measure the row. Read `_bipartite_parts` and `_greedy_max_cut` and write down their exact
   visiting order: component order, BFS order, which colour is set 0, and tie-breaks. Most of
   the risk is here.
2. RED: graph-core tests. A 4-cycle and a non-bipartite triangle each give the reference's
   coordinates (generate them once in `ge-python-oracle`, paste them as `f64::from_bits`).
3. GREEN: a new 3D layout `layout.bipartite_3d` beside the other `basic_3d` layouts, with full
   registry `Metadata` (oracle: the SciGraphs conformance row; `Ponytail:` names the greedy cut
   as a heuristic). Use `libm` for `cos` and `sin` (sg-common's numpy ulp caveat applies). If
   graph-core already has a two-colouring, reuse it only when its visiting order is identical;
   otherwise write the port and say why.
4. Map `BIPARTITE_3D` to it in the conformance `motor.rs`; re-pin; rewrite repair 11's
   bipartite sentence.

Paths: `layout/basic_3d/` (new module), `registry.rs` (additive), the conformance `motor.rs`,
`rows.rs`, `gaps.rs`, `baseline/table.rs` (BIPARTITE_3D), the doc, `docs/measurements/sg-bipartite3d.md`.

Done when: `BIPARTITE_3D` reaches f32 1020/1020 or names the fixtures and the reason it does
not, and the sg-common done-when holds.
