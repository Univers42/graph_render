# Job p13-gv1-patchwork (agent build, native patchwork port)

Why: the user wants every SciGraphs layout in the motor and the studio picker, matching SciGraphs'
output exactly (2026-09-30). SciGraphs reaches Graphviz `patchwork` through `_graphviz_engine_layout`
(`SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:340`, engine list `yifan_hu.py:7-16`,
dispatched at `dispatcher.py:140`). The user decided that the native Graphviz engines must match
Graphviz's own output (`docs/decisions/graphviz-oracle.md`). The row is `GRAPHVIZ_PATCHWORK` in
`docs/measurements/scigraphs-coverage.md`.

The model to follow is `prompts/jobs/p13-gv1-twopi.md`. Read it in full: its Facts, its Do steps 1–6,
its registry, hashgate, ledger and determinism rules, and its Done-when all apply here with `twopi`
replaced by `patchwork`. Only the differences are listed below.

Engine facts. These come from the Graphviz documentation and are to be verified against the oracle
image, not trusted:
- patchwork is a squarified treemap over the cluster tree. Each node is a square whose area comes from the
  `area` attribute (default 1). A flat graph with no clusters becomes one squarified field of equal squares.
- Edges are not laid out. The motor emits them as lines between the node centres, and the metadata says so.

Differences from the twopi brief:
1. Id `layout.treemap.patchwork`. Module `crates/graph-core/src/layout/graphviz/patchwork.rs` (child modules and `tests/`
   as needed). Registry metadata goes in its own file, `crates/graph-core/src/registry/graphviz_patchwork.rs`, so that
   parallel engine jobs touch `registry.rs` only with a one-line additive `mod` and a `LAYOUTS` append.
   Graphviz is EPL-1.0: read its source (`$GM_SCRATCH/refs`) as an algorithm reference and never translate it
   line by line (decided 2026-09-30).
2. Measure first. Run the oracle (`harness/oracle-graphviz.py <fixtures> patchwork <out>` in `ge-graphviz-oracle`)
   twice and `cmp` the outputs. Then run it at `-Gstart` 1, 7 and 99. Record both results in
   `docs/measurements/p13-gv1-patchwork.md`. If the start seed changes the output, the native port must reproduce
   Graphviz's seeded initial placement and its random-number sequence for `-Gstart=1`, and the metadata says so.
3. The differential is `graph-cli oracle-graphviz --engine patchwork`. If twopi has already landed an
   `oracle-twopi`, generalise it to take the engine name (one command, not one per engine) and keep
   `oracle-twopi` working as an alias. The metric is the largest absolute coordinate gap in points after
   both arms are rescaled to the same bounding box. Byte-exact agreement is required on the small closed
   cases (one node, two nodes, a path, a star, a cycle) wherever the engine gives a closed answer.
4. The ceiling is the next power of ten above the worst measured gap over 1000 seeds, and it is never
   widened to make a row pass. If the port cannot reach agreement, write the measured gaps and the named
   cause in the measurements file and leave the row at `Status::Implemented` (never `gated`).
5. Rows file `scripts/orch/rows/p13-gv1-patchwork.rows`, built like twopi's, with a negctl that perturbs this
   engine's output and must exit non-zero.
6. In `docs/measurements/scigraphs-coverage.md`, flip only the `GRAPHVIZ_PATCHWORK` row and update the counts block.

Paths you may touch: the module and registry file above, one-line additive edits to `registry.rs`,
`layout/mod.rs` or `layout/graphviz/mod.rs`, the hashgate pinned record
`crates/graph-cli/src/hashgate/tests/report.rs`, `capabilities/registry.rs` and `registry/unproven.rs`, the
oracle-graphviz subcommand modules in graph-cli and `command.rs`, `harness/oracle-graphviz.py` (additive
only), `crates/graph-wasm/src/exports/build.rs` (layout count), the rows file and measurements file above,
and the coverage table. Nothing else.

Done when: every Done-when item of the twopi brief holds for `layout.treemap.patchwork`, and the return block pastes
the oracle determinism results, the worst gap over 1000 seeds, and the real exit code of each command.

Resume note (2026-10-01): a previous run left partial edits in this worktree (`git status`). Keep them,
read them yourself, finish the job. Dispatch no subagents. The code is written: run the checks, fix what
is red, and write the return block before anything else. End with the return block.

Resume note 2 (2026-10-01): your patchwork work is committed (status: done, checks green on your base). origin/develop has been merged into this worktree, and the merge is **in progress** (MERGE_HEAD is set). 13 files conflict (`git diff --name-only --diff-filter=U`). Develop already has **one** generic Graphviz differential, and yours must plug into it rather than sit beside it:
- Keep develop's `oracle_python/graphviz.rs` (`by_engine`, `ENGINES`, `default_dir`, `engine_parser`) and develop's `EmitGraphvizFixtures`/`OracleGraphviz` variants and arms in `cli.rs`, aliases included. Drop your own `graphviz::emit/check`, your `graphviz_arm.rs` and your `--fixtures/--graphviz` flags.
- Port your differential to a `Differential` in `oracle_python/patchwork.rs`, shaped like `oracle_python/osage.rs`. Add `"patchwork"` to `by_engine` and `ENGINES`. Move your `graphviz/tests.rs` cases onto it where they still apply.
- `harness/oracle-graphviz.py`: develop's interface (one `target/<engine>-fixtures` dir, `<engine>-result.json`), with patchwork's specifics keyed by engine. Update `scripts/orch/rows/p13-gv1-patchwork.rows` and your measurement doc to the generic commands.
- `registry.rs`: union. Patchwork's `Capability` goes after develop's last entry, and the `LAYOUTS` count is develop's + 1. Keep the file within 300 lines.
- Knobs: develop moved `ALL` and `env` into `knob/arms.rs`. Add `PatchworkNodes` there and to the enum, `setting.rs`, `records.rs`, `tests/knob/table.rs` (count + 1), `tests/common/mod.rs` KNOBS and the `cli_p3.rs` count.
- Count tests: `roundtrip/tests.rs` `"snapshots"` = 5 × (layouts + 1). `snapshot_cmd/tests.rs`: add the patchwork id pair after develop's last pair. `report.rs` `equal`: union of both maps (develop's string plus your patchwork key). `capabilities/tests/registry.rs` and `registry/unproven.rs`: keep both arms (osage's and patchwork's).
- `scigraphs-coverage.md`: union, then recount.
Then run fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, hashgate 8, `GM_MUTATE_PATCHWORK_NODES=1` hashgate 8 (expect 1) and `oracle-graphviz --engine patchwork` (re-emit and re-run the oracle on the generic paths). Commit with `git commit` (two parents, since MERGE_HEAD is set) and push. Do not run `git merge` or `git rebase`. Dispatch no subagents. End with the return block.
