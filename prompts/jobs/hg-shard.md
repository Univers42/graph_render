# Job hg-shard (agent build, branch hg-shard, worktree ~/goinfre/wt/hg-shard)

Context, measured on 2026-10-04 (develop 0c1f32eb, release build, host dlesieur42, 20 cores, load ~2.4):
- `hashgate --seeds 1000 --tiers all` exited 2 after 2710 s:
  `"/w/target/release/graph-cli" "hashgate-arm" "--seeds" "1000": killed after 2700000ms`.
- The 2700 s limit is `CHILD_TIMEOUT` (`crates/graph-cli/src/runner.rs:33`), and it applies to each child.
- One native arm now runs 67 stages per seed. Seed `s` has `2 + s % 600` nodes (`graph_core::gate_node_count`).
- `bench --n 300,600 --repeat 1 --past-ceiling` over every layout measured the layouts alone:
  - 2.8 s per seed at n=300, and 10.2 s at n=600;
  - `layout.force.davidson_harel` takes 1.04 s at n=300 and 4.40 s at n=600.
- So one arm over 1000 seeds is roughly 3000-4000 s, all on ONE core.
- The `--tiers all` arms (`hashgate/tier.rs:83`) are worse: 6 more full recomputes run in-process.

The fix is NOT a larger timeout, and NOT fewer seeds or smaller graphs. Each arm's seeds are
independent, so split every arm's seeds across concurrent shards and merge the lines back into the
exact order the comparator already validates. `compare::diverged` (`hashgate/compare.rs`) refuses
any line that is not stage `i / seeds`, seed `i % seeds`, so a bad merge turns the gate red. It
cannot pass.

## Exact tasks

1. New module `crates/graph-cli/src/hashgate/shard.rs` (≤300 lines; each function ≤40 lines and
   ≤4 params):
   - `pub struct Shard { pub index: u32, pub count: u32 }`.
     - `Shard::WHOLE` = `{0, 1}`.
     - `Shard::parse("i/K")` refuses `K == 0`, `i >= K` and anything malformed, with a message naming the input.
     - `fn seeds(self, seeds: u32) -> impl Iterator<Item = u32>` = `(index..seeds).step_by(count)`.
       It is strided, not contiguous, because a seed's cost grows with `2 + seed % 600`.
   - `pub fn per_arm() -> u32`: `std::thread::available_parallelism()` clamped to `1..=8`. Give
     it a `Caveat:` doc line: it sizes wall clock and is not a correctness parameter, and the bytes
     are the same at any count.
   - `pub fn merge(seeds: u32, stages: &[&str], shards: &[Vec<String>]) -> Result<Vec<String>, String>`:
     - Place each `stage seed sha256` line in slot `stage_index * seeds + seed`.
     - Refuse an unknown stage, a seed `>= seeds`, a slot filled twice, and any slot left empty.
     - Every refusal names the line or slot.
     - Return the lines in stage-major, seed-minor order.
   - Run the shards concurrently with `std::thread::scope`. Collect their results in shard order,
     never in completion order.
2. Native child arm: `hashgate-arm` gains an optional `--shard i/K` (default `0/1`).
   - Parser: `crates/graph-cli/src/command.rs:49` and `main.rs:45`. Add a parser test next to the
     existing ones in `command/tests.rs:47-50`.
   - `arm_lines(seeds, setting)` (`hashgate.rs:184`) iterates only `shard.seeds(seeds)` and keeps
     its zero-seed refusal.
   - `threads_lines` (`hashgate.rs:211`) does the same.
3. `collect_arms` (`hashgate.rs:223`): `native()` and `wasm32()` each spawn `shard::per_arm()`
   children concurrently.
   - Each child goes through the existing `run_lines`, so each keeps its own `CHILD_TIMEOUT`.
   - Merge the results with `shard::merge` into ONE arm. There are still 4 arms named exactly as
     today: `native run 1`, `native run 2`, `wasm32 run 1`, `wasm32 run 2`.
   - The arms themselves stay sequential; only an arm's shards run in parallel.
4. In-process tier arms (`hashgate/tier.rs`): `scalar_arm` and `threads_arm` run their shards with
   `std::thread::scope` over `arm_lines` / `threads_lines` and merge them the same way.
5. The wasm harness:
   - `harness/wasm-run.mjs` `hash` mode accepts `hash <seeds> [--shard i/K] <stage>...`, with the
     same refusals as step 1. Update `USAGE`.
   - `harness/wasm-run/hash.mjs` `runHash` iterates only that shard's seeds.
   - The C20 check (`c20`, same file) compares the two stages seed by seed over the seeds this
     shard ran, and reports the real seed number on a divergence. Today it indexes `0..seeds`,
     which is wrong for a shard: fix that rather than skipping C20 under a shard.
6. Tests. Use cargo's built-in harness, in the existing test files next to each module.
   - Merge equivalence (the one that matters): over 7 seeds, the merged output of 3 shards equals
     `arm_lines(7, Shard::WHOLE, ..)` byte for byte. Do the same for `threads_lines` at 2 workers.
   - `merge` refuses a missing line, a duplicate line, an unknown stage and an out-of-range seed.
   - `Shard::parse` refusals.
   - Observe the equivalence test RED once: make `merge` sort by seed only, then restore it. Paste
     the failure in your report.
7. Update the doc of `CHILD_TIMEOUT` (`runner.rs:12-33`): it bounds one shard of one arm, and the
   shards are why 1000 seeds fit in it again. Keep its existing `Ponytail:` marker accurate.
8. Gate: `scripts/orch/job-check.sh` over `target/wf/hg-shard.rows`, all 15 rows PASS. That is
   `quick.rows`, plus `hashgate --seeds 8 --tiers all` and its negctl, plus
   `hashgate --seeds 60 --release`.
   - Report the wall time of `hashgate-60-release` from the summary.
   - Do NOT run `hashgate --seeds 1000`. It is a timed gate and runs under the host lock later, not in this job.

## Hard rules
- Determinism D1-D10 (`prompt.md` §6). No `HashMap`, only `BTreeMap`/`Vec`. No new dependency anywhere.
- Do not change what any stage hashes, the per-seed model, the stage list, the seed count or the knobs.
- Toolchain only through `scripts/orch/gr` and `scripts/orch/node-slim.sh`. No bare `cargo`, `npm`, `node` or `docker run`.
- Do not edit `.claude/rules/devil/`, `crates/graph-core/`, `packages/`, `app/` or `server/`.
- Commits: author `LESdylan <dev.pro.photo@gmail.com>`, message exactly `updated`, no trailers.
  Do not push and do not merge into develop: the lander does that.
- Report: files changed, the RED/GREEN evidence from step 6, the 15-row summary, and the wall time from step 8.
