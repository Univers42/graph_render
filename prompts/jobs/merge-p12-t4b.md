# Job merge-p12-t4b (agent build; merge develop into p12-t4b and resolve the conflicts)

Why: `land.sh p12-t4b` stopped on 8 conflicts when it merged origin/develop into the branch
(log: `$GM_SCRATCH/orch/logs/land-p12-t4b.out`). p12-t4b brings the 3D arms of five force kernels
and spectral (`crates/graph-core/src/layout/{spectral/z_axis,force/*,forceatlas2*}`, `registry/arms_3d`);
develop has since gained the sg-* conformance work and other layouts.

Conflicted files: `crates/graph-cli/src/capabilities/registry/unproven.rs`,
`crates/graph-cli/src/capabilities/tests/registry.rs`, `crates/graph-cli/src/hashgate/tests/report.rs`,
`crates/graph-cli/src/oracle_python.rs`, `crates/graph-cli/src/oracle_python/cli.rs`,
`crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs`, `crates/graph-core/src/layout/forceatlas2.rs`,
`crates/graph-core/src/layout/forceatlas2/state.rs`.

Do:
1. First, in this worktree (a branch made from develop): `git merge --no-commit origin/p12-t4b`. Resolve
   every conflict by editing files, keeping
   both intents (CLAUDE.md "Parallel branches"): registry and test lists are unions; in
   `forceatlas2*` keep develop's behaviour for 2D byte for byte and p12-t4b's 3D arm on top.
2. No other edits. A conflict whose two sides cannot both hold is a stop: report it under
   "decisions needed" with both sides quoted and your recommendation.
3. Run fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, the wasm32 build of
   graph-core, and `hashgate --seeds 8` with its `GM_MUTATE_REFERENCE_DEGREE=9` negative control.

Done when: no conflict markers (`git diff --check` clean, `git grep -n '^<<<<<<<\|^>>>>>>>'` empty), every
command above exits as required, and the return block pastes each real exit code. After the merge, never run git
commands that change state; the gate commits.
