# Perf PM velocity: fuse the tick's link and charge merges, same bytes (measured, not kept)

Status: measured on 2026-10-05 and **not kept** (see "Speed"). The code is at the tag
`archive/perf-pm-velocity`; develop carries only this report.

Measured 2026-10-05 on branch `perf-pm-velocity` (from develop bd98bf32). Host: dlesieur42,
i5-13600KF, 20 threads, 31 GB. Rust from `scripts/orch/gr`, wasm under Node in the same image.

## Why

`perf-pm-stencil.md` left the particle-mesh tick at 107 ms with `motion::Velocity` at 11.7 ms
and **six** runner calls per tick. Four of those six are the two merges — link's and charge's
— each a full read-modify-write of the 8 MB `vx` and 8 MB `vy` columns, 8 MB read and 8 MB
written per axis per merge, to add one delta. The tick is memory-bound, so a pass that
re-reads and re-writes a column only to add the next delta is traffic with no arithmetic in
it.

The two merges cannot be reordered for free, but they can be *fused*: the mesh field is a
function of the positions and the params alone, so the link deltas can wait while the field
is solved, and both merges can be one pass.

## Change

`Mesh::solve` (`particle_mesh/mesh.rs:123`) takes `&Sim` and reads `sim.params`, `sim.x` and
`sim.y` — no velocity, on any path (`frame::bounds`, `frame::place`, `deposit`, `Kernel::refresh`,
the two FFT passes). Verified before editing; the velocity enters the mesh pass only at the
merge, which is after the solve.

So the link pass writes into a new `Mesh.link` column instead of `how.deltas`, the charge
solve and field read run, and one `motion::Velocity` run per axis adds both columns:
`v1 = v + (link + stolen_l)`, then `v2 = v1 + (charge + stolen_c)` — each merge's own
grouping, in the order the tick did them, each with its own `split` flag. When `Mesh::solve`
answers `false` the link merge alone runs, as before.

| File (`crates/graph-core/src/layout/force/particle_mesh/`) | What changed |
|---|---|
| `motion.rs` | `Velocity.also`, the second gather of a fused pair; `added()`, the one merge's contribution, factored out of `step_range`; `merge_pair()`; `Merges`, so the run keeps four parameters |
| `charge.rs` | `apply` split into `read` (solve + field read) and `merge`; `apply` is the two, for `charge_pass`'s sake |
| `mesh.rs` | `link: Vec<(f64, f64)>`, sized in `new` and resized in `grow` |
| `particle_mesh.rs` | `merge_both`, and `tick` calls it where it used to merge and then call `charge::apply` |

`charge::apply` keeps its behaviour for its second caller, `ForceSession::charge_deltas`
(`session/fidelity.rs:69`); the fidelity tests pass unchanged. No `Runner` method was added:
the fused pass is a `StepRange` kernel like any other.

