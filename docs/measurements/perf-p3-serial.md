# Perf P3 serial: the coordinator-only passes of the particle-mesh tick

Measured 2026-10-03 on branch `perf-p3-serial` (from `perf-p3-scale`). Host: dlesieur42, i5-13600KF
(six P-cores with hyperthreads, eight E-cores). Node from `scripts/orch/gr`.

Why: after `perf-p3-scale` about 24 ms of each 1M tick at 16 workers still ran on the coordinator
alone (`perf-p3-scale.md`). That time was the counting sort's scatter and its copy (13–15 ms), the
centering shift (about 2.75 ms) and link's velocity merge (about 2.4 ms).

## Design

| Piece | Where | What changed |
|---|---|---|
| counting sort | `particle_mesh/collide.rs` `Grid::build` | `start` gets two more entries than there are buckets. The sort counts two ahead and scatters one ahead, so the scatter leaves `start` in place and the `copy_within` is gone. The scatter itself is still one thread's. |
| sorted positions | `collide.rs` `Sorted` | the gather `at[k] = xy[order[k]]` is a `StepRange` through the runner; it was a serial loop inside the scatter |
| link merge | `barnes_hut/link.rs` `pass_with`, `particle_mesh.rs` | the particle mesh takes link's deltas unmerged and merges them with `motion::merge` as a range pass. `Gathered.slot` is `None` for deltas in node order. Barnes-Hut still calls `apply_with`, which merges serially as before. |
| centering | `barnes_hut/sim.rs` `center_shift`, `motion.rs` `center` | the mean stays one thread's fold in node order (D3); the shift `x[i] -= dx` is a `Shift` pass into `px`/`py`, then swapped |

Every new pass writes `out[i]` from start-of-pass state only (D10), with the expression and the
evaluation order of the serial code it replaces, so the bytes do not move.
`motion/tests.rs` checks `motion::center` against `Sim::center`, and the link merge against
`step::merge`, at several worker counts.

## Gates

`scripts/orch/gate.sh target/gate-serial scripts/orch/rows/perf-p3-serial.rows`:

| Row | Expect | Exit | What it shows |
|---|---|---|---|
| `fmt` | 0 | 0 | |
| `clippy`, `clippy-threads` | 0 | 0 | workspace `-D warnings`, and graph-wasm built with `--features threads` |
| `test` | 0 | 0 | `cargo test --workspace --no-fail-fast` (1558 s, on a shared host) |
| `wasm-threads` | 0 | 0 | the threads artifact builds and imports only `env.memory` |
| `session-parity` | 0 | 0 | 8 seeds × both engines × workers {1,2,3,4,7}: 0 of 80 cells differ from the serial artifact |
| `negctl-session-parity` | exit 1 | 1 | `--break`: 64 of 80 cells differ |

## Bench, one live tick

`target/ab/run.sh`: the `perf-p3-scale` artifact and this branch's, built with `--features threads`,
run alternately, three rounds each: `harness/wasm-threads.mjs tick --n 1000000,400000 --workers
8,12,16 --ticks 7`. Each run's cell is the median of its ticks; the table gives the median of the
three runs, with the three run medians after it.

| n | workers | before (`perf-p3-scale`) ms | after ms | change |
|---:|---:|---|---|---:|
| 1 000 000 | 8 | 100.7 (100.4, 109.5, 100.7) | 94.3 (94.3, 93.2, 100.9) | −6.4% |
| 1 000 000 | 12 | 91.2 (91.2, 92.7, 90.0) | 84.5 (88.0, 84.5, 82.0) | −7.4% |
| 1 000 000 | 16 | 82.8 (87.2, 82.2, 82.8) | 79.6 (79.6, 78.7, 79.7) | −3.9% |
| 400 000 | 8 | 39.0 (40.3, 38.3, 39.0) | 37.9 (37.1, 37.9, 38.7) | −2.8% |
| 400 000 | 12 | 35.7 (34.9, 37.6, 35.7) | 35.1 (35.1, 35.4, 34.2) | −1.7% |
| 400 000 | 16 | 33.0 (33.0, 31.1, 34.9) | 33.0 (33.0, 34.0, 30.7) | 0% |

At 1M the branch is faster in every round at 12 and 16 workers, and in two of three at 8. At 400k
the difference is inside the spread between rounds.

Caveat: the host's load average was 8–10 during the run (three landers shared it), and the
rounds of one arm spread by up to 9%. The arms alternate so both saw the same load, but a
difference under about 5% here is not a result.

## What it does not do

- The counting sort's scatter is still serial: each node's cursor depends on every node before it.
  A stable parallel scatter (per-chunk counts, then a prefix over chunks × buckets) is the next
  slice; its memory is chunks × buckets `u32`, so the chunk count must stay small.
- The centering mean is still one thread's fold, by D3. A fixed-order tree reduction would change
  the bytes and needs a new layout id.
- Barnes-Hut's link merge is unchanged.
- The 1M tick meets the browser target (≤ 100 ms) at 8 workers and more in this run, but the
  studio's own number needs `live-tick.py` after this lands.
