# Perf P3-psort — a threaded grid sort, measured and not kept

Measured 2026-10-02 on branch `perf-p3-psort` at `1e71d25`, against `af3bc79` (`perf-p3-collide`).
Host dlesieur42, 20 cores, load average 27–29 for the whole run.

The particle-mesh collide grid is built by a one-thread counting sort: 66.2 M of the 83.7 M
instructions a 1M tick runs outside its workers (below). This slice tried to thread it without
moving a byte, and is **not kept**: the code is reverted, the stronger build test stays.

## Design tried (`1e71d25`, `collide/sort.rs`)

Five range passes, each a gather: every range sorts its own keys `bucket << 32 | node`, read in
the previous build's slot order (`sort_unstable`); every output range finds its first key in every
run by bisecting the key space, then merges the runs' heads; then the positions, the bucket starts
and each node's slot (a binary search inside its bucket) are read off the merged order.

## Identity

`graph-cli snapshot --seed S --nodes N --layout force.particle_mesh --out-bin …`, release builds of
both commits (`target/ps/build.sh`, `measure.sh`):

| seed | n | both builds |
|---:|---:|---|
| 1 | 100 000 | `3d47b7d1` |
| 3 | 20 000 | `6c50077d` |
| 4 | 1 000 | `baeff1b6` |
| 5 | 5 000 | `453c138c` |

## Instructions per tick

`valgrind --tool=callgrind --separate-threads=yes` on `graph-cli tick --layout particle-mesh
--n 1000000 --warm 1 --ticks T --seed 1 --workers 8`, `T` = 1 and 3, in `ge-profile`; a tick is
the difference over two. Self cost summed over the thread files:

| M Ir per tick | `af3bc79` | `1e71d25` |
|---|---:|---:|
| whole tick | 3 430.5 | 3 872.9 |
| main thread (not on a worker) | 83.7 | 16.3 |
| workers, summed | 3 346.8 | 3 856.6 |
| critical path, main + workers / 8 | 502.1 | 498.4 |
| the sort's own passes | 93.2 | 506.6 |

The sort's passes in `1e71d25`: `Starts` 116.0, `Merge` 109.4, the `u64` quicksort and its small
sorts 152.1, `Slots` 78.2, `Runs` 39.0, `Place` 12.0. In `af3bc79`: the counting sort 66.2,
`Buckets` 27.0.

Moving 67 M off the main thread cost 510 M more on the workers: the critical path is unchanged.

## Wall clock, native

`tick --layout particle-mesh --n 1000000 --warm 2 --ticks 6 --seed 1 --workers W`, three rounds,
the arm order alternating per round. Tick median, ms:

| W | arm | round 1 | round 2 | round 3 |
|---:|---|---:|---:|---:|
| 4 | `af3bc79` | 324.34 | 358.50 | 311.99 |
| 4 | `1e71d25` | 444.26 | 345.87 | 373.70 |
| 8 | `af3bc79` | 237.51 | 283.96 | 253.47 |
| 8 | `1e71d25` | 376.35 | 349.05 | 391.17 |

At eight workers the threaded sort is slower in every round. Why it is slower than its unchanged
critical path predicts is not measured (cache misses were not counted).

## Decision

Reverted. A threaded sort is worth having only if it adds less work than it takes off the main
thread; one that does is a parallel radix sort, whose scatter is not a gather and so does not fit
`Runner::run` as it stands.

Kept: `collide/tests.rs::every_division_of_the_build_is_the_one_thread_build` now compares the
positions bit for bit and rebuilds after a move, at workers 2, 3, 7, 8, 64 and 100.

## Gates

`cargo test` on the reverted tree; the merge floor runs when the branch lands. Hashgate
`--seeds 1000`, the Python oracles and mutants: **not run** here.
