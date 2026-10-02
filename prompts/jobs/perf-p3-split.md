# Job perf-p3-split (agent build): `Threads` writes into the caller's column

Why: `prompts/perf-plan.md` P3, first rung. `Threads::run` (`crates/graph-cli/src/exec_native.rs:36-71`)
spawns OS threads per call, gives each worker its own `Vec` (`:57`), then `parts.concat()` and
`out.extend_from_slice` (`:70`): two extra allocations and two full copies of the column per step.
At 1 000 000 nodes and 112 ticks that is 224 copies of an 8 MB column per pass. Remove them without
`unsafe` and without moving a byte.

Facts:

- `partition(n, workers)` (`crates/graph-core/src/exec/partition.rs`) returns ascending, contiguous
  ranges that cover `0..n` exactly. That contract is what makes a `split_at_mut` chain legal.
- A kernel's `step_range(range, out)` writes `out[..range.len()]` at range-relative indices
  (the comment at `exec_native.rs:40-41`).
- The tests that must stay green: `crates/graph-cli/src/exec_native/tests.rs` (every worker count
  gives the serial bytes, zero workers, more workers than outputs, overwrite-not-append).
- graph-core is not touched. Only `crates/graph-cli/src/exec_native.rs`, its `tests.rs`, and the
  two docs below.

Do:

1. `out.clear(); out.resize(kernel.len() as usize, O::Out::default());` once, then inside
   `std::thread::scope` walk the ranges, peel each span off the remaining slice with
   `split_at_mut(range.len())`, and spawn `kernel.step_range(range, span)`. No per-worker `Vec`,
   no `concat`, no `extend_from_slice`. The scope's join stays the one barrier per step.
2. Rewrite the comment block at `:46-51` to say why the disjoint borrow is legal (the partition
   contract) in at most four lines. Keep the module header's claims true: it says "Not yet
   measured"; replace that paragraph with the numbers from step 4, or keep it if you could not run.
3. Add one test: a kernel of 1 000 003 outputs at 7 workers equals the serial column (catches an
   off-by-one in the span chain at a size the existing tests do not reach).
4. Measure, before and after (stash nothing: build the old file as it is on `origin/develop` in a
   scratch copy if needed, or report the before number from `docs/measurements/phase11-threads.md`):
   `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.barnes_hut --n 100000 --tiers scalar,threads --workers 2,4,7`
   and the same at `--n 1000000 --past-ceiling` once (it is slow; cap it with `timeout 3600`).
   Record host load (`cat /proc/loadavg`) next to each run: the host is shared.
5. Spawn overhead: time an empty kernel (`step_range` does nothing) over 112 steps at 7 workers
   in a unit test or a bench, and state it as a share of one 100 000-node tick. Write the share.
   If it is 3% or more, say so under "decisions needed" (a persistent pool is the next rung);
   do not build the pool.
6. Write `docs/measurements/perf-p3-split.md`: problem, change, a before/after table with load,
   the spawn share, how to reproduce, what it does not do. House style: `docs/measurements/perf-p2.md`.

Done when:

- `scripts/orch/gr cargo test -p graph-cli exec_native` passes, with the new test.
- `scripts/orch/gr cargo clippy -p graph-cli --all-targets -- -D warnings` and `cargo fmt --all --check` are clean.
- `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all` exits 0 (the
  Threads arms at 2, 4, 7 still hash equal to serial).
- The measurement doc exists with numbers you ran, each with its command. A number you did not
  run is written "not run", never estimated.
