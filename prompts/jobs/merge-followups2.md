# Job merge-followups2 (agent build, conflict resolution only)

The orchestrator has run `git merge --no-commit origin/followups2` into this worktree (a branch made
from develop). Git state is left as the merge left it: 13 conflicted files. Your job is to resolve
them by editing files. Never run git commands that change state; never delete a file (report it).

Context (`prompts/RESUME.md`, HANDOFF 2026-09-30 00:40, "Branches pushed but NOT on develop"):
- `followups2` = phase-7 follow-ups: graph-wasm analysis/post registries, 15 ANALYSIS/POST stage
  knobs, staged hashgate. develop has p11 (compute tiers: 11 knobs = the 10 original + `SplitSum`,
  `exec/` runners, Threads/Tiers arms) and the `limits` split (files over 300 lines split into child
  modules, e.g. `graph-wasm/src/analysis/tests.rs` → `analysis/tests/{json,registry}.rs`).
- Both sides are additive. **Keep both intents in every hunk.**
- The knob union is 26: the 10 original knobs, then followups2's 15 in its table order, then
  `SplitSum` LAST, so `the_analysis_and_post_controls_are_the_knobs_table`'s slice against
  `knobs::ANALYSIS_POST_STAGES` still lines up. Check every p11 test that indexes `Knob::ALL`
  (`git grep -n 'Knob::ALL' crates/`) and fix indices or counts it assumes.
- `crates/graph-cli/tests/common/mod.rs` merged cleanly with `KNOBS: [&str; 25]`. CLAUDE.md says the
  knob list exists once: make that list and `Knob::ALL`'s env names agree (26, same order), or show
  with `file:line` why SplitSum is deliberately not an env knob there.

Conflicts (`git diff --name-only --diff-filter=U`; count of `<<<<<<<` hunks):
- `crates/graph-cli/src/hashgate/knob.rs` (5), `hashgate/tests/knob.rs` (3), `tests/mod.rs` (1),
  `tests/report.rs` (1), `tests/stages.rs` (1)
- `crates/graph-cli/tests/cli.rs` (3), `tests/cli_ledger.rs` (3). Tests that find a capability row
  look it up by id, never by index (CLAUDE.md).
- `crates/graph-wasm/src/analysis.rs` (2); `analysis/tests/json.rs` (5, add/add),
  `analysis/tests/registry.rs` (3, add/add), `post/tests/registry.rs` (2, add/add).
- Modify/delete: `crates/graph-wasm/src/analysis/tests.rs` and `post/tests.rs` were split away on
  develop and modified on followups2 (followups2's version is in the tree). Port every change
  followups2 made to them (`git diff origin/develop...origin/followups2 -- <path>`) into the split
  child modules, then list both old files under `delete:` in your return block (the orchestrator
  deletes them). Make sure no `mod tests;` declaration points at both a `tests.rs` and a `tests/`.
- `report.rs`'s expected `equal` set must list `layout.force.yifan_hu` (on develop since p12-t1).

House limits hold on every file you touch: ≤ 40 lines per function, ≤ 300 lines per file (split into
child modules), no new `#[allow]` without a reason. Determinism rules: CLAUDE.md "Determinism".

Checks you run (filtered, cheap; never a timed gate):
- `git diff --check` and `git grep -n -e '^<<<<<<<' -e '^>>>>>>>' -e '^=======$' -- crates` are empty.
- `scripts/orch/gr cargo fmt --all --check`
- `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings`
- `scripts/orch/gr cargo test -p graph-cli knob`, `... -p graph-cli --test cli`, `... --test cli_ledger`,
  `scripts/orch/gr cargo test -p graph-wasm`
- `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` and `-- codegen --check`

Paths you may touch: the 13 conflicted files, the child modules they declare, `crates/graph-cli/tests/common/mod.rs`,
and any test file that indexes `Knob::ALL`. Nothing else.

Done when: no conflict marker remains; the checks above pass (paste each command's last line);
the return block lists `delete:` paths, the final knob order (26 names), and every test you changed
because of an index or count.
