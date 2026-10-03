# Perf PM simd: wasm `simd128` buys nothing, and where a 1M particle-mesh tick goes

Measured 2026-10-03 on branch `perf-pm-simd` (from `f1a2dd4`). Host: dlesieur42, i5-13600KF,
20 threads, L2 20 MiB, L3 24 MiB, 31 GB. Rust from `scripts/orch/gr`, wasm under Node in the same
image (`harness/wasm-threads.mjs`).

## 1. `+simd128` on the threaded wasm build: a null result

Question: the particle-mesh passes are plain `f64` loops; does letting LLVM vectorise them
(`-C target-feature=+simd128` added to `scripts/orch/wasm-threads.sh`) shorten the browser tick?

Parity first, on the simd artifact against the serial one:

| Check | Result |
|---|---|
| `wasm-threads.mjs hash`, BH and PM, workers {1,2,3,4,7}, seeds 0–1 | rc 0, every row `equal` |
| `wasm-threads.mjs hash --n …` | rc 0, 0 differing |
| `wasm-threads.mjs session` | rc 0, 0 differing |
| `wasm-threads.mjs hash --break` (negative control) | rc 1, 64 differing, as expected |

Speed: `wasm-threads.mjs tick --n 1000000,400000 --workers 8,12,16 --layouts particle_mesh
--ticks 7`, arms alternated base, simd, base, … three rounds (`$GM_SCRATCH/bench/pm-simd/tick.sh`).
Median ms per tick at 8 workers:

| nodes | base, 3 runs | simd, 3 runs | median base | median simd |
|---|---|---|---:|---:|
| 1 000 000 | 118.4, 119.8, 111.0 | 117.7, 121.3, 109.0 | **118.4** | **117.7** |
| 400 000 | 46.8, 44.9, 42.2 | 44.7, 44.8, 44.2 | **44.9** | **44.7** |

The difference is inside one run's spread at both sizes. **Not landed**: the build flag is reverted.
The reason is in section 2: the passes that dominate are bound by random memory reads, not by
arithmetic, so wider lanes have nothing to chew on.

## 2. `graph-cli tick --passes`: the per-pass table

New tool (`crates/graph-cli/src/bench/tick/passes.rs`): a `Runner` that wraps `Threads` and adds
each pass's wall time to its kernel's row. What no row covers is the tick's serial code between
passes (the counting sorts, the centering fold, gravity).

```
scripts/orch/gr cargo run --release -q -p graph-cli -- tick --layout particle-mesh \
  --n 1000000 --ticks 7 --workers 8 --passes      # and --workers 1
```

Native, 1 000 000 nodes, 1 549 780 edges, load average 14–17 on 20 threads:

| pass | calls/tick | 1 worker ms | 8 workers ms | speed-up |
|---|---:|---:|---:|---:|
| collide `Gather` | 1 | 147.07 | 42.16 | 3.5× |
| PM `Deposit` | 1 | 78.52 | 26.63 | 2.9× |
| PM `Interpolate` | 1 | 75.97 | 8.41 | 9.0× |
| motion `Velocity` | 6 | 57.47 | 11.74 | 4.9× |
| fft `Pass` | 4 | 40.27 | 10.70 | 3.8× |
| PM `Stencils` | 1 | 30.14 | 3.99 | 7.6× |
| link `LinkPass` | 1 | 21.83 | 6.52 | 3.3× |
| link `LinkForces` | 1 | 12.81 | 4.97 | 2.6× |
| collide `Sorted` | 1 | 10.31 | 2.64 | 3.9× |
| motion `Position` | 4 | 7.76 | 3.67 | 2.1× |
| the rest (`Shift`, `Blocks`, `Buckets`) | 5 | 8.20 | 2.73 | |
| outside any pass (serial) | — | 12.61 | 15.30 | 0.8× |
| **tick** | — | **502.97** | **139.45** | **3.6×** |

Raw: `$GM_SCRATCH/bench/pm-simd/passes-1m-w{1,8}.out`.

What it says:

- `Interpolate` and `Stencils` scale 8–9× because each reads its own positions in order and writes
  its own output. `Gather` (3.5×) and `Deposit` (2.9×) do not: both chase an index into columns
  far larger than L3 (the 1M working set is about 100 MB).
- `Deposit` recomputes each slot's CIC stencil from a random read of `x`/`y`, and `Interpolate`
  recomputes the same stencil a second time. Both are the target of the next change: compute the
  stencil once per slot, in node order, and have both passes read it.
- The serial residue is 15 ms at 8 workers, 11% of the tick, and does not shrink with workers.

## What this does not do

- `Caveat:` a kernel is named by its type, so its call sites share a row (`Velocity` is six calls).
  A pass's time includes `Threads`' spawn and join, which the wasm pool pays as a wake and a wait;
  carry the serial row to the browser, not the pass rows.
- `Caveat:` the host was loaded (14–17) for every run. The arms in section 1 alternated, so they saw
  the same load; only medians are claimed.
- Nothing here is a gate, and no motor byte changed.
