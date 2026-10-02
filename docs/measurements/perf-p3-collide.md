# Perf P3-collide — the collide gather filters its candidates without a branch

Measured 2026-10-02 on branch `perf-p3-collide` at `fa7f5a3`, against `bd75ed5` (`perf-p3-sqrt`).
Host dlesieur42, 20 cores, load average 25–35 for the whole run.

`collide::Gather` is the largest pass of a particle-mesh tick: 1 199.1 M of the 2 762.1 M
instructions of a 1M tick at eight threads (`perf-p3-link-once.md`). Each sorted slot reads up
to three slot runs around its cell and calls `resolve` on every other slot in them; `resolve`
returns early unless the two discs overlap.

## Why a branch was the cost

Line counts of the base build's callgrind output (`target/sq/cg-base.out`, the Gather window,
100k nodes, `--warm 1 --ticks 1`, so two ticks and 200 000 queries):

| base `collide.rs` line | Ir | per execution | executions |
|---|---:|---:|---:|
| 196 `let mut l = dx * dx + dy * dy;` (every candidate) | 24 768 640 | 5 | 4 953 728 |
| 209 `let push = (c.reach - dist) / dist * 0.5;` (every overlap) | 6 930 776 | 3 | 2 310 259 |

About 24.8 candidates per query and 47 % of them overlap, so the `l >= d2` branch goes each way
about as often as the other and mispredicts: 9.4 % of the window's branches (below).

Caveat: the executions are Ir divided by the instructions on that line, read off the base build's
code; a line whose instruction count differs from the assumed 5 and 3 shifts both counts by the
same factor, not the ratio.

## Design

`Grid::delta` walks each run `HITS = 64` slots at a time. `Grid::overlaps` writes every slot index
into `hits[found]` and advances `found` by `(l < d2) & (q != k)`, a compare and an add, no branch.
`resolve` then runs on the hits only, in ascending slot order, which is the order the old loop met
them in, so each slot's sum has the same terms in the same order. `l < d2` is false for a NaN `l`,
which is `resolve`'s own `l.is_nan() || l >= d2` negated, so the filter drops exactly what
`resolve` would have skipped. `resolve` is unchanged and still tests: the filter only removes the
calls that would have returned at once.

## Identity

`graph-cli snapshot --seed S --nodes N --layout force.particle_mesh --out-bin …` with the profile
builds of both commits (`target/sq/measure.sh`); the files are equal and so are their SHA-256:

| seed | n | both builds |
|---:|---:|---|
| 1 | 100 000 | `3d47b7d1` |
| 3 | 20 000 | `6c50077d` |
| 4 | 1 000 | `baeff1b6` |
| 5 | 5 000 | `453c138c` |

These are the hashes of `perf-p3-pm-serial.md` and `perf-p3-link-once.md`. No new layout id, no
golden update.

`collide/tests.rs::the_filtered_gather_is_the_branched_one_bit_for_bit` compares every slot's
delta against `branched`, the old loop kept as a test function, on a model whose longest run is
longer than `HITS` (asserted), so a run split over several batches is covered.

Negative control: the self-exclusion `& (q as usize != k)` dropped; `cargo test -p graph-core`
fails 3 tests and exits non-zero.

## Instructions and branches

`valgrind --tool=callgrind --branch-sim=yes --toggle-collect="*collide::Gather*"` on `graph-cli
tick --layout particle-mesh --n 100000 --warm 1 --ticks 1 --seed 1 --workers 1` (image
`ge-profile`, `target/sq/wall.sh`):

| | `bd75ed5` | `fa7f5a3` | change |
|---|---:|---:|---:|
| Ir | 163 600 688 | 252 071 611 | +54.1 % |
| conditional branches | 33 457 173 | 32 067 068 | −4.2 % |
| mispredicted | 3 131 073 | 2 159 831 | −31.0 % |
| mispredict rate | 9.4 % | 6.7 % | |

The filter computes `dx² + dy²` once and `resolve` computes it again for every hit, which is most
of the extra instructions. They are cheaper than the mispredicts they replace (below).

## Wall clock, native

`tick --layout particle-mesh --n 200000 --warm 2 --ticks 20 --seed 1 --workers 1`, six rounds, the
arm order alternating per round (`target/sq/wall.sh`). Tick median, ms:

| round | 1 | 2 | 3 | 4 | 5 | 6 | median |
|---|---:|---:|---:|---:|---:|---:|---:|
| base | 85.91 | 85.80 | 87.66 | 83.52 | 85.61 | 85.53 | 85.71 |
| new | 78.88 | 79.25 | 78.42 | 73.61 | 74.33 | 74.66 | 76.54 |

−10.7 % on the median of medians, and the new build is faster in every round. The fastest tick of
the 120 per arm: 68.02 → 62.76 ms (−7.7 %).

At 1M (`target/sq/measure.sh`, three rounds, `--ticks 6`) the load average was 25–31 and the ticks
spread by ±30 % within one arm, so those rows are recorded in `measure.out` and no claim is made
from them.

## Wall clock, wasm32 serial

`node harness/wasm-tick-bench.mjs --layout layout.force.particle_mesh --n N --repeat R`, the
release `graph_wasm.wasm` of each commit, two rounds with the arm order alternating
(`target/sq/bench.sh`, inside `scripts/orch/gr`). `tick_ms`, the settle time over its 112 ticks:

| n | repeat | base, rounds | new, rounds | base mean | new mean | change |
|---:|---:|---|---|---:|---:|---:|
| 100 000 | 3 | 53.21, 58.95 | 47.76, 52.83 | 56.08 | 50.30 | −10.3 % |
| 200 000 | 2 | 57.53, 58.37 | 47.66, 54.61 | 57.95 | 51.14 | −11.8 % |

## Gates

**Not run** on this branch alone. The merge floor runs when the branch lands through
`queue.sh land`; hashgate `--seeds 1000`, the Python oracles and mutants run once on develop.

## What it does not do

- **`dx² + dy²` is computed twice per hit**, once to filter and once in `resolve`. Passing it
  through would change `resolve`'s signature for the Barnes-Hut collide too; not measured.
- **The candidate count is unchanged**: cells one diameter wide still read about 24.8 slots per
  query to find about 11.6 overlaps. Finer cells or each pair resolved once change the summation
  order, so the bytes, and need a new layout id.
- **The counting sort that builds the grid is still serial**: 66 M of a 1M tick's 84 M serial
  instructions (`perf-p3-pm-serial.md`).
