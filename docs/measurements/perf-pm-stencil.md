# Perf PM stencil: scale each position once, read it twice

Measured 2026-10-03 on branch `perf-pm-stencil` (from `perf-pm-simd` f620978). Host: dlesieur42,
i5-13600KF, 20 threads, 31 GB. Rust from `scripts/orch/gr`, wasm under Node in the same image.

## Why

`perf-pm-simd.md` §2 put the 1M particle-mesh deposit at 26.6 ms of a 139 ms tick (8 workers), and
showed that `Deposit` and `Interpolate` each recomputed every slot's cloud-in-cell stencil from a
random read of the `x` and `y` columns. `Deposit` walks the slots in mesh-row order, so its reads of
two 8 MB columns jump; `Interpolate` then repeats the same divisions per node.

## Change

`Stencils` (the pass that already ran in node order) now stores the frame-scaled position
`((x − origin) / h, (y − origin) / h)` per slot in `Mesh.at`. `Deposit` reads that one 16-byte
entry instead of two columns, and `Interpolate` reads the same entry in node order. The cell and
the fraction are split from it by `frame::split`, the only place left that turns a scaled position
into a stencil. A non-finite position scales to `NaN`, which `split` and `Rows::sort` skip, as they
skipped the raw position before.

| File (`crates/graph-core/src/layout/force/particle_mesh/`) | What changed |
|---|---|
| `frame.rs` | `scaled`, `cell`, `split`; the old `stencil` kept for tests only |
| `deposit.rs` | `Stencils::Out = Scaled`; `Rows::sort` and `Deposit::add` read `at` |
| `mesh.rs` | `at: Vec<Scaled>`; `field_of(k)` reads the stored position |
| `charge.rs` | `Interpolate` drops its `x`/`y` columns |
| `deposit/tests.rs` | a finite position never scales to the non-finite mark |

The arithmetic is the same divisions in the same order, done once and stored, so the bytes do not
move. The checks below show it.

## Byte identity

| Check | Result |
|---|---|
| `graph-cli snapshot --layout force.particle_mesh` at n = 333, 1000, 4096, 12345, 20000, 50000, 77777, 100000: sha256 on this tree against `perf-pm-simd` | identical at all 8 |
| `wasm-threads.mjs hash --serial <perf-pm-simd serial.wasm>` (this tree's threaded build against the base tree's serial build), BH and PM, workers {1,2,3,4,7}, seeds 0–7 | rc 0, every row `equal` |
| `wasm-threads.mjs session --serial <perf-pm-simd serial.wasm>` | rc 0, 0 differing |
| `wasm-threads.mjs hash --n 1000,20000` | rc 0 |
| `wasm-threads.mjs hash --break` (negative control) | rc 1, 64 differing, as expected |
| `graph-cli hashgate --seeds 8` | PASS |
| `cargo test -p graph-core particle_mesh` | 26 passed |

Raw: `$GM_SCRATCH/bench/pm-stencil/{xtree-simd,xtree-stencil,hash-base,session-base,hash-n,hash-break}.out`.

## Speed

```
scripts/orch/gr cargo run --release -q -p graph-cli -- tick --layout particle-mesh \
  --n 1000000 --ticks 7 --workers 8 --passes      # and --workers 1
```

Arms alternated base (`perf-pm-simd`), stencil, base, … three rounds per worker count
(`$GM_SCRATCH/bench/pm-stencil/passes.sh`); load average 11.4–12.9 on 20 threads. 1 000 000 nodes,
1 549 780 edges. Median of the three runs:

| pass | 8 workers base ms | 8 workers stencil ms | 1 worker base ms | 1 worker stencil ms |
|---|---:|---:|---:|---:|
| collide `Gather` | 33.07 | 33.48 | 165.03 | 155.58 |
| PM `Deposit` | 23.60 | **9.95** | 76.20 | **24.98** |
| PM `Interpolate` | 8.70 | **2.75** | 71.36 | **10.51** |
| PM `Stencils` | 3.21 | 2.51 | 31.52 | 10.80 |
| motion `Velocity` | 11.18 | 11.70 | 48.42 | 47.88 |
| fft `Pass` | 9.22 | 9.21 | 38.59 | 39.95 |
| link `LinkPass` | 6.39 | 6.53 | 19.23 | 19.44 |
| outside any pass (serial) | 14.04 | 16.03 | 12.11 | 14.21 |
| **tick, mean of 7** | **123.78** | **107.02** | **465.81** | **360.03** |

The bench's own per-tick median, run by run: 8 workers base 120.04 / 110.04 / 129.24, stencil
106.57 / 107.33 / 108.44; 1 worker base 403.85 / 654.94 / 443.94, stencil 339.85 / 325.64 / 286.89.

**At 1M: 8 workers 120.0 → 107.3 ms per tick (−11%), 1 worker 443.9 → 325.6 ms (−27%).** Deposit
and Interpolate together went from 32.3 to 12.7 ms at 8 workers. Every untouched pass moved by
less than its own run-to-run spread.

Memory: `Mesh.at` holds 16 bytes per slot where the stencil cell held 4, so +12 MB at 1M.

Raw: `$GM_SCRATCH/bench/pm-stencil/passes-{simd,stencil}-w{8,1}-r{1,2,3}.out`.

## What this does not do

- `Caveat:` the host was loaded (11–13) by landers and OpenCode jobs for every run. The arms
  alternated, so they saw the same load; one-worker runs spread by up to 60% and only medians are
  claimed.
- `Caveat:` the serial row grew by about 2 ms. `Rows::sort` now reads a 16-byte entry per slot
  where it read a 4-byte cell; inside the noise of one run, not proven either way.
- It does not touch collide, which is now the largest pass (`Gather` 33 ms, 31% of the tick).
- No browser number yet: the wasm tick bench (`wasm-threads.mjs tick`) has not been run on this
  tree.
