# Job lanes-merge (agent build: `layout.dag.lanes` shares a lane into a column already waiting for the target)

Why: `layout.dag.lanes` draws real histories much wider than the reference tool. On the plugin's
committer-time logs, git/git is 281 lanes against 181 for `git log --graph --date-order`
(`docs/measurements/dag-lanes.md`, "Commit DAGs").

The cause is in the motor, `crates/graph-core/src/layout/lanes/assign.rs:236-249` `carry`.
- A vertex's first forward edge always carries the vertex's own lane down to its target.
- It does so even when a smaller lane is already reserved for that target.
- So every branch forked from one old base holds its own lane all the way down to that base.

The decision record is `docs/decisions/dag-lanes-merge.md`:
- the first ruling (rule C) is superseded;
- its **"Addendum: rule D"** is the ruling this job implements. Its conditions are this job's
  acceptance criteria, and they win over this brief wherever the two disagree. Read the addendum
  in full before editing.

Two of the addendum's conditions are already settled, and this job does not redo them:
- **Condition 7.** The plugin's switch to committer time landed apart, in 9327c58a. This job
  touches nothing under `examples/`, and the row `no-plugin` checks it. The `grep -c '%ct'` check
  in condition 7 described the order before that landing.
- **`capabilities --check` (condition 5).** On a fresh worktree it is red, because no 1000-seed
  hashgate or roundtrip record exists for the tree. Run it, report its exit code and the rows it
  names, and say whether any of them is not about seed counts. It is the develop full gate's row,
  not this job's.

**Rule D.** For each forward edge of `v`, in edge order:
- Let `S` be the target's smallest reserved lane, if it has one.
- The edge **shares `S`** when `S` exists and either:
  - `S < lane(v)`, or
  - `lane(v)` is already carried by an earlier edge of `v`.
- Otherwise:
  - the first such edge carries `lane(v)`;
  - a later one takes a free lane;
  - either way, the lane is pushed into the target's reservation.
- After all of `v`'s forward edges, `lane(v)` goes back to the pool if no edge took it. This
  happens **after** the edges, never before: the addendum's conditions explain why.

The geometry does not change. `geometry.rs:84-91` already bends an edge into its carrying lane
half a row after its earlier end, whenever that lane is not the end's own lane.

Facts (develop, 2026-10-06; re-check each on your branch before editing, and stop if one no
longer holds):
- `crates/graph-core/src/layout/lanes/assign.rs`:
  - `:1-13` the module doc, which states today's rule;
  - `:56-59` `Pool::give`;
  - `:122-141` `Reserved::settle`;
  - `:148-158` `assert_sole`, a debug-only check of the one-reservation invariant;
  - `:233-249` `State::carry(first, lane, late)`;
  - `:281-293` `Drawing::place`, which calls `carry(k == 0, ...)` and gives the lane back only when
    `out.is_empty()`.
- `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs`, the hand oracle. It restates the
  rule with `BTreeSet`s:
  - `:7-9` the convention in its module doc;
  - `:113-131` `assign`, which calls `state.carry(k == 0, own, late)` at `:125` and gives the
    lane back at `:127-129`;
  - `:194-212` `carry`.
- `crates/graph-core/src/layout/lanes/tests.rs`:
  - `:74-94` `a_branch_and_its_merge_take_two_lanes`;
  - `:97-110` `a_directed_cycle_is_broken_at_the_lowest_index_and_noted`.
- `crates/graph-core/src/layout/lanes/tests/history.rs:99-120`: the fan-in-50 test. Its doc
  comment says "each source carries its own lane down to `sink`", which rule D changes.
- `crates/graph-core/src/registry/lanes.rs`:
  - `:21-28` `oracle`;
  - `:33-41` `degradation`;
  - `:42-54` `ponytail`, including the `Ponytail (lane choice)` clause.
- `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md:60-74`: the spec's "Lanes"
  algorithm step. The hand oracle's doc names it as the convention.
- `server/graph-server/tests/digest/manifest.json:52` and `:111` hold the two `layout.dag.lanes`
  digest pins.
  - Only the ignored test `emit_the_manifest` rewrites a pin, with `GM_SVC_DIGEST_EMIT=1`
    (`server/graph-server/tests/digest/manifest.rs:9-11`).
  - Never hand-edit a hash.
  - The command is: `scripts/orch/gr -e GM_SVC_DIGEST_EMIT=1 cargo test --manifest-path
    server/Cargo.toml -p graph-server --test digest -- --ignored emit_the_manifest`.
  - Afterwards `git diff server/` must show exactly those two lines changed.
- The "Commit DAGs" section of `docs/measurements/dag-lanes.md`:
  - The table has columns `lanes width (author time)` and `lanes width (committer time)`, both
    for today's rule.
  - The plugin feeds committer time since 2026-10-06 (`examples/plugins/git/git-log.sh`).
  - The bare clones are at `/tmp/gitviz/{contributor-stats,activitywatch,aw-server-rust,git}.git`.
- `scripts/orch/rows/lanes-merge.rows` is already on develop. Do not edit it.
  - Its `pins-kept` row checks condition 4.
  - Its `named-test` and `named-test-d` rows check conditions 3 and 6.

