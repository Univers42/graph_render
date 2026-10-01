# Job merge-p12-t2 (agent build, merge and finish)

Why: `p12-t2` (head b1aa19d) ports SciGraphs' `spring` and `circular.hierarchy` layouts, with Python
oracles `harness/oracle-spring.py` and `harness/oracle-circular-hierarchy.py`, knobs
`GM_MUTATE_SPRING_ITERATIONS` and `GM_MUTATE_CIRCULAR_HIERARCHY_NODES`. On its own tree every check
passed. It branched before igraph, twopi, force-wasm, tier-settle, studio-live and p13-3d, so 22 files conflict.

First, in this worktree (a branch made from develop): `git merge --no-commit origin/p12-t2`. Resolve
every conflict by editing files, keeping both intents (CLAUDE.md "Parallel branches": registry, lib.rs,
capabilities, knob lists are edited additively). After the merge, never run git commands that change state.

Known shapes of the conflicts:
a. `crates/graph-core/src/registry.rs`: keep develop's `LAYOUTS` (igraph's 6 entries, `twopi::ID`, and
   whatever else is there) and add p12-t2's two entries; fix the array length.
b. Python oracle CLI: develop moved the oracle subcommands into `crates/graph-cli/src/oracle_python/cli.rs`
   (`Cli` enum, flattened into `command.rs` as `PythonOracle`). Take develop's `command.rs`/`main.rs`/
   `oracle_python.rs` and add p12-t2's emit/oracle subcommands as variants and run arms in `cli.rs`, the way
   `EmitTwopiFixtures`/`OracleTwopi` are done.
c. Knob lists stay in sync, each gaining the two p12-t2 knobs: `Knob::ALL` (`hashgate/knob.rs`, length
   fixed), `knob/records.rs`, `knob/setting.rs`, `hashgate/tests/knob/table.rs`, `tests/common/mod.rs`
   `KNOBS`, and the count asserted in `tests/cli_p3.rs`. `ForceSessionGravity` stays last in `ALL`.
d. Count tests (`hashgate/tests/report.rs` record, `snapshot_cmd/roundtrip/tests.rs` "snapshots",
   `snapshot_cmd/tests.rs` id pairs, `capabilities/tests/*`, `tests/cli*.rs`, `tests/snapshot.rs`): union
   of both sides, counts recomputed from the merged registry, never guessed.
e. p13-3d added a `dim` to snapshots: p12-t2's layouts are 2D, so they must produce `dim` 0 / the 2D
   version label through `label_for` (`crates/graph-contract/src/snapshot/dim.rs`) like every other layout.
f. `docs/measurements/scigraphs-coverage.md`: both sides' rows; recount the totals.
g. House limits: 40 lines/fn, 300 lines/file (split into child modules if a merge pushes a file over),
   4 params, no new `#[allow]` without a reason.

Checks (paste each last line): fmt --check, clippy -D warnings, `cargo test --workspace --no-fail-fast`,
wasm32 build of graph-core, `hashgate --seeds 8` and the `GM_MUTATE_REFERENCE_DEGREE=9` negctl (exit 1),
`GM_MUTATE_SPRING_ITERATIONS=3 hashgate --seeds 8` (exit 1), `force-gate` rows of `scripts/orch/rows/quick.rows`,
`roundtrip --seeds 100`, `codegen --check`. Rerun `emit-spring-fixtures --seeds 20`, the spring oracle in
`ge-python-oracle`, and `oracle-spring` only if the image exists; otherwise say "not run".

Paths you may touch: the conflicted files, `crates/graph-core/src/layout/{force/spring,circular}*`,
`crates/graph-core/src/registry*`, `crates/graph-cli/**`, `harness/oracle-{spring,circular-hierarchy}.py`,
`docs/measurements/{scigraphs-coverage,p12-t2}.md`.

Done when: no conflict marker, the checks pass, and the return block lists (a)-(g) one line each.