**Six `Velocity` calls a tick became four** (two merges → one, plus the integrate's two).

## Byte identity

| Check | Result |
|---|---|
| `cargo fmt --all --check` | rc 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | rc 0 |
| `cargo test --workspace --no-fail-fast` | rc 0 (1608 + 484 + 56 + 208 passed, 0 failed) |
| `cargo build -p graph-core --target wasm32-unknown-unknown` | rc 0 |
| `graph-cli snapshot --layout force.particle_mesh` at n = 333, 1000, 4096, 12345, 20000, 50000, 77777, 100000: sha256, this tree against its own base | identical at all 8 (`diff` rc 0) |
| `wasm-threads.mjs hash --serial <base-serial.wasm>` (this tree's threaded build against the base tree's serial build), BH and PM, workers {1,2,3,4,7}, seeds 0–7 | rc 0, 80 rows `equal`, 0 differing |
| `wasm-threads.mjs session --serial <base-serial.wasm>` | rc 0, 0 differing |
| `wasm-threads.mjs hash --n 1000,20000` | rc 0, 0 differing |
| `wasm-threads.mjs hash --break` (negative control) | rc 1, 64 differing, as expected |
| `graph-cli hashgate --seeds 8` | rc 0, PASS |
| `graph-cli hashgate --seeds 8` with `GM_MUTATE_REFERENCE_DEGREE=9` | rc 1, FAIL: 8 of 8 seeds diverge |
| `graph-cli hashgate --seeds 8 --tiers all` with `GM_MUTATE_SPLIT_SUM=link` (negative control) | rc 1 |
| … with `GM_MUTATE_SPLIT_SUM=charge` (negative control) | rc 1 |
| … with `GM_MUTATE_SPLIT_SUM=collide` (the untouched pass's own control) | rc 1 |
| … with `GM_MUTATE_SPLIT_SUM=collide --seeds 2` (the vacuity refusal) | rc 2, refused |
| `the_fused_merge_is_two_merges_bit_for_bit` (`motion/tests.rs`, 1000 nodes, non-identity `slot`, each `split` in turn, workers 1/2/3/7) | passes |

Raw: `$GM_SCRATCH/bench/pm-velocity/{xtree-base,xtree-branch,parity-hash,parity-break}.out`.

Both split controls still bite, which is the row that matters for a fused merge: each of the
two gathers keeps its own `split` flag, so `GM_MUTATE_SPLIT_SUM=link` corrupts only the link
gather and `=charge` only the charge gather.

## Speed

**Verdict: not kept.** The keep rule set before the bench was: `motion::Velocity` at 8 workers
drops by at least 2 ms (median), and the tick median does not rise. Velocity **rose** by
1.25 ms. Four calls a tick do more work than six did, so the code stays off develop, at the
tag `archive/perf-pm-velocity`.

`graph-cli tick --layout particle-mesh --n 1000000 --ticks 7 --workers 8 --passes`, frozen
release binaries of both trees, three rounds with the arms alternated, each run under
`~/goinfre/orch/bench.lock`. Each run waited until no full gate was running, ≥ 12 GB was
available and the 1-minute load was < 14. Raw output:
`$GM_SCRATCH/bench/pm-velocity/passes-{base,branch}-w8-r{1,2,3}.out`, script
`$GM_SCRATCH/bench/pm-velocity/passes.sh`.

| arm | run | load (1 min) | `motion::Velocity` calls | `motion::Velocity` ms/tick | serial ms/tick | passes tick ms | tick median ms |
|---|---|---|---:|---:|---:|---:|---:|
| base | r1 | 4.18 | 6 | 10.25 | 12.59 | 91.17 | 93.60 |
| branch | r1 | 4.41 | 4 | 10.78 | 12.93 | 89.46 | 88.60 |
| base | r2 | 4.41 | 6 | 10.12 | 12.72 | 91.02 | 93.60 |
| branch | r2 | 4.22 | 4 | 11.50 | 12.78 | 92.06 | 93.30 |
| base | r3 | 4.12 | 6 | 10.37 | 12.62 | 90.97 | 91.79 |
| branch | r3 | 4.12 | 4 | 11.59 | 13.13 | 91.36 | 93.22 |
| **base median** | | | 6 | **10.25** | 12.62 | 91.02 | 93.60 |
| **branch median** | | | 4 | **11.50** | 12.93 | 91.36 | 93.22 |

- **Velocity is +1.25 ms (+12%).** The fused run reads two delta columns and two `slot` gathers
  per node, so it saves one write-back of `vx`/`vy` per merge but adds a second random gather.
  At 1M nodes the gathers cost more than the traffic it saves. The stated premise, "the tick is
  memory-bound on the column re-read", does not survive this measurement.
- **The tick median moved −0.38 ms.** The three branch runs span 4.7 ms, so that is inside the
  spread, not a gain.
- **Memory:** +16 MB at 1M nodes (the `Mesh.link` column) for no measured gain.

Memory: `Mesh.link` holds 16 bytes a node, so **+16 MB at 1M nodes** and nothing at a size
that does not tick. That is the price of holding both delta columns at once; it buys back two
full passes over `vx`/`vy`.

## What this does not do

- `Caveat:` the host was shared. A peer's full gate (`gate-full-cb11`) held the CPU for most of
  the session and the bench waited it out, per the rule; the load each run saw is printed with
  it. The arms alternated, so they saw the same host, and only medians are claimed.
- It does not fuse the collide merge into the integrate's `Velocity` run: collide's projection
  reads `x + v`, so its deltas do not exist until after both merges are in the velocities, and
  the integrate already carries the collide merge and the decay in its two runs.
- It does not fuse the two axes. `vx` and `vy` are separate columns and a kernel writes one
  output column; fusing them would need a second `Runner` method and a change to
  `graph-wasm/src/pool.rs` and `graph-cli/src/exec_native.rs`, which is a concurrency change
  and not this job's.
- It does not touch gravity. `gravity::apply` is a one-thread loop skipped at the default
  `0.0`, so folding it into the integrate's `Velocity` run would move no byte at the default
  and would add a live `x` read to the hottest pass. Not attempted.
- No browser number: the wasm tick bench (`wasm-threads.mjs tick`) was not run on this tree.