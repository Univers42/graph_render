# Perf BH 1M: threaded Barnes-Hut at a million nodes, and where its tick goes

Measured 2026-10-03 on branch `perf-bh-1m` (6a44d24b, then develop merged in as ee8167e9 for the pass
table). Host: dlesieur42, i5-13600KF, 20 threads, 31 GB. Rust from `scripts/orch/gr`. Raw output:
`~/goinfre/bench/bh-1m/` (`run.sh`, `run.out`, `passes-8w.out`, `session.out`).

## Change

| File | What changed |
|---|---|
| `crates/graph-cli/src/bench/tick.rs` | `tick` passes `--workers` to Barnes-Hut; `--collide-radius`, a bench-only override |
| `barnes_hut/charge.rs` | `prepare_with` aggregates the charge arena on the runner, then calls `aggregate::finish` |
| `barnes_hut/charge/aggregate.rs` | the aggregate as a `StepRange` over preorder cells, plus the serial `finish` |
| `barnes_hut/charge/tests/aggregate.rs` | the divided aggregate equals the serial one, for any division |

`aggregate::Pass::step_range` computes the cells whose subtree lies inside its range. A cell whose
subtree a range boundary cuts is left at `Body::default()`, whose `skip` is 0, a value no computed
cell has (`skip > k` always). `finish` walks the arena downward and computes exactly those cells.
The per-cell arithmetic is `Pass::body`, unchanged, so the bytes do not move.

### The pool hang this fixed

The first version of `finish` recomputed the runner's division from the worker count. graph-wasm's
`PoolRunner` divides differently: `len.min(parts × 16)` chunks claimed from an atomic counter. Every
cut cell it left unfinished kept `skip == 0`, and the walk's `centre` then looped on it. The symptom
was the `graph-wasm` pool tests running for 80+ minutes. A `Runner` fixes the bytes, not the ranges,
so `finish` now finds the cut cells by their sentinel. The test
`any_division_of_the_aggregate_finishes_to_the_serial_one` covers 2, 3, 16, 48 and 112 chunks and one
chunk per cell. Its negative control (skip `finish`) fails with
`assertion left == right failed: 2 chunks`.

## Bench: 1M nodes, release, `--warm 5 --ticks 5`

Median ms per tick, one row per run, arms alternated, 3 rounds (`run.sh`).

| arm | r1 | r2 | r3 | median | load (1 min) |
|---|---:|---:|---:|---:|---|
| 1 worker | 1893.46 | 1987.73 | 2012.46 | **1987.73** | 5.65 / 5.72 / 5.02 |
| 4 workers | 942.56 | 948.25 | 957.07 | **948.25** | 4.68 / 5.48 / 4.21 |
| 8 workers | 828.79 | 777.29 | 926.41 | **828.79** | 4.35 / 5.10 / 6.15 |
| 16 workers | 670.04 | 664.12 | 661.96 | **664.12** | 4.14 / 5.55 / 6.22 |
| 8 workers, threaded aggregate | 780.28 | 779.82 | 762.63 | **779.82** | 3.81 / 5.26 / 5.88 |
| 8 workers, serial aggregate | 801.79 | 795.53 | 798.90 | **798.90** | 3.54 / 4.61 / 5.29 |
| 8 workers, `--collide-radius 0` | 924.07 | 772.76 | 708.78 | **772.76** | 4.30 / 4.61 / 4.95 |

- The threaded aggregate is 19 ms faster than the serial one (2.4 %) and won all three rounds. Kept.
- 1 → 16 workers is 3.0×.
- Collide radius 0 is within the run-to-run spread of the 8-worker row (709–924 ms). The pass
  table below shows why: with radius 0 the collide tree is still built and walked.

## Where the tick goes

`tick --n 1000000 --ticks 5 --warm 5 --workers 8 --passes` (develop's `Timed` runner):

| pass | calls/tick | ms/tick | share |
|---|---:|---:|---:|
| `step::CollidePass` | 1 | 156.43 | 19.8 % |
| `step::Pass` (charge walk) | 1 | 115.06 | 14.5 % |
| `aggregate::Pass` | 1 | 10.43 | 1.3 % |
| `step::LinkPass` | 1 | 5.96 | 0.8 % |
| `step::LinkForces` | 1 | 4.78 | 0.6 % |
| outside any pass (serial) | — | 498.66 | **63.0 %** |
| tick | — | 791.32 | 100 % |

Instructions inside `Sim::tick` (callgrind, `--toggle-collect=*Sim>::tick*`, 200 000 nodes, 1 worker,
one tick, 3.88 G instructions):

| part | instructions | share |
|---|---:|---:|
| charge walk (`step::Pass`) | 2 034 M | 52.4 % |
| collide pass (`node_delta`) | 1 037 M | 26.7 % |
| `Quadtree::build`, both trees | 590 M | 15.2 % |
| `Quadtree::flatten` | 155 M | 4.0 % |
| aggregate | ~46 M | 1.2 % |
| link | ~47 M | 1.2 % |

The serial part is the two quadtree builds and their flattening. They are 19 % of the instructions,
and 25 % of the one-worker tick (499 of 1988 ms), because inserting a million points into a pointer
tree misses cache on every level. No worker count shortens them, so at 8 workers they are 63 %.

## Byte identity and gates

| Check | Result |
|---|---|
| `any_division_of_the_aggregate_finishes_to_the_serial_one` | pass; negative control fails at 2 chunks |
| `cargo clippy --workspace --all-targets -D warnings` on the merged tree | 0 |
| `wasm-threads.mjs session`, BH and PM | rc 0, 0 differing cells (seeds 0–7, workers 1, 2, 3, 4, 7) |
| `wasm-threads.mjs session --break` | rc 1, 32 differing cells |
| fmt, workspace tests, wasm32, hashgate 8 and its negative control | `land.sh`'s `quick.rows` gate on the merged tree |

## Gap to the targets

| target | 1M BH now (median) | gap |
|---|---:|---:|
| ≤ 25 ms native tick | 664 ms at 16 workers, 829 ms at 8 | 27×, 33× |
| ≤ 100 ms browser tick | not measured here (pool at 8 workers ≥ native 8) | ≥ 8× |

Barnes-Hut is not the million-node engine. Particle-mesh at 1M is 107 ms at 8 workers
(`perf-pm-stencil.md`), and `docs/decisions/gpu-force-tier.md` moves it to the GPU.

## What it does not do

- Build the trees in parallel. Step 4 of the brief allows a key-sorted build when the serial
  prologue is above 15 %; it is 63 %. That build is a slice of its own, `perf-bh-build`. Removing
  all 499 ms would still leave about 300 ms at 8 workers.
- Change the opening criterion, `theta`, the frozen defaults or any per-cell floating-point order.
- The GPU tier, the wasm ABI, or the studio.

Caveat: the host ran other jobs during the bench (1-minute load 3.5–6.2), which spreads the
8-worker rows by up to 200 ms; only medians of three alternated rounds are claimed. The callgrind
shares are instruction counts, not time: they under-count the tree builds, which stall on memory.
