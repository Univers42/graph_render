# Job p13-gv2-dot (agent build, native dot port)

Why: the user wants every SciGraphs layout in the motor and the studio picker, matching SciGraphs'
output exactly (2026-09-30). SciGraphs reaches Graphviz `dot` through `_graphviz_engine_layout`
(`SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:340`, engine list `yifan_hu.py:7-16`,
dispatched at `dispatcher.py:140`). The user decided that the native Graphviz engines must match
Graphviz's own output (`docs/decisions/graphviz-oracle.md`). The row is `GRAPHVIZ_DOT` in
`docs/measurements/scigraphs-coverage.md`.

The model to follow is `prompts/jobs/p13-gv1-twopi.md`. Read it in full: its Facts, its Do steps 1–6,
its registry, hashgate, ledger and determinism rules, and its Done-when all apply here with `twopi`
replaced by `dot`. Only the differences are listed below.

Engine facts. These come from the Graphviz documentation and are to be verified against the oracle
image, not trusted:
- dot is layered, in four passes: network-simplex ranking, mincross ordering (median plus transpose), network-simplex
  x-coordinates, and splines. Only the node centres are gated; edges are emitted as the motor's polylines through
  the virtual nodes, and the metadata says so.
- It is deterministic for a given input order. Emit the DOT in fixture order.
- The motor already has `layout.dag.sugiyama`, a different function, and the module doc says how they differ.
- This is the largest port. If it will not fit in one job, stop after the rank and mincross passes with their own
  tests, and write the remaining passes as a brief draft in the return block.

Differences from the twopi brief:
1. Id `layout.dag.dot`. Module `crates/graph-core/src/layout/graphviz/dot.rs` (child modules and `tests/`
   as needed). Registry metadata goes in its own file, `crates/graph-core/src/registry/graphviz_dot.rs`, so that
   parallel engine jobs touch `registry.rs` only with a one-line additive `mod` and a `LAYOUTS` append.
   Graphviz is EPL-1.0: read its source (`$GM_SCRATCH/refs`) as an algorithm reference and never translate it
   line by line (decided 2026-09-30).
2. Measure first. Run the oracle (`harness/oracle-graphviz.py <fixtures> dot <out>` in `ge-graphviz-oracle`)
   twice and `cmp` the outputs. Then run it at `-Gstart` 1, 7 and 99. Record both results in
   `docs/measurements/p13-gv2-dot.md`. If the start seed changes the output, the native port must reproduce
   Graphviz's seeded initial placement and its random-number sequence for `-Gstart=1`, and the metadata says so.
3. The differential is `graph-cli oracle-graphviz --engine dot`. If twopi has already landed an
   `oracle-twopi`, generalise it to take the engine name (one command, not one per engine) and keep
   `oracle-twopi` working as an alias. The metric is the largest absolute coordinate gap in points after
   both arms are rescaled to the same bounding box. Byte-exact agreement is required on the small closed
   cases (one node, two nodes, a path, a star, a cycle) wherever the engine gives a closed answer.
4. The ceiling is the next power of ten above the worst measured gap over 1000 seeds, and it is never
   widened to make a row pass. If the port cannot reach agreement, write the measured gaps and the named
   cause in the measurements file and leave the row at `Status::Implemented` (never `gated`).
5. Rows file `scripts/orch/rows/p13-gv2-dot.rows`, built like twopi's, with a negctl that perturbs this
   engine's output and must exit non-zero.
6. In `docs/measurements/scigraphs-coverage.md`, flip only the `GRAPHVIZ_DOT` row and update the counts block.

Paths you may touch: the module and registry file above, one-line additive edits to `registry.rs`,
`layout/mod.rs` or `layout/graphviz/mod.rs`, the hashgate pinned record
`crates/graph-cli/src/hashgate/tests/report.rs`, `capabilities/registry.rs` and `registry/unproven.rs`, the
oracle-graphviz subcommand modules in graph-cli and `command.rs`, `harness/oracle-graphviz.py` (additive
only), `crates/graph-wasm/src/exports/build.rs` (layout count), the rows file and measurements file above,
and the coverage table. Nothing else.

Done when: every Done-when item of the twopi brief holds for `layout.dag.dot`, and the return block pastes
the oracle determinism results, the worst gap over 1000 seeds, and the real exit code of each command.

Note (2026-10-01), read before writing oracle code: develop has **one** generic Graphviz differential. `crates/graph-cli/src/oracle_python/graphviz.rs` holds `by_engine` and `ENGINES`, and `cli.rs` holds `emit-graphviz-fixtures --engine <e>` / `oracle-graphviz --engine <e>`. Add your engine as a `Differential` in `oracle_python/<engine>.rs` (shape: `oracle_python/osage.rs`) plus one `by_engine` arm and one `ENGINES` entry. Key engine specifics inside `harness/oracle-graphviz.py`. Do not add another subcommand, dispatcher or fixture layout. Append your `Capability` after the last entry in `LAYOUTS`. Other Graphviz engines (neato, patchwork, circo) land in parallel, so before you finish, `git fetch origin` and read `origin/develop`'s versions of those files.
