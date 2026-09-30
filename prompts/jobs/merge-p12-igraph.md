# Job merge-p12-igraph (agent build, merge and finish)

Why: `p12-igraph` (head 8238039) is the third branch of the merge train (`prompts/RESUME.md`
"Branches pushed but NOT on develop"): igraph layouts DRL, LGL, DavidsonHarel, Graphopt, Kamada-Kawai,
Fruchterman-Reingold; `registry/igraph.rs`; `harness/oracle-igraph.py`; a capabilities/registry split.

First, in this worktree (a branch made from develop): `git merge --no-commit origin/p12-igraph`. Resolve
every conflict by editing files, keeping both intents (CLAUDE.md "Parallel branches": registry, lib.rs,
capabilities are edited additively). After the merge, never run git commands that change state.

Finish, items (a)-(g) from RESUME:
a. Compile the registry split on top of develop's registry (p12-t1's yifan_hu and any later ids stay).
b. Tests that hard-code layout counts: `crates/graph-cli/src/hashgate/tests/report.rs` (the `equal`
   set), snapshot_cmd, roundtrip, the `cli_ledger` tests: look rows up by id, never by index or count.
c. The Rust `oracle-igraph` reads 0 cases for DRL/LGL while the Python side writes 100: find the
   fixture/case-name mismatch (`harness/oracle-igraph.py` vs the Rust reader) and fix the reader or
   writer so both agree; show the case count after the fix.
d. DavidsonHarel measured 51.91 and Graphopt 15.39 against a 2e0 ceiling. Do not widen a ceiling to
   pass. Measure with the metric the other force layouts use against their oracle (read how
   `layout.force.*` rows are gated) and write `docs/measurements/p12-igraph-ceilings.md`: metric, value,
   ceiling, why. If a layout cannot meet a defensible ceiling, register it `implemented`, not `gated`.
e. `.hypot` at `drl/tests.rs:82` and `lgl/tests.rs:32` goes through `libm` (D-rules, CLAUDE.md "Determinism").
f. Each new layout has a hashgate entry and a negctl that turns it red (the knob list exists once,
   `crates/graph-cli/tests/common/mod.rs` and `Knob::ALL`; add a knob only if no existing knob perturbs it).
g. Lint: no new `#[allow]` without a reason; house limits (40 lines/fn, 300 lines/file, 4 params).

Checks (paste each last line): fmt --check, clippy -D warnings, `cargo test --workspace --no-fail-fast`,
wasm32 build of graph-core, `hashgate --seeds 8` and the `GM_MUTATE_REFERENCE_DEGREE=9` negctl (exit 1),
`capabilities --check`, `codegen --check`. Python side: run `harness/oracle-igraph.py` in its image only
if the image exists (`docker images | grep igraph`); otherwise say "not run".

Paths you may touch: the conflicted files, `crates/graph-core/src/layout/{drl,lgl,davidson_harel,graphopt,kamada_kawai,fruchterman_reingold}*`,
`crates/graph-core/src/registry*`, `crates/graph-cli/**`, `harness/oracle-igraph.py`,
`docs/measurements/p12-igraph-ceilings.md`.

Done when: no conflict marker, the checks pass, and the return block lists (a)-(g) one line each.
