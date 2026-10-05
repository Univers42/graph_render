# Job git-plugin-ct (agent build: the git plugin feeds committer time, and the lane widths are re-measured)

Why: `docs/decisions/dag-lanes-merge.md` (2026-10-06) names two causes for `layout.dag.lanes`
drawing real histories wider than `git log --graph`. Its condition 6 says the two must land
apart, so the table can tell them apart. This job is the second cause, alone.

**Cause 2.** The plugin feeds **author** time (`%at`) as each record's `version`.
- A rebased or cherry-picked commit keeps its old author time.
- So ordering rows by version interleaves branches.
- Committer time (`%ct`) is the time the commit was written. git's own `--date-order` sorts by it.

The motor is not touched. Nothing under `crates/` changes.

The orchestrator's model of today's motor gives these widths:

| input | `%at` | `%ct` |
|---|---:|---:|
| aw-server-rust | 14 | 8 |
| git/git | 366 | 282 |
| graph_render | 40 | 40 |
| activitywatch | 6 | 6 |

Facts (develop, 2026-10-06; re-check each on your branch before editing, and stop if one no
longer holds):
- `examples/plugins/git/git-log.sh:5` is the plugin's one git call. Its format string is
  `'%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s'`.
- `examples/plugins/git/map.mjs:4`: `export const FORMAT = "%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s";`.
  - The fourth field is parsed as `time` (`:46`, `:51-52`).
  - It becomes the record's `updatedAt` (`:93`), which the SDK's rows adapter maps onto `version`.
- `examples/plugins/git/synthetic.mjs:7` and `:13` name the field `%at` in the header comment.
- `examples/plugins/git/README.md` documents the plugin. It does not mention time today.
  `grep -n 'time' examples/plugins/git/README.md` prints nothing.
- `docs/measurements/dag-lanes.md`, section `## Commit DAGs`:
  - its commands are at `:150-172`;
  - its results table is at `:187-194`, with columns `input | n | m | build ms | lanes ms |
    sugiyama ms | lanes width | git log --graph width | note 5 | note 4`;
  - its `Caveat:` is at `:215-220`, and it mentions `%at` (`:219`).
- Bare clones: `/tmp/gitviz/{contributor-stats,activitywatch,aw-server-rust,git}.git`. This
  worktree is itself a checkout of graph_render. If `/tmp/gitviz` is missing, say so and measure
  this worktree only.
- `scripts/orch/rows/git-plugin-ct.rows` is already on develop. Do not edit it.

Steps:
1. **Committer time.**
   - In `git-log.sh:5` and `map.mjs:4`, change `%at` to `%ct`. Nothing else in either format.
   - Name the field committer time where the plugin names it:
     - `synthetic.mjs`'s header (`:7`, `:13`);
     - `map.mjs`'s doc comment on `parseLog` if it names the field;
     - one line in `README.md` saying why committer time and not author time.
   - Add one test to `examples/plugins/git/test/map.test.mjs`, in its style: `FORMAT` holds
     `%ct` and not `%at`.
2. **Re-measure the width only.** Times do not change with the format, so do not re-time.
   - Build the wasm: `CARGO_BUILD_JOBS=3 scripts/orch/gr cargo build -p graph-wasm --release
     --target wasm32-unknown-unknown`.
   - For each of the five histories, under `flock ~/goinfre/orch/bench.lock`, one round:
     - write the log with the new `git-log.sh` to `target/git-lanes/<name>.log`;
     - run `bench.mjs` on it and read `layout.dag.lanes.width=`.
   - The reference width with git's own date order: `git -C <repo> log --all --date-order
     --graph --format=%H > target/git-lanes/<name>.date.graph`, then `graph-width.mjs` on it.
3. **Report.** In `docs/measurements/dag-lanes.md`, section `## Commit DAGs` only:
   - Rename the column `lanes width` to `lanes width (author time)`. Keep its numbers: they are
     the record of the first run.
   - Add the columns `lanes width (committer time)` and `git log --graph --date-order width`,
     with the numbers from step 2. The synthetic's cells are `n/a` in both.
   - Add the step-2 commands to the commands block.
   - Make the `Caveat:` name committer time. Say that the two width columns differ only in the
     plugin's format: same motor, same tree.

Rules (beyond `scripts/orch/common.md`):
- Node only through `scripts/orch/node-slim.sh`. Cargo only through
  `CARGO_BUILD_JOBS=3 scripts/orch/gr`.
- Never take `~/goinfre/orch/timed.lock`. Benches run only under
  `flock ~/goinfre/orch/bench.lock`, one at a time.
- Nothing under `crates/`, `packages/`, `app/`, `src/`, `server/`, `harness/` or `fixtures/` may
  change.
- Paths you may touch:
  - `examples/plugins/git/**`;
  - `docs/measurements/dag-lanes.md`, only its `## Commit DAGs` section.

Done when:
- `scripts/orch/gate.sh target/rows-git-plugin-ct scripts/orch/rows/git-plugin-ct.rows` writes a
  `summary.txt` with every row PASS;
- the table carries both width columns and the date-order reference.

Return:
- the branch tip;
- the files changed, with line counts;
- the new table rows;
- each row's result;
- every deviation.
