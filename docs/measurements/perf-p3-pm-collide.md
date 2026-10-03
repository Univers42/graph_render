# Perf P3-PM-collide — a masked overlap test for the collide gather (rejected)

Measured 2026-10-02 on branch `perf-p3-pm-collide`, against `27c3828` (`perf-p3-pm-serial`). Host
dlesieur42, 20 cores, load average 20–22 for the whole run. **Result: not kept.** Both attempts were
byte-identical and both were slower; the code is back to `27c3828`, and this page is what is left.

## Why collide

`collide::Gather` is the largest pass of a 1M tick: 1 215.5 M of 2 953.7 M instructions
(`perf-p3-pm-serial.md`). Per query node (cachegrind at 100 000 nodes, one worker, `--branch-sim=yes`,
per tick = (ticks 3 − ticks 1) / 2, per source line through the line tables):

| what | per tick | per node |
|---|---:|---:|
| candidates tested (the `l >= d2` branch, `collide.rs:197` at `27c3828`) | 3.50 M | 35 |
| overlaps resolved (the `dx == 0.0` branch in `resolve`) | 1.22 M | 12.2 |
| mispredictions of that test | 1.02 M | 10.2 |
| mispredictions in all of `Gather` | 1.77 M | 17.7 |

One candidate in three overlaps, so the test's branch is close to a coin toss. The candidates are
this many because the warm ticks are the crowded start: d3's phyllotaxis puts one node per ~314
units², so a 32-unit disk holds about ten others.

## What was tried

Both attempts keep every hit in slot order, so `resolve` adds the same terms in the same order.

1. **AoS, 8 lanes.** The test runs over chunks of 8 `[f64; 2]` as a bit mask, then the set bits
   are walked lowest first.
2. **SoA, 4 lanes, padded.** The sorted positions are two columns (`ax`, `ay`), each written by a
   threaded gather (`Sorted`, a `StepRange` over `order`) and padded with 4 NaN. The test reads
   fixed `&[f64; 4]` lanes, and a `live` mask drops the slots past the run and the querying slot.

Identity: each snapshot from attempt 2 is byte-equal to `perf-p3-pm-serial`'s
(`graph-cli snapshot --layout force.particle_mesh`, seeds 1, 3, 4 and 5 at 100 000, 20 000, 1 000 and
5 000 nodes; `cmp` silent). The test against the one-by-one scan passed bit for bit. Its negative
controls all exited 101: the querying slot left in, the bits walked highest first, and the run
end off by one. The run-end control needed a unit test of `live`, because no dataset made it fail.

## Measured

Cachegrind, `Gather` per tick at 100 000 nodes, one worker:

| | Ir | Bcm |
|---|---:|---:|
| `27c3828` | 106.0 M | 1.77 M |
| attempt 1 | +21 % | −18 % |
| attempt 2 (`Gather` 169.4 M + `resolve`, no longer inlined, 39.1 M) | 208.5 M | 1.65 M |

The misprediction moved instead of going away. In attempt 2 the exit of the bit walk
(`while hits != 0`) mispredicts 0.97 M times per tick, where the old test missed 1.02 M times. A chunk
of four holds 0 to 4 hits, which is as unpredictable as one candidate.

Wall clock, `graph-cli tick --layout particle-mesh --n 1000000 --warm 2 --ticks 6 --seed 1 --workers W`,
release builds of `27c3828` (base) and attempt 2 (new), three rounds with the arm order alternating.
Tick median in ms, per round, then the median of the three:

| workers | base, per round | base | new, per round | new | change |
|---:|---|---:|---|---:|---:|
| 1 | 580.4, 552.6, 571.0 | 571.0 | 781.9, 805.3, 813.3 | 805.3 | +41 % |
| 8 | 254.9, 480.6, 210.9 | 254.9 | 257.5, 280.2, 300.1 | 280.2 | +10 % |

## What it leaves open

- **No branch at all**: add every candidate's term, with `-0.0` for a non-hit. `x + (-0.0) == x`
  for every `x`, NaN included, so the sum stays exact. The price is a square root and a division
  per candidate, 35 instead of 12. Not tried.
- **Compact, then resolve**: append each candidate's slot to a per-node buffer with no branch
  (`buf[len] = q; len += hit`), then resolve the buffer. That is one unpredictable exit per run plus
  one per node, against ten per node now. Not tried; the gain is bounded by the mispredictions, about
  a fifth of `Gather`.
- **Fewer candidates** changes the bytes: finer cells, or each pair visited once and added to both
  nodes. That needs a new layout id or a recorded golden update.
