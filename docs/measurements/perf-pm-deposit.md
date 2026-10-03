# Perf PM deposit: one pass over the slots per tick, not one per chunk

Measured 2026-10-03 on branch `perf-pm-deposit` (from develop `d1f85bc`, which carries the pool's
chunk claiming). Host: dlesieur42, i5-13600KF (6 P-cores with HT + 8 E-cores). Node from
`scripts/orch/gr`.

Why: a 1M particle-mesh tick at 16 workers (`node --cpu-prof` over the threads artifact) spent
4.3 s of 15.5 s of non-wait samples in `Deposit::step_range`. That was 27.6% of compute and the top
kernel. Each range scanned all n slots and kept the ones whose cells fell in the range. The pool cuts
a pass into `min(len, parts * 16)` chunks, so 16 workers made 256 full scans of 1M slots per tick.
The cost grew with the worker count, which is the wrong way round.

## Design

| Piece | Where | What changed |
|---|---|---|
| row index | `particle_mesh/deposit.rs` `Rows` | a counting sort of the slots by the mesh row of their lower-left cell: `starts` (side + 1) and `slots` (n), allocated once in `Mesh::new` and refilled per tick, O(n + side) |
| the kernel | `deposit.rs` `Deposit::step_range` | a range walks only the rows it touches. A slot whose lower-left cell is in row r writes rows r and r + 1, so row r's cells take the slots of rows r − 1 and r. The two lists are merged by slot index, so every cell still sums its contributions in slot order |
| wiring | `particle_mesh/mesh.rs` | `rows.sort(at, side)` after the stencil pass, before the deposit pass |

Why the bytes cannot move: each cell's sum is a fixed-order reduction over slots (D2). The old scan
added contributions in ascending slot order. The merge also yields ascending slot order, because both
lists are ascending (the counting sort is stable) and the merge takes the lower head. A slot with
`NONE` (a non-finite node) is in no row and deposits nothing, as before. Cells never wrap rows,
because `frame::stencil` clamps `cx` to `cells - 2`.

## Gates

| Check | Result |
|---|---|
| `cargo test -p graph-core particle_mesh` | 25 + 1 pass. The bit-for-bit test runs workers 1, 2, 3, 7, 64 and 4096; 4096 cuts most rows, so ranges start and end mid-row |
| mutation: the merge takes the higher head (`a > b`) | the bit-for-bit test fails, as it should; restored |
| `rows_hold_every_finite_slot_once_ascending_in_its_row` | pass. It covers a `NONE` slot, an empty row and a re-sort |
| `clippy -D warnings`, `fmt --check`, wasm32 build | 0, 0, 0 |
| `hashgate --seeds 8` | PASS, 4-way equal on 8/8 seeds |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | FAIL, 8 of 8 seeds diverge (exit 1, as expected) |
| `harness/wasm-threads.mjs hash`, threads vs serial of this branch, n 20000 and 300000, workers 1, 2, 3, 7, 16 | 0 differing |
| the same with `--break` | 18 differing, exit 1 |
| new threads artifact vs the serial artifact of `d1f85bc`, 8 gate seeds, particle_mesh and barnes_hut, workers 1, 2, 7, 16 | 0 differing: the output bytes match the previous release |

## Bench: wasm threads, `tick` mode

`harness/wasm-threads.mjs tick --wasm <arm> --n 1000000,400000 --workers 8,12,16 --ticks 7`. The two
artifacts are the threads build of `d1f85bc` (`base`) and of this branch (`deposit`). There were
three rounds with the arms alternated (`base`, `deposit`, `base`, …). Load went from 12.7 to 16.2
over the run; landers and OpenCode jobs shared the host. Each cell below is the median of the three
per-round medians, in ms/tick.

| n | workers | base, 3 rounds | deposit, 3 rounds | base | deposit | change |
|---:|---:|---|---|---:|---:|---:|
| 1000000 | 8 | 119.2, 119.8, 111.9 | 112.0, 103.9, 100.3 | 119.2 | 103.9 | −12.8% |
| 1000000 | 12 | 99.0, 123.9, 101.0 | 89.7, 92.1, 76.3 | 101.0 | 89.7 | −11.2% |
| 1000000 | 16 | 114.9, 107.2, 99.2 | 82.9, 89.4, 89.3 | 107.2 | 89.3 | −16.7% |
| 400000 | 8 | 44.8, 45.7, 45.0 | 45.4, 37.4, 40.1 | 45.0 | 40.1 | −10.9% |
| 400000 | 12 | 42.9, 41.0, 39.9 | 42.1, 33.5, 32.0 | 41.0 | 33.5 | −18.3% |
| 400000 | 16 | 47.3, 44.3, 44.0 | 37.1, 30.0, 27.6 | 44.3 | 30.0 | −32.3% |

In 17 of the 18 same-round pairs the deposit arm is faster. The one exception is round 1, 400k at 8
workers: 45.4 against 44.8. The gain grows with the worker count, which matches the cause: the old
cost was chunks × n.

## Profile: where the 1M tick goes now

`node --cpu-prof` over `tick --n 1000000 --workers 16 --ticks 5`. Before: `perf-p3-steal`'s
`target/prof-steal` (`d1f85bc` threads build). After: `target/prof-dep`. Self time is summed over
every thread and grouped by kernel. Condvar waits are left out (75% and 83% of the samples).

| kernel | before ms | after ms |
|---|---:|---:|
| `Deposit::step_range` (+ `Rows`) | 4287 | 1319 |
| collide (gather, resolve) | 2374 | 2798 |
| motion (velocity, axes) | 2034 | 2437 |
| indexmap (graph build, not the tick) | 1469 | 2624 |
| barnes_hut | 1077 | 1435 |
| FFT | 532 | 681 |
| non-wait total | 15522 | 15951 |

Deposit fell by 69%, from 27.6% of compute to 8.3%. Collide's gather is now the top kernel.

## What it does not do

- The `Rows` sort is serial, O(n + side), about 1M slots per tick on the coordinator. It is cheap
  against the old 256 scans, but it is the next serial piece of this pass if the worker count grows.
- Caveat: a range walks both whole lists for every row it touches. A row crowded with slots, such as a
  graph collapsed onto one line, costs a range that only touches a few cells the full list. The worst
  case is one row holding every slot, which is the old cost (n per range), never more.
- Caveat: the host was shared (load 10.7–16.2, landers and OpenCode jobs). The arms alternated, so
  they saw the same load, but each change is from 3 rounds of 7 ticks.
- The two profiles are not comparable in absolute terms. The after run was on a busier host: the
  graph build, which is identical work in both, took 1.8× as long. Only the deposit's share is
  compared. The wall-clock evidence is the alternated bench above.
