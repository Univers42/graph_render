# Job sg-sugiyama (agent build, SciGraphs conformance: SUGIYAMA)

Read `prompts/jobs/sg-common.md` first. Row: `SUGIYAMA` (gaps `G_NO_ITERATIONS`,
`G_SCALE_FIXED_LAYER` at `sugiyama/mod.rs:50`).

Why: SciGraphs' sugiyama (`hierarchical.py:638-692`) is its own pipeline, not dagre:
`_acyclic_arcs` → `_longest_path_layers` → `_reduce_slack` → `_build_ordering_graph` →
`_init_order` → `_order_layers` (8 iterations, transpose 4 on small graphs) → `_assign_x`
(priority method, 4 passes). It then normalises each axis on its own:
`x = ((x - lo) / width * 2 - 1) * scale`, `y = (layer / max_layer * 2 - 1) * scale`, `z = 0`.
The motor's output is in layer-spacing units with no scale, so today the row compares
different units before it compares any ordering.

Do:
1. Measure the row. Then apply SciGraphs' per-axis normalisation to the motor arm in the
   conformance code (one function, its own test against the formula above on a 3-node chain).
   Re-measure: this isolates the shape gap from the unit gap. Close `G_SCALE_FIXED_LAYER` or
   narrow its note to what remains.
2. Stage diff on the diamond DAG and lesmis: call each SciGraphs stage function directly in
   `ge-python-oracle` (import from the submodule, never copy) and dump its output (layers,
   per-layer order, x). Dump graph-core's corresponding stage. Name the first stage that
   differs and the first node that differs in it.
3. RED: a graph-core test that pins SciGraphs' output of that stage on the smallest graph
   that shows the difference. GREEN: the smallest change in graph-core. The dagre crossing
   counts in `docs/decisions/sugiyama-heuristics.md` must stay green; if the SciGraphs order
   and the dagre gate conflict, add a SciGraphs-ordered variant as a new registered layout
   (`layout.dag.sugiyama_scigraphs`, full Metadata) and map the row to it, leaving
   `layout.dag.sugiyama` byte-identical.
4. Repeat 2-3 for the next divergent stage until the row is at `f32 N/N` or a stage's
   difference is a tie order the reference does not define (write it down and stop).
5. Re-pin the row.

Paths: `layout/sugiyama/**` (or a new sibling module for the variant), the conformance
`motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs` (one row),
`docs/measurements/sg-sugiyama.md`, `docs/decisions/sugiyama-heuristics.md` if a variant is added.

Done when: the per-stage table (stage, first divergent node, before/after) is pasted, the
dagre differential (`oracle-layouts --dag`) is unchanged, and the sg-common done-when holds.
