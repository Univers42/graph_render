# Perf P3-PM-serial — the particle-mesh tick's serial part, threaded

Measured 2026-10-02 on branch `perf-p3-pm-serial` at `969b89a`, against `de4459a` (the threaded
FFT, `docs/measurements/perf-p3-pm-fft.md`). Host dlesieur42, 20 cores, shared with other
sessions' jobs (load average 19–35 all day), so the decisions rest on callgrind's instruction
counts and the wall clock is printed with its load.

After the FFT slice, eight threads were capped at 4.02× by Amdahl: 374.6 M instructions of a 1M
tick still ran on the calling thread (14.1 %). The largest pieces were the charge deposit, the
collide grid build, the frame, the velocity merges and the integrate.

## Design

Every pass below goes through the `Runner` seam (`exec/partition.rs`), and every output element
is the old loop's expression, evaluated in the old order, so the bytes are the old ones. No new
layout id, no golden update.

- **Deposit** (`deposit.rs`). `Stencils` finds each sorted slot's lower-left cell, one output per
  slot. `Deposit` divides the **cells**: a range walks every slot in grid order and adds only the
  weights that land inside it, so each cell's terms arrive in the one-thread order.
- **Frame bounds** (`frame.rs`). The bounding box is a fold over blocks of 4096 nodes (`Blocks`),
  then a serial fold of `n / 4096` boxes in block order. Min and max are exact, so the order only
  matters for NaN, and the fold keeps the old one.
- **Collide grid** (`collide.rs`, `collide/hash.rs`). The bounds reuse `Blocks`; each node's bucket
  is a range pass (`Buckets`). The counting sort (histogram, prefix sum, scatter) stays serial.
- **Velocity merges, projection, integrate** (`motion.rs`). `step::merge` scatters slot `k`'s delta
  onto node `order[k]`; `Velocity` gathers it from the node's side through `slot[i]`, so each
  node writes only its own output. Each pass writes into `px`/`py` and the result is swapped in.
- **The runner fills per span** (`exec_native.rs`, `partition.rs`). `out` is resized, not cleared;
  each range fills its own span with `Default` on its own thread, which `Deposit`'s `+=` needs.
  `Threads` with fewer than two workers runs `Serial`.

## Identity

The snapshot of a whole particle-mesh stage, `graph-cli` built at `de4459a` and at `969b89a`,
SHA-256 of the binary snapshot (`graph-cli snapshot --seed S --nodes N --layout
force.particle_mesh --out-bin …`; the old build is this tree with `layout/force/` checked out at
`de4459a`):

| seed | n | both builds |
|---:|---:|---|
| 1 | 100 000 | `3d47b7d1…` |
| 3 | 20 000 | `6c50077d…` |
| 4 | 1 000 | `baeff1b6…` |
| 5 | 5 000 | `453c138c…` |
| 6 | 100 000 | `3d47b7d1…` |

These are the hashes of `c73df2f` in the FFT report, so the three slices together change no byte.
`snapshot` caps `--nodes` at 100 000, so seed 6 ran there; at one size the seed does not change
the graph (the FFT report says why), so it equals seed 1.

Threads against one thread, bit for bit, in the unit tests:

| test | covers |
|---|---|
| `deposit::tests::every_division_of_the_cells_is_the_one_thread_loop_bit_for_bit` | workers 1, 2, 3, 7, 64; side 128 and 256; NaN and infinite nodes |
| `frame::tests::the_blocked_bounds_are_one_fold_at_every_worker_count` | the block fold against one fold |
| `collide::tests::every_division_of_the_build_is_the_one_thread_build` | the bucket pass at every division |
| `motion::tests::the_passes_are_merge_and_integrate_bit_for_bit` | the gathered merge and integrate against `step::merge` and `Sim::integrate`, pins, decay |
| `particle_mesh::tests::the_layout_is_finite_and_the_same_run_to_run_and_at_every_worker_count` | a whole run divided 2, 3 and 7 ways, 120 nodes |
| `graph-cli` `exec_native::tests` | `Threads` writes the bytes `Serial` writes for the same division, up to 1M outputs at 7 workers |

