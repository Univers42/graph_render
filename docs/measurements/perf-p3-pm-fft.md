# Perf P3-PM-FFT — the particle-mesh FFT, threaded and pruned

Measured 2026-10-02 on branch `perf-p3-pm-fft`: the FFT at `5d9dc5c`, `tick --workers` at
`de4459a`, against the serial FFT at `c73df2f` (`perf-p2-pm`). Host dlesieur42, 20 cores, shared:
the load average from other sessions' jobs was 22–35 during this slice, so every wall-clock number
below is printed with its load, and the decisions rest on callgrind's instruction counts instead.

Before this slice, `Mesh::solve` ran its two 2D transforms on one thread, as rows in place, a
transpose, then rows again. On the native 1M tick that was 27.7 % of the instructions, all serial,
and it was the largest reason model (a) stopped near 2.5× (`docs/measurements/perf-p3-wasm-threads.md`,
"Why it stops near 2.5×").

## Design

- `fft/pass.rs`: a 2D transform is two range passes through the `Runner` seam. Output row `r` of a
  pass is one line transform of input row `r` (`Read::Rows`) or of input column `r`
  (`Read::Columns`, the transpose fused into the load). `Read::RowsTimes(gain)` multiplies by the
  kernel spectrum while loading, so the convolution's product is no separate pass.
- Two exact prunings. The forward transform skips input rows from `cells` on: the deposit leaves
  them +0, and a line of +0 transforms to +0. The inverse skips output rows from `cells` on,
  which no node reads.
- A range that starts or ends inside a row transforms that row on the stack (`[C; MAX_SIDE]`,
  `MAX_SIDE` = 1024), so any `partition` of the rows gives the same lines.
- Every line sees the same inputs in the same order as the old sequence, so the bytes are the old
  ones. No new layout id, no golden update.

## Identity

| check | command | expect | exit |
|---|---|---|---|
| a line against the naive DFT, both directions | `gr cargo test -p graph-core fft` (5 tests) | 0 | 0 |
| the passes against the old rows/transpose/rows sequence, bit for bit, side 8 and 64, `cells` = side and less, workers 1, 2, 3, 7 | the same run (`the_passes_are_the_reference_bit_for_bit`) | 0 | 0 |
| negative control: `pass.rs` with the partial-row path broken | the same run on the broken file | non-zero | 101 (3 of 5 red) |

The snapshot of a whole particle-mesh stage, `graph-cli` built at `c73df2f` and at `5d9dc5c`,
SHA-256 of the binary snapshot:

| seed | n | both builds |
|---:|---:|---|
| 1 | 100 000 | `3d47b7d1…` |
| 2 | 100 000 | `3d47b7d1…` |
| 3 | 20 000 | `6c50077d…` |
| 4 | 1 000 | `baeff1b6…` |
| 5 | 5 000 | `453c138c…` |

`graph-cli snapshot --seed S --nodes N --layout force.particle_mesh --out-bin …`; the old build
is this tree with `particle_mesh/` checked out at `c73df2f`. Seeds 1 and 2 share a hash: the seed
only re-draws node sources and edge kinds (`remix`, `crates/graph-core/src/stage/topology.rs:37`),
and the structure comes from `n` alone, so at one size the force layout sees one graph.

## Instructions per tick, one thread

`valgrind --tool=callgrind '--toggle-collect=*ParticleMeshRun>::step_with*'` on
`graph-cli tick --layout particle-mesh --seed 1` (image `ge-profile`), collected over the warm
tick and the measured ones:

| n | ticks collected | `c73df2f` | `5d9dc5c` | change |
|---:|---:|---:|---:|---:|
| 100 000 | 4 | 1 397.3 M | 1 298.0 M | −7.1 % |
| 1 000 000 | 2 | 5 007.4 M | 4 861.9 M | −2.9 % |

The pruning removes a quarter of the line transforms. The rest of the gain is threading, below.

## The serial part of a 1M tick, eight threads

`valgrind --tool=callgrind --separate-threads=yes` on
`graph-cli tick --layout particle-mesh --n 1000000 --warm 1 --seed 1 --workers 8`, run at
`--ticks 1` and `--ticks 3`; the per-tick figure is the difference over two ticks, so ingest and
setup cancel. `Threads` spawns one thread per range and the calling thread waits, so everything
counted on thread 1 is serial.

| thread | Ir per tick |
|---|---:|
| 1, the caller: serial | 374.6 M |
| 2–10, the ranges: threaded | 2 273.4 M |
| serial share | 14.1 % |

Amdahl on these counts, perfect balance and free cores assumed: 1.75× at 2 threads, 2.81× at 4,
**4.02× at 8**, 5.12× at 16, 7.07× at most. Before this slice the transforms' 693 M sat in the
serial part too (27.7 % of a 2 504 M tick at `c73df2f`, `callgrind_annotate --inclusive=yes`),
which puts the serial share near 43 % and eight threads near 2.0×.

What is left on thread 1, `callgrind_annotate` self cost, per tick:

| function | Ir | share of serial |
|---|---:|---:|
| `Mesh::deposit` | 130.0 M | 34.7 % |
| `collide::Grid::build` | 96.8 M | 25.8 % |
| `frame::place` | 52.0 M | 13.9 % |
| `barnes_hut::step::merge` | 46.5 M | 12.4 % |
| `Sim::integrate` | 28.0 M | 7.5 % |
| `memset` (the `Runner` refilling `out`) | 7.7 M | 2.1 % |
| `Sim::center` | 7.3 M | 1.9 % |
| `collide::apply` | 5.5 M | 1.5 % |

The plan's P3 exit, native ≥ 5× on 8 cores at 1M, is **not reachable** from this tree even on
free cores: the bound is 4.02×. The deposit, the merge and the integrate are the next slice; the
grid build after them.

## Wall clock, for the record

`graph-cli tick --layout particle-mesh --n 1000000 --ticks 4 --warm 1 --seed 1 --workers W`,
the build of `de4459a`, two rounds, load average (1 min) 28–35 on 20 cores throughout:

| workers | round 1 min / median ms | round 2 min / median ms |
|---:|---|---|
| 1 | 532.5 / 640.8 | 655.7 / 667.3 |
| 2 | 395.9 / 534.2 | 725.0 / 788.8 |
| 4 | 449.8 / 518.8 | 571.5 / 714.0 |
| 8 | 519.3 / 582.9 | 767.1 / 853.0 |

With more runnable threads than cores, these numbers measure the other jobs, not the threads.
They are not used for any conclusion here; a quiet host re-runs this table.

## Gates

GATES_PENDING

## What it does not do

- **The `Runner` still clears and refills every pass's `out`**, a serial write of `side²` samples
  per transform pass (16 MiB at side 1024). Instruction counts make it look small (2.1 % of the
  serial part); it is memory traffic, which callgrind does not weigh.
- **The deposit, the frame, the collide grid build, merge, center and integrate are serial.**
- **No wall-clock speed-up is claimed**: the host never had eight free cores during this slice.
- **The kernel spectrum refresh** runs through the same passes when the rung moves; it was not
  measured on its own.
