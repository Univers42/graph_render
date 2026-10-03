# Perf P2-PM — `layout.force.particle_mesh`, the many-body force on a mesh

Measured 2026-10-02 on branch `perf-p2-pm` at `7d922da` (the code; this page and the ceiling
came after), host dlesieur42 (20 cores, shared: load average 5.4–13.4 from other jobs, printed
next to each run), image `ge-rust`, release build, one thread.

At 1M nodes, `layout.force.barnes_hut` spends 95.5 % of a tick's instructions in its charge and
collide passes, and 79.9 % in their two quadtree walks (`docs/measurements/perf-p2.md`, the
callgrind table). This layout keeps Barnes-Hut's link, center
and integrate code as it is and replaces the two walks with grid methods:

- **Charge.** A cloud-in-cell deposit on a `P × P` mesh, one FFT convolution with d3's
  `manyBody` law, then a read-back with the same weights. `P = clamp(next_pow2(ceil(√n)), 128, 1024)`.
- **Collide.** A counting sort into a hashed cell list one diameter wide, with nine neighbour
  buckets per node.

A tick is `O(n + P² log P)`, where Barnes-Hut's is `O(n log n)` with a large constant in the walks.
The bytes differ from Barnes-Hut's, so this is a new id with its own hash-gate entry, not a tier
of the old one. The target came from `docs/measurements/perf-p1-baseline.md:130`: one 1M tick in
at most 150 ms on one thread.

## One tick, native, single thread

`graph-cli tick --layout <l> --n N --ticks 3 --warm 2`, the median tick with its min and max, at
alpha 0.7339. Both layouts ran one after the other, under the same load.

| n | m | particle mesh | Barnes-Hut | speed-up | load (1 min) |
|---:|---:|---:|---:|---:|---|
| 100 000 | 154 978 | 31.58 ms (30.49–33.56) | 114.95 ms (112.07–118.24) | 3.6× | 6.1–7.3 |
| 1 000 000 | 1 549 780 | **361.13 ms** (344.59–373.37) | 1 635.44 ms (1 540.13–1 741.03) | 4.5× | 6.8–7.3 |

From 100k to 1M a tick grows 11.4× for 10× the nodes; Barnes-Hut grows 14.2×. The P1 target of
150 ms per 1M tick is **missed**: the result is 2.4× over it.

## The whole stage, 112 ticks

`graph-cli bench --layout … --n N --past-ceiling`. `stress-1` is the bench's own stress figure
(lower is better).

| n | m | particle mesh | stress-1 | Barnes-Hut | stress-1 | runs | load (1 min, start → end) |
|---:|---:|---:|---:|---:|---:|---|---|
| 100 000 | 154 978 | 3 595.13 ms | 0.4177 | 13 735.36 ms | 0.4205 | 1 | 5.44 → 6.06 |
| 500 000 | 775 457 | 32 729.72 ms | 0.4308 | not run | | median of 3 | 7.30 → 7.63 |
| 1 000 000 | 1 549 929 | 71 202.68 ms | 0.4205 | 277 862.02 ms | 0.4168 | 1 | 6.06 → 13.35 |
| 1 000 000 | 1 549 929 | **68 483.65 ms** | 0.4205 | not run | | median of 3 | 7.30 → 7.63 |

**Unexplained:** at 1M the stage takes 68.5 s, but 112 ticks at the measured 361 ms would take
40.4 s. The tick column samples only alpha 0.73, after two warm ticks; the stage runs every
alpha down to the floor. That gap was not profiled. A per-tick trace over the whole stage
(`tick --ticks 112 --warm 0`) is the next measurement.

## Quality: the stress gate against d3-force

`graph-cli stress --oracle d3 --layout layout.force.particle_mesh` (8 seeds, the default)
compares the d3-force@3.0.0 stress correlation with ours. Floor: margin −0.05, the same one
Barnes-Hut is held to. It exited 0 and wrote `target/gates/stress-pm.json`.

| seed | r(d3) | r(ours) | margin |
|---:|---:|---:|---:|
| 0 | no correlation (a component with no pair) | | |
| 1 | 0.96151 | 0.98076 | +0.01925 |
| 2 | −0.48757 | −0.18351 | +0.30406 |
| 3 | 0.36611 | 0.50307 | +0.13696 |
| 4 | 0.14230 | 0.15146 | +0.00916 |
| 5 | 0.11988 | 0.21103 | +0.09114 |
| 6 | 0.05537 | 0.16062 | +0.10526 |
| 7 | 0.23602 | 0.22496 | −0.01106 |

The median margin is **+0.09114** over 7 correlated cases. The worst is −0.01106, inside the
floor. Load at the run was 6–7.

## The ceiling

`PM_CEILING = 800 000` (`crates/graph-core/src/registry/force.rs`). The ceiling uses the 60-second
budget that `FORCE_CEILING` uses. 1 000 000 nodes is past that budget, at 68.5 s and 71.2 s. The
power law through the two median points is 500 000 at 32.7 s and 1 000 000 at 68.5 s, with an
exponent of 1.07. Solved for 60 s it gives 883 000, which is rounded down to 800 000. That is an
interpolation between two runs, not a run at 800 000.

## Reproduce

```sh
scripts/orch/gr cargo test -p graph-core particle_mesh
scripts/orch/gr cargo run -q --release -p graph-cli -- tick --layout particle-mesh --n 1000000 --ticks 3 --warm 2
scripts/orch/gr cargo run -q --release -p graph-cli -- tick --layout barnes-hut --n 1000000 --ticks 3 --warm 2
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.particle_mesh \
  --n 500000,1000000 --repeat 3 --past-ceiling
scripts/orch/gr cargo run -q -p graph-cli -- stress --oracle d3 --layout layout.force.particle_mesh
```

The raw outputs are in `$GM_SCRATCH/logs/perf-p2-pm/{bench,bench2,tick,stress}.out`.

## What it does not do

- **It is not Barnes-Hut's picture.** Below about two mesh cells the charge is smoothed, so two
  close nodes repel less. Link and collide own that range. The stress gate grades the result as
  not worse than d3; it never claims it is equal to it.
- **It caps the mesh at 1024 per side.** Past about 1M nodes, a cell holds more than one node on
  average, and more of the charge falls in the smoothed range.
- **It is single-threaded.** The deposit, the read-back and collide are per-node passes, and they
  are the next rung for the thread executor (`docs/measurements/perf-p3-split.md`). The two FFTs
  are not per-node passes.
- **It is not a live session.** `ParticleMeshRun` runs the frozen stage. The studio's live loop
  stays on Barnes-Hut.
- **The 1M rows come from a shared host.** One bench pair is a single run (load up to 13.4). The
  other is a median of 3 (load about 7.3). The two 1M numbers agree within 4 %.