No gate on this branch hashes a threaded run of the whole stage; the 10-way hash gate with
threaded arms is `perf-p3-pm-tiers`'s.

Negative controls, each a one-line break of this slice's code, run with
`cargo test -p graph-core particle_mesh`:

| break | expect | exit |
|---|---|---|
| `Deposit` without the `at + side + 1 < start` skip | non-zero | 101 (subtract overflow) |
| `Deposit` adding a cell outside its range | non-zero | 101 (side 128, workers 64, cell 582) |
| `Velocity`: `v = v + axis(d) + stolen` (rounding order) | non-zero | 101 |
| `Position` ignoring the pins | non-zero | 101 |
| `Buckets` hashing `0..out.len()` instead of its range | non-zero | 101 |
| the bounds fold reversed (`.rev()`) | non-zero | 101 |

## The serial part of a 1M tick, eight threads

`valgrind --tool=callgrind --separate-threads=yes` on `graph-cli tick --layout particle-mesh
--n 1000000 --warm 1 --seed 1 --workers 8` (image `ge-profile`, release with
`debug = "line-tables-only"`), run at `--ticks 1` and `--ticks 3`; the per-tick figure is the
difference over two ticks. `Threads` spawns one thread per range and the caller waits, so
thread 1 is the serial part.

| | `de4459a` | `969b89a` |
|---|---:|---:|
| thread 1, serial, Ir per tick | 374.6 M | 83.6 M |
| the ranges, threaded, Ir per tick | 2 273.4 M | 2 870.1 M |
| serial share | 14.1 % | 2.83 % |
| Amdahl at 8 threads | 4.02× | 6.68× |
| Amdahl at 16 threads | 5.12× | 11.2× |

What is left on thread 1 (`callgrind_annotate --auto=no`, self cost, per tick):

| function | Ir |
|---|---:|
| `collide::apply` (the counting sort, inlined: histogram, prefix sum over 2²¹ buckets, scatter, shift) | 66.2 M |
| `barnes_hut::step::merge` (the link pass's merge) | 8.5 M |
| `Sim::center` | 7.3 M |
| `memcpy`, `memset` | 1.4 M |

The threaded part, largest first:

| function | Ir |
|---|---:|
| `collide::Gather` | 1 215.5 M |
| `fft::Plan::line` | 441.7 M |
| `barnes_hut::link::halves` | 294.4 M |
| `deposit::Deposit` | 224.2 M |
| `charge::Interpolate` | 143.0 M |
| `motion::Velocity` | 118.0 M |
| `barnes_hut::step::LinkPass` | 116.7 M |
| `memset` (the per-span fill) | 83.1 M |
| `deposit::Stencils` | 68.0 M |
| `frame::Blocks` | 44.0 M |
| `motion::Position` | 37.5 M |

**The total grew 11.5 %**, 2 648.0 M to 2 953.7 M per tick. Three causes, by these counts:

- The deposit: every range reads every slot's cell, so `Deposit` plus `Stencils` is 292 M against
  the serial loop's 130 M. More workers, more reads.
- The per-span fill: 83 M of `memset`. `rep stosb` is counted once per byte, so this overstates
  its time; it is memory traffic all the same, and only `Deposit` needs it.
- `Velocity` calls its axis through a function pointer, one indirect call per node and axis.

## Wall clock

WALL_PENDING

## Gates

GATES_PENDING

## What it does not do

- **The counting sort is serial**: 66 M of the 84 M left. A parallel one needs per-worker
  histograms (8 × 2²¹ counters, 64 MiB) or a two-pass radix with an n-sized scratch.
- **`Sim::center` and the link merge are serial**, 16 M together.
- **The swap moves `x`, `y`, `vx`, `vy` between two allocations every tick.** No caller keeps an
  address across ticks today; graph-wasm's C7 would need a copy back (`motion.rs` header).
- **No persistent native pool**: `Threads` spawns per pass. The browser's pool and replicas are
  other branches (`perf-p3-wasm-threads`, `perf-p3-wasm-replicas`), which need this slice's
  fill-per-span runner when they merge.
- **The plan's P3 exit, ≥ 5× on 8 cores at 1M**, is now inside the Amdahl bound (6.68×) but is not
  claimed: the host never had eight free cores.
