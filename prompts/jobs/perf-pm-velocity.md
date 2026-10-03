# Job perf-pm-velocity (agent build): fuse the mesh tick's link and charge velocity merges, same bytes

Why: at 1M nodes, 8 workers, the particle-mesh tick spends 11.7 ms in `motion::Velocity`, six runner
calls per tick (`docs/measurements/perf-pm-stencil.md:58-67`). The tick is memory-bound, so a pass
that re-reads and re-writes `vx`/`vy` only to add the next delta is pure traffic. Fuse what can be
fused without moving one output byte, measure it, and keep it only if it pays.

Facts (verified on develop e5e2ddca):

- **The tick.** `crates/graph-core/src/layout/force/particle_mesh.rs:107-138` `tick`:
  1. `:115` `link::pass_with(sim, runner, workers, how.deltas)` writes link's `(dvx, dvy)` in
     **node order** into `how.deltas`;
  2. `:121` `motion::merge(sim, linked, ..)` adds them to `vx`, `vy` (one `Velocity` run per axis);
  3. `:122` `charge::apply(sim, mesh, how)` (`particle_mesh/charge.rs:37-53`): `mesh.solve` then the
     `Interpolate` kernel writes the field in **slot order** into the same `how.deltas`, then
     `motion::merge` again with `slot: Some(&mesh.grid.slot)` (two more `Velocity` runs). It returns
     early, merging nothing, when `mesh.solve` answers `false`;
  4. `:123` `motion::center`, `:124` `collide::apply` (its projection reads `x + v`, so it needs both
     merges done), `:128-130` `gravity::apply` when `gravity > 0` (default `0.0`,
     `session/live_params.rs:183`), `:136` `motion::integrate` (two more `Velocity` runs, with the
     collide merge and the decay, then two `Position` runs).
- **The merge expression.** `particle_mesh/motion.rs:50-66` `Velocity::step_range`, per node `i`:
  `v += axis(deltas[k]) + stolen` where `k = slot.map_or(i, |s| s[i])` and `stolen` is the next
  slot's delta under the negative control (`Gathered.split`), else `0.0`. Note the grouping:
  `v + (delta + stolen)`. A fused merge must keep each merge's own grouping and their order:
  `v1 = v + (link + stolen_l)`, then `v2 = v1 + (charge + stolen_c)`.
- **Buffers.** `How.deltas` (`barnes_hut/sim.rs:27-38`) is one `Vec<(f64, f64)>` of `n`, shared by
  every pass. Fusing link and charge needs both delta columns alive at once: one more `n`-long
  `(f64, f64)` column (16 MB at 1M). Put it in `Mesh` (`particle_mesh/mesh.rs:36-50`), sized once and
  reused across ticks, like `at`.
- **Runner contract.** `exec/partition.rs:41-89`: a `StepRange` kernel writes one output column.
  Do not add a `Runner` method: the runners in `graph-cli/src/exec_native.rs:41` and
  `graph-wasm/src/pool.rs:213` would change, which is a concurrency change outside this job.
- **The negative controls** `Split::Link` and `Split::Charge` (`how.split.splits(..)`) must still
  turn their gates red: each fused merge keeps its own `split` flag.
- **Prior art for the gates and the bench.** `docs/measurements/perf-pm-stencil.md` and its scripts
  `~/goinfre/bench/pm-stencil/{passes.sh,xtree.sh,parity.sh}`. The base serial wasm for cross-tree
  parity is `~/goinfre/bench/pm-stencil/serial.wasm` (the stencil tree, which is develop's PM).
- Files near the cap: `particle_mesh/motion.rs` 196 lines, `charge.rs` 53, `mesh.rs` 154,
  `particle_mesh.rs` 138. House limits: 40 lines a function, 4 parameters, 300 lines a file,
  nesting 3. No `unsafe`, no new dependency.

Do, in order:

1. Pin first. Run `xtree.sh` (copy it into the job's own `~/goinfre/bench/pm-velocity/`, pointing at
   this worktree) on the untouched branch and keep its output as `xtree-base.out`.
2. Fuse the link and charge merges into one `Velocity` run per axis:
   - the link pass writes into the new `Mesh` column (or `how.deltas`) and the `Interpolate` kernel
     into the other; the fused kernel reads `link[i]` then `charge[slot[i]]`, in that order, with the
     groupings above;
   - when `mesh.solve` answers `false`, the link merge alone still runs, as today;
   - `mesh.solve` must not read `vx`/`vy` (check it; if it does, stop and report).
   That is 6 → 4 `Velocity` runs per tick.
3. Optional, only if it is byte-identical: fold `gravity::apply` (a one-thread loop) into the
   integrate's `Velocity` run as `v + (0 - x) * gravity * alpha` before the collide merge, keeping
   the `gravity > 0` skip. The tests in `session/gravity.rs` and `barnes_hut/sim/tests.rs` pin the
   signed-zero case; they must still pass. Skip this step if any byte moves.
4. Unit test in `particle_mesh/motion/tests.rs` (or a sibling file if it would pass 300 lines): the
   fused merge equals two sequential merges bit for bit on a 1000-node column with a non-identity
   `slot`, with and without each `split`.
5. Gates (exit codes in the report):
   - `scripts/orch/gr cargo fmt --all --check` → 0
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` → 0
   - `scripts/orch/gr cargo test --workspace --no-fail-fast` → 0
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → 0;
     `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` → 1
   - `xtree.sh` on the branch: every line equal to `xtree-base.out` (`diff` → 0)
   - `parity.sh`-style: build this tree's threads wasm (`scripts/orch/wasm-threads.sh`), then
     `harness/wasm-threads.mjs hash --serial <~/goinfre/bench/pm-stencil/serial.wasm>` → rc 0, every
     row `equal`; and `--break` → rc 1.
6. Bench, only when `free -g` shows ≥ 12 GB available and the 1-minute load is < 14 (wait otherwise):
   `tick --layout particle-mesh --n 1000000 --ticks 7 --workers 8 --passes`, base (develop at the
   branch point, in a second worktree or a `git stash`-free copy) and branch alternated, 3 rounds,
   `GR_MEM=12g`, load printed per run. Medians only.
7. Keep rule: keep the change only if the 8-worker `Velocity` row drops by ≥ 2 ms median **and** the
   tick median does not rise. Otherwise revert the code, keep the test and the report, and say
   "not kept" with the numbers.
8. Report `docs/measurements/perf-pm-velocity.md`, shaped like `perf-pm-stencil.md`: the change, the
   gate table, the per-pass table (base vs branch, 8 workers), memory added, a `Caveat:` on load, and
   "what it does not do".

Paths you may edit: `crates/graph-core/src/layout/force/particle_mesh.rs`,
`crates/graph-core/src/layout/force/particle_mesh/{motion.rs,motion/,charge.rs,mesh.rs}`,
`crates/graph-core/src/layout/force/session/gravity.rs` (step 3 only),
`docs/measurements/perf-pm-velocity.md`, anything under `~/goinfre/bench/pm-velocity/`.
Nothing in `exec/`, `barnes_hut/` (except reading), `graph-wasm`, `graph-cli`, `packages/`, `app/`.

Done when: every step-5 gate has its expected exit code, the bench table has three rounds per arm,
and the report says kept or not kept against the step-7 rule.

Return block:

```
status: done | partial | blocked
kept: yes | no (Velocity 8w base → branch ms, tick base → branch ms, medians)
changed: <files>
commands: <each gate> -> <rc>
deviations: <none | list>
decisions needed: <none | list>
```
