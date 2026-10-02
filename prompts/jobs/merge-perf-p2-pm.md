# Job merge-perf-p2-pm (agent build; merge perf-p2-pm onto develop and resolve the conflicts)

Why: perf-p2-pm (`origin/perf-p2-pm`) conflicts with develop in
`crates/graph-cli/src/hashgate/tests/report.rs`, `crates/graph-cli/src/snapshot_cmd/tests.rs` and
`crates/graph-core/src/registry.rs` (measured with `git merge-tree`). Released after merge-p12-t4b
lands, which touches the same registry lists.

Do:
1. In this worktree (a branch made from develop): `git merge --no-commit origin/perf-p2-pm`. Resolve
   every conflict keeping both intents (CLAUDE.md "Parallel branches"): registry and test lists are
   unions, in registry order; expected counts in tests are recomputed from the merged lists, never
   guessed.
2. No other edits. A conflict whose two sides cannot both hold is a stop: report it under
   "decisions needed" with both sides quoted and your recommendation.
3. Run fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, the wasm32 build of
   graph-core, and `hashgate --seeds 8` with its `GM_MUTATE_REFERENCE_DEGREE=9` negative control.

Report: `docs/measurements/merge-perf-p2-pm.md` (each conflict and its resolution).

Done when: no conflict markers (`git diff --check` clean, `git grep -n '^<<<<<<<\|^>>>>>>>'` empty),
every command above exits as required, and the return block pastes each real exit code. After the
merge, never run git commands that change state; the gate commits.
