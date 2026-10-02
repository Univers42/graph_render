# Job split-300 (agent build: bring every Rust file back under the 300-line house limit)

Read `scripts/orch/common.md` first. A pure move: no behaviour, no output and no public path changes.

Fact (2026-10-02, after the merge-p12-t4b merge, `git ls-files 'crates/*.rs' | xargs wc -l`):

| Lines | File |
|---|---|
| 394 | `crates/graph-core/src/layout/graphviz/osage/tests.rs` |
| 335 | `crates/graph-cli/src/capabilities/tests/registry.rs` |
| 325 | `crates/graph-core/src/layout/graphviz/circo/tests.rs` |
| 325 | `crates/graph-core/src/layout/forceatlas2/state.rs` |
| 316 | `crates/graph-core/src/layout/force/session.rs` |
| 315 | `crates/graph-cli/src/snapshot_cmd/tests.rs` |
| 313 | `crates/graph-core/src/layout/force/session/tests/verbs.rs` |
| 310 | `crates/graph-core/src/layout/graphviz/neato/solve.rs` |
| 306 | `crates/graph-cli/src/oracle_python/cli.rs` |
| 304 | `crates/graph-core/src/registry.rs` |

Re-run the count on your tree first; the table may have moved. Skip a file a queued or running job
also edits (`scripts/orch/oc-status.sh`, and the `Paths:` line of every brief named in
`scripts/orch/queue.txt` that has no `.land` file under `$GM_SCRATCH/orch/queue/`). List each skip
with the job that owns the file.

1. Split each remaining file into child modules along a seam the file already has: a group of tests
   on one subject, a helper set, one phase of the algorithm. Name each child for what it holds
   (`state/initial.rs`, `tests/ids.rs`), never `part2.rs`, `utils.rs` or `helpers.rs`.
2. Moves only. Every function, test, doc comment and `Ponytail:` line keeps its text. `pub` paths
   other crates use stay reachable from the same path (re-export from the parent where needed).
3. `registry.rs`: the order of `LAYOUTS` is load-bearing (`graph-wasm/src/exports/build.rs` maps by
   index). Keep `LAYOUTS` itself in `registry.rs` and move only the `Metadata` constants, or leave
   the file and say why.
4. A test count must not move: paste `cargo test --workspace --no-fail-fast 2>&1 | grep 'test result'`
   summed before and after (the passed count is equal).

Proof that nothing moved: `hashgate --seeds 8` (exit 0) with the same per-stage hashes as before
(paste both), its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero), `codegen --check` (exit 0),
`scripts/scigraphs-conformance.sh` (exit 0).

Paths: the files in the table and new child modules beside them; `docs/measurements/split-300.md`
(the before and after table, the skips).

Done when: quick.rows green; every file you did not skip is at most 300 lines; the test count is
equal; the measurement doc holds both tables.
