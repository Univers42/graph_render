# Perf P3 scale: the wasm pool hands out chunks, not one part per thread

Measured 2026-10-03 on branch `perf-p3-scale` (from develop `d3b0e8e`). Host: dlesieur42, i5-13600KF:
CPUs 0–11 are six P-cores with hyperthreads, CPUs 12–19 are eight E-cores. Node from `scripts/orch/gr`.

Why: at eight workers the 1M live tick stopped near 3× (`perf-p3-session-threads.md`). A Chrome CPU
profile of the coordinator showed it blocked on its helpers for 32% of the tick. `PoolRunner` gave
helper `i` range `i` of `partition(len, workers)`, so every pass waited for its slowest thread, and on
this host a part that lands on an E-core or a busy hyperthread sibling runs about 2× longer.

## Design

| Piece | Where | What changed |
|---|---|---|
| chunk claim | `crates/graph-wasm/src/pool.rs` `PoolRunner::run` | a pass is cut into `min(len, parts × 16)` chunks. The calling thread and its helpers take chunks from one `AtomicU32` until none is left. Each chunk clears and writes its own span. |
| O(1) range | `crates/graph-core/src/exec/partition.rs` `range_at` | chunk `i` of `partition(n, k)` without building the list. `ranges` is now `range_at` over `0..k`. |
| negative control | `PoolRunner::skip_last` | the last chunk writes nothing (was: the last part) |

The bytes cannot move: a `StepRange` writes out[i] from start-of-pass state only (D10), so any
division of `0..len` gives the same column. `range_at` is checked against `partition` index by
index at (10, 3), (11, 4), (3, 7) and (1 000 003, 128), and is empty past the last range
(`exec/partition/tests/rule.rs`).

## Gates

`scripts/orch/gate.sh target/gate-scale scripts/orch/rows/perf-p3-scale.rows`:

| Row | Expect | Exit | What it shows |
|---|---|---|---|
| `fmt` | 0 | 1, then fixed | `rule.rs:95` was not rustfmt's layout. `cargo fmt --all` fixed it and touched no other file. |
| `clippy`, `clippy-threads` | 0 | 0 | workspace `-D warnings`, and graph-wasm built with `--features threads` |
| `test` | 0 | 0 | `cargo test --workspace --no-fail-fast` (1374 s, on a host shared with three landers) |
| `wasm-threads` | 0 | 0 | the threads artifact builds and imports only `env.memory` |
| `session-parity` | 0 | 0 | 8 seeds × both engines × workers {1,2,3,4,7}, 30 ticks: 0 of 80 cells differ from the serial artifact |
| `negctl-session-parity` | exit 1 | 1 | `--break`: 64 of 80 cells differ, every cell with two threads or more |

## Bench, one live tick

`scripts/orch/gr node harness/wasm-threads.mjs tick --n 1000000,400000 --workers 8`, branch and
develop artifacts run alternately on the same host. Each cell is the median and the minimum of 5
single-tick calls after one warm-up, on a fresh particle-mesh session over `gm_seed_handle(1, n)`.

| n | workers | before median / min ms | after median / min ms |
|---:|---:|---:|---:|
| 1 000 000 | 8 | 224.5 / 149.7 | 138.3 / 114.4 |
| 400 000 | 8 | 63.4 / 56.8 | 50.9 / 42.8 |

The coordinator's blocked share of the 1M tick fell from 32% to 3.9%. The helpers were busy 81% of
the tick (Chrome CPU profiles, `--cpu-prof`, one per thread).

Scaling at 1M after the change, median ms per tick:

| workers | 4 | 6 | 8 | 12 | 16 |
|---|---:|---:|---:|---:|---:|
| 1M tick | 158.7 | 133.4 | 114.5 | 93.9 | 90.4 |

## Where the 1M tick goes now

The serial tick pinned to one P-core (`taskset -c 10`, `--workers 1`) is about 404 ms. Its self time
by pass:

| pass | share |
|---|---:|
| collide gather | 37% |
| link force and pass | 16.6% |
| motion (merges, projection, integrate) | 11% |
| deposit | 10% |
| FFT | 10% |
| interpolate | 8% |
| counting sort | 3.2% |

At 16 workers about 24 ms of each tick still runs on the coordinator alone: the counting sort
(13–15 ms), the centering (about 2.75 ms) and link's merge (about 2.4 ms). That floor is the next
slice (perf-p3-serial).

## Rejected

- simd128 (`target/wasm-threads-simd`): the build with `+simd128` gave no gain in a pinned A/B, so
  it was dropped. The gather is bound by its random reads, not by arithmetic.

## What it does not do

- The plan's browser target, a 1M tick ≤ 100 ms, is met only at 12 workers and more. The studio
  clamps its helpers to 8 (`packages/graph-studio/src/motor/threads.ts`).
- The serial floor of about 24 ms per tick is untouched.
- Caveat: on a hybrid CPU an unpinned run can land the coordinator on an E-core, and the medians
  move by 20% between runs. The before/after rows were taken alternately on the same host so both
  arms saw the same load. Each row is one run.
