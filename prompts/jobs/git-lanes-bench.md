# Job git-lanes-bench (agent build: `layout.dag.lanes` measured on real commit histories)

Why: `layout.dag.lanes` landed on 2026-10-06. Its spec's "Measurement" section
(`docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`) asks for real commit DAGs and a
1M synthetic history. For each input it wants:
- the time;
- the lane width;
- the width the reference tool draws (`git log --graph`).

The targets are:
- git/git ≤ 50 ms in wasm32;
- a 1M synthetic history ≤ 1 s in wasm32;
- zero note 4.

The motor may not know what its inputs are (user rule, 2026-10-05). So the history-shaped work
lives in the git plugin, `examples/plugins/git/`, and the numbers go in
`docs/measurements/dag-lanes.md`, which says today that they are "measured elsewhere".

Facts (develop, 2026-10-06; re-check each on your branch before editing, and stop if one no
longer holds):
- `examples/plugins/git/bench.mjs` (79 lines) takes `<wasm> <log>...`. Per log it times, over 3
  rounds with medians:
  - `parseLog`, then `toRows` + `rowsToIngest`, then `JSON.stringify`;
  - `motor.buildContract`;
  - each layout of `["layout.dag.sugiyama", "layout.dag.lanes"]` that `motor.layouts()` lists.

  It prints each layout's note counts from `motor.toJSON(run.handle)` (`:68-77`), and it records a
  refusal instead of a time (`:61-66`).
- `examples/plugins/git/map.mjs`:
  - `FORMAT = "%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s"` (`:4`);
  - `parseLog` (`:28-40`) refuses a hash that is not 40 or 64 hex digits, a time that is not a
    u32, and a commit seen twice.
