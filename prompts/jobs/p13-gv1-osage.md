# Job p13-gv1-osage (agent build, native osage port)

Why: the user wants every SciGraphs layout in the motor and the studio picker, matching SciGraphs'
output exactly (2026-09-30). SciGraphs reaches Graphviz `osage` through `_graphviz_engine_layout`
(`SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:340`, engine list `yifan_hu.py:7-16`,
dispatched at `dispatcher.py:140`). The user decided that the native Graphviz engines must match
Graphviz's own output (`docs/decisions/graphviz-oracle.md`). The row is `GRAPHVIZ_OSAGE` in
`docs/measurements/scigraphs-coverage.md`.

The model to follow is `prompts/jobs/p13-gv1-twopi.md`. Read it in full: its Facts, its Do steps 1–6,
its registry, hashgate, ledger and determinism rules, and its Done-when all apply here with `twopi`
replaced by `osage`. Only the differences are listed below.

Engine facts. These come from the Graphviz documentation and are to be verified against the oracle
image, not trusted:
- osage packs clusters recursively with the pack and packmode attributes (default: array packing). A flat graph
  is one array of nodes, filled row by row in the order the nodes were declared.
- It is closed form: no iteration.

Differences from the twopi brief:
1. Id `layout.packing.osage`. Module `crates/graph-core/src/layout/graphviz/osage.rs` (child modules and `tests/`
   as needed). Registry metadata goes in its own file, `crates/graph-core/src/registry/graphviz_osage.rs`, so that
   parallel engine jobs touch `registry.rs` only with a one-line additive `mod` and a `LAYOUTS` append.
   Graphviz is EPL-1.0: read its source (`$GM_SCRATCH/refs`) as an algorithm reference and never translate it
   line by line (decided 2026-09-30).
2. Measure first. Run the oracle (`harness/oracle-graphviz.py <fixtures> osage <out>` in `ge-graphviz-oracle`)
   twice and `cmp` the outputs. Then run it at `-Gstart` 1, 7 and 99. Record both results in
   `docs/measurements/p13-gv1-osage.md`. If the start seed changes the output, the native port must reproduce
   Graphviz's seeded initial placement and its random-number sequence for `-Gstart=1`, and the metadata says so.
3. The differential is `graph-cli oracle-graphviz --engine osage`. If twopi has already landed an
   `oracle-twopi`, generalise it to take the engine name (one command, not one per engine) and keep
   `oracle-twopi` working as an alias. The metric is the largest absolute coordinate gap in points after
   both arms are rescaled to the same bounding box. Byte-exact agreement is required on the small closed
   cases (one node, two nodes, a path, a star, a cycle) wherever the engine gives a closed answer.
4. The ceiling is the next power of ten above the worst measured gap over 1000 seeds, and it is never
   widened to make a row pass. If the port cannot reach agreement, write the measured gaps and the named
   cause in the measurements file and leave the row at `Status::Implemented` (never `gated`).
5. Rows file `scripts/orch/rows/p13-gv1-osage.rows`, built like twopi's, with a negctl that perturbs this
   engine's output and must exit non-zero.
6. In `docs/measurements/scigraphs-coverage.md`, flip only the `GRAPHVIZ_OSAGE` row and update the counts block.

Paths you may touch: the module and registry file above, one-line additive edits to `registry.rs`,
`layout/mod.rs` or `layout/graphviz/mod.rs`, the hashgate pinned record
`crates/graph-cli/src/hashgate/tests/report.rs`, `capabilities/registry.rs` and `registry/unproven.rs`, the
oracle-graphviz subcommand modules in graph-cli and `command.rs`, `harness/oracle-graphviz.py` (additive
only), `crates/graph-wasm/src/exports/build.rs` (layout count), the rows file and measurements file above,
and the coverage table. Nothing else.

Done when: every Done-when item of the twopi brief holds for `layout.packing.osage`, and the return block pastes
the oracle determinism results, the worst gap over 1000 seeds, and the real exit code of each command.