Steps:
1. **RED first.** Add the two tests below, with the exact values from the addendum's conditions
   3 and 4, to `tests.rs`. Use `tests/history.rs` instead if `tests.rs` would pass 300 lines.
   - `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge`
     (condition 3);
   - `three_lines_forked_from_one_base_share_the_column_waiting_for_it` (condition 4).

   Run `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-core lanes`.
   Both new tests must fail against today's code; record the values they print.
2. **The motor.**
   - Rule D in `State::carry` and `Drawing::place`.
   - Keep `assert_sole`.
   - Add the give-side check the addendum asks for.
   - Rewrite the module doc `:1-13` to state rule D.
   - House limits: ≤ 40 lines per function, ≤ 4 parameters, ≤ 300 lines per file, nesting ≤ 3,
     no `unsafe`, no new dependency.
3. **The hand oracle.**
   - The same rule in `hand_oracles/lanes.rs` `carry` and `assign`, with its own `BTreeSet`
     code. Do not share code with the layout.
   - Update its module doc `:7-9`.
4. **Tests.**
   - Do not change the assertions of `a_branch_and_its_merge_take_two_lanes` or
     `a_directed_cycle_is_broken_at_the_lowest_index_and_noted`. They keep their landed values
     under rule D (condition 4). If either goes red, the build implemented rule C.
   - Update the fan-in-50 doc comment.
   - Run `cargo test -p graph-core lanes`: all green.
   - **The guard's control (condition 3).** Temporarily drop the `S < lane(v)` clause, so the
     edge shares whenever `S` exists. Run the condition-3 test: it must fail, drawing
     `x == [1.0, 0.0, 1.0, 0.0, 2.0]`. Restore the clause, re-run, and record both runs.
   - **The give-back control (condition 1).** Temporarily give `lane(v)` back before the edge
     loop. A debug-build `cargo test -p graph-core lanes` must panic in `assert_sole` or in the
     give-side check. Restore it, and record both runs.
5. **Registry and spec.**
   - `registry/lanes.rs`:
     - `oracle` names the pinning test and `nothing_sits_on_an_edge`
       (`hand_oracles/lanes/compare.rs`);
     - fix `degradation` and `Ponytail (lane choice)` where they state the old rule.
   - The spec's "Lanes" step: state rule D, with one dated line pointing to
     `docs/decisions/dag-lanes-merge.md`.
6. **Digests.** Re-emit the two pins with the command in Facts, then run the whole svc suite
   (the svc rows).
7. **Re-measure the width.**
   - Build the wasm: `CARGO_BUILD_JOBS=3 scripts/orch/gr cargo build -p graph-wasm --release
     --target wasm32-unknown-unknown`.
   - Under `flock ~/goinfre/orch/bench.lock`, run `examples/plugins/git/bench.mjs` once per
     history on the committer-time logs. Write each log with `examples/plugins/git/git-log.sh
     <repo> > target/git-lanes/<name>.log`. Run the 1M synthetic too, at `DRUN_MEM=12g`, after
     checking `free -g` shows ≥ 12 GB available.
   - In "Commit DAGs", add a column `lanes width (rule D)` beside the two existing width columns,
     and keep their numbers. Add the commands. Add one sentence saying the motor changed and the
     plugin did not.
   - Re-time `lanes ms` only if the addendum asks for it.
   - The addendum's model predicts committer-time widths of 4 (activitywatch), 4
     (aw-server-rust), 38 (graph_render) and 197 (git/git). A measured width that differs from
     its prediction by more than 1 is a finding: report it with both numbers, and do not tune the
     rule to match.

Rules (beyond `scripts/orch/common.md`):
- Cargo only through `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr`. Node only through
  `scripts/orch/node-slim.sh`.
- Never take `~/goinfre/orch/timed.lock`. Benches run only under
  `flock ~/goinfre/orch/bench.lock`, one at a time.
- Nothing under `examples/` may change: the plugin's switch to committer time landed apart
  (condition 6 of the first ruling).
- Paths you may touch:
  - `crates/graph-core/src/layout/lanes/**`;
  - `crates/graph-core/src/registry/lanes.rs`;
  - `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs` and `.../hand_oracles/lanes/**`;
  - `crates/graph-cli/src/snapshot_cmd/hand_oracles/tests/lanes.rs`;
  - `server/graph-server/tests/digest/manifest.json`, the two lanes lines, re-emitted only;
  - `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`, the "Lanes" step only;
  - `docs/measurements/dag-lanes.md`, the "Commit DAGs" section only.

Done when:
- `scripts/orch/gate.sh target/rows-lanes-merge scripts/orch/rows/lanes-merge.rows` writes a
  `summary.txt` with every row PASS;
- every condition of the addendum is met. Name each condition with its evidence: a command and
  its exit code, or a test name.

Return:
- the branch tip;
- the files changed, with line counts;
- the two new tests' failing values from step 1;
- the guard and give-back controls from step 4, each run with its result;
- the new width column;
- each row's result;
- each addendum condition with its evidence;
- every deviation.