- `examples/plugins/git/git-log.sh` is the plugin's one git call: `git -C <repo> log --all
  --topo-order --format=FORMAT`.
- `layout.dag.lanes` draws Point nodes: `x = lane * lane_spacing` and `y = row * row_spacing`.
  - Both spacings are 1.0 by default (`crates/graph-core/src/layout/lanes.rs`;
    `crates/graph-core/src/registry/tunable.rs`).
  - Note 5 marks an edge drawn head to tail. Note 4 must never appear.
- The wasm artifact: `CARGO_BUILD_JOBS=3 scripts/orch/gr cargo build -p graph-wasm --release
  --target wasm32-unknown-unknown` writes
  `target/wasm32-unknown-unknown/release/graph_wasm.wasm`.
- Bare clones: `/tmp/gitviz/{contributor-stats,activitywatch,aw-server-rust,git}.git` (488,
  1271, 989 and 85,928 commits). This worktree is itself a checkout of graph_render: `git -C .
  log --all` reads its whole shared history. If `/tmp/gitviz` is missing (a reboot), say so and
  measure this worktree only.
- `scripts/orch/node-slim.sh` runs under `scripts/orch/drun`. `DRUN_MEM` defaults to 4g; pass
  `DRUN_MEM=12g` for the 1M input. Precondition: `free -g` shows ≥ 12 GB available.

Steps:
1. **Width of the reference drawing.** Create `examples/plugins/git/graph-width.mjs`, exporting
   `graphWidth(text)`. Its input is `git log --graph --format=%H` output. Its result is the
   maximum over lines of `(index of "*") / 2 + 1`; a line with no `*` is skipped. Add a CLI
   tail: `node graph-width.mjs <file>` prints the number. Add
   `examples/plugins/git/test/graph-width.test.mjs`, in `map.test.mjs`'s style, with:
   - a three-line fixture string giving 1;
   - a branch-and-merge fixture giving 2;
   - a string with no `*` giving 0.
2. **Lane width in the bench.** In `bench.mjs` `layoutOnce`, for `layout.dag.lanes` only, also
   print `layout.dag.lanes.width=<k>`, where `k` is the number of distinct node `x` values in
   `motor.toJSON(run.handle)`. Read the JSON face's actual shape first and say which member you
   read. Keep the file ≤ 120 lines.
3. **Synthetic history.** Create `examples/plugins/git/synthetic.mjs <n> <seed> <out>`. It writes a
   log in `FORMAT`, children first, that `parseLog` accepts. The model is exact; use no other:
   - mulberry32 seeded with `<seed>`;
   - commit `i`'s hash is `i` in hex, zero-padded to 40 digits;
   - `%at` = 1,600,000,000 + i;
   - author `a<i % 50>`; empty refs; subject `c<i>`.
   - `live` is an array of branch tips. It starts as `[0]`; commit 0 is the root.
   - For each `i` from 1 to n − 1, draw `r`:
     - `r < 0.05` and `live.length < 64`: **branch**. The parent is the tip of a uniformly drawn
       live branch; push `i` as a new tip.
     - `r < 0.10` and `live.length > 1`: **merge**. Draw two distinct live indices `a < b`. The
       parents are `[live[a], live[b]]`. `live[a] = i`; remove `live[b]`.
     - otherwise: **commit**. With probability 0.5 on `live[0]`, else on a uniformly drawn live
       branch `j`. The parent is `live[j]`; `live[j] = i`.

   Add a test: `synthetic.mjs`'s output for n = 1000, seed 1 parses with `parseLog`, holds 1000
   commits, and has at least one two-parent commit. Header: what it models, and a `Caveat:` naming
   what a real history has that it does not (octopus merges, cherry-picks, clock skew).
4. **Measure.** Build the wasm (Facts). Then, under `flock ~/goinfre/orch/bench.lock`, with the
   load printed before each run:
   - `bench.mjs` on the five histories: contributor-stats, activitywatch, aw-server-rust,
     graph_render (this worktree) and git/git. Each log comes from `git-log.sh` into
     `target/git-lanes/<name>.log`.
   - `graphWidth` on `git -C <repo> log --all --topo-order --graph --format=%H` for each of them.
   - `synthetic.mjs` at n = 1,000,000, seed 1, then `bench.mjs` on it, with `DRUN_MEM=12g`.
     - If the 1M run is killed or refused (exit 137, a wasm memory error, a refusal), halve n
       until it completes.
     - Report the largest n that completed, with its number, and what stopped the larger one.
       A missed target is reported with its number, not rounded into a pass.
   - Run the five histories and the synthetic once each per round, 3 rounds, alternating inputs
     round by round. Claim medians only, and say the host was loaded.
5. **Report.** In `docs/measurements/dag-lanes.md`, replace the section "Real-history inputs are
   measured elsewhere" with "## Commit DAGs". It holds:
   - the commands;
   - one table: input, n, m, build ms, lanes ms (wasm32), sugiyama ms (wasm32), lanes width,
     `git log --graph` width, note-5 count, note-4 count;
   - each spec target with its measured number and PASS or MISSED;
   - a `Caveat:` line.

   The native column the spec asks for is not measured. `graph-cli` has no file-input layout
   bench, and adding one is a crate change outside this job. Say so in the section, and put it
   under "decisions needed".

Rules (beyond `scripts/orch/common.md`):
- Node only through `scripts/orch/node-slim.sh`; cargo only through
  `CARGO_BUILD_JOBS=3 scripts/orch/gr`.
- Never take `~/goinfre/orch/timed.lock`. Benches run only under
  `flock ~/goinfre/orch/bench.lock`. Run one bench at a time.
- Nothing under `crates/`, `packages/`, `app/`, `src/`, `server/`, `harness/` or `fixtures/` may
  change. A refusal from the motor or the SDK is a finding: write it under "decisions needed" with
  the exact message.
- Paths you may touch:
  - `examples/plugins/git/**`;
  - `docs/measurements/dag-lanes.md`, only the section step 5 names.

  `scripts/orch/rows/git-lanes-bench.rows` is already on develop: do not edit it.

Done when:
- `scripts/orch/gate.sh target/rows-git-lanes-bench scripts/orch/rows/git-lanes-bench.rows` writes a
  `summary.txt` with every row PASS;
- the "Commit DAGs" section holds the table and the targets.

Return:
- the branch tip;
- the files changed, with line counts;
- the table;
- each target: number and PASS or MISSED;
- each row's result;
- decisions needed;
- every deviation.
