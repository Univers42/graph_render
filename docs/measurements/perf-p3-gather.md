# Perf P3-gather — one contiguous candidate window per cell

Measured 2026-10-02 on branch `perf-p3-gather`, the working tree on `96cfcc0`, against `96cfcc0`.
Host dlesieur42, 20 cores, load average 3.2–3.7 for the whole run (the `perf-p3-collide` landing gate
ran alongside it).

`perf-p3-collide.md` made the distance filter branchless. Each query still read its candidates run by
run: up to nine runs, 3.9 on average, each with its own loop and its own loop exit. The exit is
taken after a variable number of iterations, so it mispredicts on almost every run of every query.

## Design

`collide/gather.rs`. Every slot of one cell reads the same runs (`Grid::reads`), and consecutive slots
are mostly in the same cell. The cell's candidates are copied once into a 256-entry window (positions
and slots, `Reads` order). Each query then filters the window in one loop into a list of hit places,
and resolves those hits in a second loop. The querying slot's own place is known (the run that holds
its bucket, plus its offset), and its entry reads NaN while the filter runs. `l < d2` is false for a
NaN `l`, which is `resolve`'s own skip. A cell with more than 256 candidates is read one window at a
time, and each window is refilled on every query.

The terms of each slot's sum are added in the same order as before: the runs in `Reads` order, the slots
ascending in each run.

## v1, rejected: per-run batches

The first version kept the runs and filtered each one into a 64-entry hit batch. At 100k nodes, one
worker, it cut the Gather's instructions by 1.6 % and raised its mispredicts by 29 %:

| | base | v1 |
|---|---:|---:|
| Ir | 253 948 978 | 249 851 966 |
| mispredicted | 2 154 691 | 2 774 257 |

That version kept one loop per run plus one per batch, so it added exits rather than removing them. It
was not committed.

## Identity

`snapshot --layout force.particle_mesh --out-bin`, both builds (`target/gt/run.sh`):

| seed | n | base sha256 | new sha256 |
|---:|---:|---|---|
| 1 | 100 000 | `3d47b7d1…` | `3d47b7d1…` |
| 3 | 20 000 | `6c50077d…` | `6c50077d…` |
| 4 | 1 000 | `baeff1b6…` | `baeff1b6…` |
| 5 | 5 000 | `453c138c…` | `453c138c…` |

`collide::tests::the_filtered_gather_is_the_branched_one_bit_for_bit` compares every slot's delta
with the per-candidate branch, at every worker count. Its crowd fixture now has 300 nodes inside
17 units, and it asserts that some cell holds more than one window of candidates. Negative control: with
the NaN mask removed, the querying slot resolves against itself and 3 of the 21 `particle_mesh` tests
fail.

## Instructions and branches

`valgrind --tool=callgrind --branch-sim=yes --toggle-collect="*Gather*step_range*"` on `graph-cli
tick --layout particle-mesh --n 100000 --warm 1 --ticks 1 --seed 1 --workers 1` (image `ge-profile`):

| | `96cfcc0` | new | change |
|---|---:|---:|---:|
| Ir | 253 948 932 | 267 163 712 | +5.2 % |
| conditional branches | 32 067 066 | 28 189 596 | −12.1 % |
| mispredicted | 2 154 689 | 1 263 114 | −41.4 % |
| mispredict rate | 6.7 % | 4.5 % | |

The copy into the window is most of the extra instructions. It costs less than the mispredicts it
removes (below).

## Wall clock, native

`tick --layout particle-mesh --n 200000 --warm 2 --ticks 20 --seed 1 --workers 1`, six rounds, the
arm order alternating per round. Tick median, ms:

| round | 1 | 2 | 3 | 4 | 5 | 6 | median |
|---|---:|---:|---:|---:|---:|---:|---:|
| base | 59.14 | 57.90 | 61.20 | 59.59 | 58.83 | 59.78 | 59.37 |
| new | 53.07 | 51.39 | 55.04 | 54.36 | 54.10 | 53.25 | 53.68 |

The median of medians drops 9.6 %, and the new build is faster in every round.

`--n 1000000 --warm 2 --ticks 6 --workers 8`, three rounds:

| round | 1 | 2 | 3 | median |
|---|---:|---:|---:|---:|
| base | 115.60 | 117.23 | 113.13 | 115.60 |
| new | 110.70 | 105.91 | 108.46 | 108.46 |

The median drops 6.2 %. With three rounds the spread inside one arm (±2 %) is under a third of the
gap.

## Gates

`cargo test -p graph-core --lib particle_mesh` (21 passed), `--test tick_alloc` (a steady tick still
allocates nothing: the window is on the stack, about 5.5 KB per range), fmt, and clippy `-D warnings`
on graph-core. The merge floor runs when the branch lands through `queue.sh land`. Hashgate
`--seeds 1000`, the Python oracles and mutants run once on develop: **not run** here.

## What it does not do

- **A crowded cell is refilled on every query.** Past 256 candidates, every query copies every window
  again. That is linear in the crowd, as before, but it pays the copy too. Caveat: a dense overlap
  (thousands of nodes within one diameter) may be slower than in `96cfcc0`; not measured.
- **The candidate count is unchanged**: about 25 per query, to find about 12 overlaps
  (`perf-p3-collide.md`).
- **wasm32 is not measured** for this slice.
