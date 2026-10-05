# Job p13-gv3-dot-diff (agent build, branch p13-gv3-dot-diff, worktree ~/goinfre/wt/p13-gv3-dot-diff)

Context: `layout.dag.dot` is registered on develop (job `p13-gv3-dot-position`) with its rank,
mincross and position passes, but `capabilities` cannot rate it above `implemented`: it has no
graph-cli differential against Graphviz's own `dot` and no hashgate knob. Read first, in full:
`docs/measurements/p13-gv2-dot.md` (the "Mincross" and "Position" sections give the agreement
counts you will see again), `docs/decisions/graphviz-oracle.md`, and the twopi and osage wiring
this job copies: `crates/graph-cli/src/oracle_python/twopi.rs` (bare graph, no `box` column,
closed form: the closest model) and the osage knob, commit `c020ff08`
(`git show --stat c020ff08`).

Licence, hard rule: Graphviz is EPL-1.0. Read `~/goinfre/refs/graphviz-16.1.0/` for behaviour
only. Never copy its text or comments.

Exact tasks:
1. The differential. New `crates/graph-cli/src/oracle_python/dot.rs`, modelled on `twopi.rs`:
   `pub const DOT: Differential { name: "dot", ceilings: &[("layout.dag.dot", "dot", CEILING)], line }`.
   The fixture line is twopi's (gate model, `source`/`target`, our coordinates under `"dot"`),
   with no `box` column: the dot port sizes nodes from `text_width.rs`, the same widths Graphviz
   gives the default labels, so the harness draws the bare graph. Add `mod dot;` beside the other
   engines, `"dot"` to `ENGINES` (7 -> 8) and to `by_engine` in `oracle_python/graphviz.rs`.
2. The harness, additively. `harness/oracle-graphviz.py` needs no new branch in `engine_arms`
   (no `box`). Add `FRAMED_CLOSED["dot"]` in `harness/gv_frames.py` with the six closed cases'
   points exactly as the measurement file's table "The six closed cases, as the oracle prints
   them" prints them. First check those six cases are the same six graphs `framed_cases` draws
   (the graphs imported from `harness/oracle-twopi.py`); if they are not, stop and report.
3. Measure before you pick a ceiling. Run by hand, in this order, and keep each output:
   `scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine dot --seeds 1000`,
   `scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle python3 harness/oracle-graphviz.py target/dot-fixtures dot target/gv-dot --differential`,
   then `scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine dot` with a
   provisional `CEILING = 1e-1`. Report the per-seed worst gap: how many seeds are at or under
   the printed resolution (`-Tplain` prints five significant digits; see `twopi.rs` `CEILING`
   for the reasoning), the median, the worst, and the worst seed's id.
   - If every seed is at the printed resolution: `CEILING = 1e-1`, with a doc comment that says so
     with the numbers.
   - If not: the ceiling is the next power of ten above the measured worst gap, and its doc
     comment says plainly it records a **disagreement**, how many seeds carry it, and that the
     cause is the order/position disagreements already counted in the measurement file. Do not
     fix the layout in this job.
4. The status. In `crates/graph-cli/src/capabilities/registry/unproven.rs` route
   `"layout.dag.dot" => Some(("oracle-dot", Status::...))` beside the osage and fdp rows, with a
   comment citing the measurement. `Gated` only if task 3 found printed resolution on every seed
   AND task 5's knob is wired; otherwise `Implemented`, and the comment says which of the two is
   missing. Update `crates/graph-cli/src/capabilities/tests/*` only where a test asserts that
   row's status or a count that the new row moves; regenerate count literals from the failing
   test output, never by guessing.
5. The hashgate knob `GM_MUTATE_DAG_DOT_NODES`, record `hashgate-control-dag-dot-nodes`, exactly
   the osage pattern (`git grep -n 'OSAGE_LAYOUT_STAGES\|Packing[O]sageNodes\|PACKING_OSAGE_NODES' -- crates`):
   a `DOT_LAYOUT_STAGES: [Stage; 1]` table in `hashgate/knobs.rs` chained into `all()`, the
   `Knob` variant in `knob/kind.rs`, its arms in `knob/arms.rs`, `knob/records.rs`,
   `knob/wiring.rs`, a test file `hashgate/tests/knob/dot.rs` copied from `knob/osage.rs`'s shape,
   the knob list in `crates/graph-cli/tests/common/mod.rs`, and `tests/cli_p3.rs`. If
   `coverage.rs` reports `Gap::NoControl` for the dot row before the knob, it must not after.
   The knob must turn `hashgate --seeds 8` red on `layout.dag.dot` and on no other stage: paste
   the stage list the red run names.
6. Rows. New `scripts/orch/rows/p13-gv3-dot.rows`, copied from `scripts/orch/rows/p13-gv1-osage.rows`
   with osage -> dot: the merge floor, the two wasm32 builds, `hashgate-8`, `negctl-degree`,
   `negctl-dot-nodes|nonzero|...GM_MUTATE_DAG_DOT_NODES=1...`, `dot-emit-1000`, `dot-oracle-1000`,
   `dot-check-1000`, `dot-oracle-determinism`, `dot-oracle-start-inert`,
   `dot-oracle-perturb-negctl` and `dot-check-negctl`. Drop the osage `negctl-osage-sizes` row
   (dot has no `box` column) and say why in one comment line. Every container through
   `scripts/orch/drun`; run `scripts/orch/drun-check.sh` (expect exit 0). Then append the same
   dot rows to `scripts/orch/rows/develop-full.rows`, ADDITIVE only, beside the osage rows.
7. Add a section "## Differential (2026-10-04)" at the end of `docs/measurements/p13-gv2-dot.md`:
   the commands of task 3, the distribution, the ceiling and why, the knob's red run. Do not edit
   any other section.

Limits:
- Files at most 300 lines (split into child modules), functions at most 40 lines and 4 parameters.
- No `HashMap`. No new dependency. Every heuristic carries a `Ponytail:` line.
- Do not touch `crates/graph-core` except reading it; the layout is not this job's to change.

Paths allowed:
- `crates/graph-cli/src/oracle_python.rs`, `crates/graph-cli/src/oracle_python/**`
- `crates/graph-cli/src/capabilities/**`
- `crates/graph-cli/src/hashgate/**`
- `crates/graph-cli/tests/common/mod.rs`, `crates/graph-cli/tests/cli_p3.rs`,
  `crates/graph-cli/tests/cli_ledger.rs` (only at a failing literal)
- `harness/gv_frames.py`, `harness/oracle-graphviz.py`
- `scripts/orch/rows/p13-gv3-dot.rows`, `scripts/orch/rows/develop-full.rows` (additive)
- `docs/measurements/p13-gv2-dot.md` (the new section only), `prompts/jobs/p13-gv3-dot-diff.md`

Not allowed: everything else, in particular `crates/graph-core`, `crates/graph-wasm`, `packages`,
`app`, `server`, `deploy`.

Done when, each with its command and exit pasted:
- `scripts/orch/gate.sh target/gate-dot-diff scripts/orch/rows/p13-gv3-dot.rows` all PASS
  (the `nonzero` rows count as PASS when they fail);
- `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` exits 0, and
  `capabilities` prints the `layout.dag.dot` row with the status task 4 chose;
- the task-3 distribution and the knob's red stage list are in the measurement file.
