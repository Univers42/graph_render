# Perf P3 — browser threads, model (a): one shared memory, a pool of wasm threads

Measured 2026-10-02 on branch `perf-p3-wasm-threads` at `aa86fef`, host dlesieur42 (20 cores,
shared: the load average from other jobs is printed with each table), Node from `scripts/orch/gr`.
Model (b), one instance per worker, is `docs/measurements/perf-p3-wasm-replicas.md`; the user picks
between the two in `docs/decisions/browser-threads.md`.

## Design

- `graph-wasm --features threads` (`crates/graph-wasm/src/exports/threads.rs`), built by
  `scripts/orch/wasm-threads.sh` with `+atomics,+bulk-memory,+mutable-globals`, std rebuilt with
  `-Z build-std` (`RUSTC_BOOTSTRAP=1` on the pinned toolchain, image `docker/wasm-threads.Dockerfile`)
  over one shared, imported `WebAssembly.Memory`. Its own target dir: the default artifact still
  imports nothing.
- Each helper is the same module instantiated in a worker over that memory, with its own 4 MiB stack
  and TLS block (`harness/wasm-threads-helper.mjs`), parked in `gm_thread_serve` on a
  `Mutex`/`Condvar` pool (`crates/graph-wasm/src/pool.rs`); on wasm32 those are
  `memory.atomic.wait32`/`notify`.
- `PoolRunner` is graph-core's `Runner` seam: part `i` of `partition(len, workers)` goes to helper
  `i`, part 0 runs on the caller, and every part writes straight into its own span of the caller's
  column. One wake and one join per pass, no per-worker buffer, no copy back. graph-core is
  unchanged.
- `gm_run_threaded(handle, layout, workers, flags)` accepts `layout.force.barnes_hut` and
  `layout.force.particle_mesh`; flag bit 0 makes the last part write nothing (the negative control).
- Bytes equal `Serial`'s by the `StepRange` contract (disjoint spans, start-of-pass reads); the
  harness checks it against the default artifact's `gm_run`, the hash gate's own wasm arm.

## Gates

| gate | command | expect | exit |
|---|---|---|---|
| fmt | `gr cargo fmt --all --check` | 0 | 0 |
| clippy, graph-wasm native | `gr cargo clippy -p graph-wasm --all-targets -- -D warnings` | 0 | 0 |
| clippy, wasm32 default | `gr cargo clippy -p graph-wasm --target wasm32-unknown-unknown -- -D warnings` | 0 | 0 |
| clippy, wasm32 threads | the same with `--features threads` | 0 | 0 |
| pool tests | `gr cargo test -p graph-wasm pool` (4 tests, `std::thread` helpers) | 0 | 0 |
| artifact | `scripts/orch/wasm-threads.sh` (imports `env.memory` shared, exports the TLS/stack globals) | 0 | 0 |
| parity | `gr node harness/wasm-threads.mjs hash` — 8 gate seeds and n ∈ {1000, 20000}, workers {1,2,3,4,7}, both layouts: 100 cells | 0 | 0 |
| parity, negative control | the same with `--break`: the 80 cells with two parts or more must differ | 1 | 1 |
| clippy, workspace | `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0 |
| tests, workspace | `gr cargo test --workspace --no-fail-fast` | 0 | 0 |
| hash gate | `gr cargo run -q -p graph-cli -- hashgate --seeds 8` (the default artifact is unchanged) | 0 | 0 |

## Bench, the whole stage

`gr node harness/wasm-threads.mjs bench --n N --workers 1,2,4,8 --layouts …`: `gm_run_threaded` on
`gm_seed_handle(1, N)` (the gate model at the gate's reference degree), median of `--repeat`.
`equal` is the snapshot's SHA-256 against workers 1. Memory is `memory.buffer.byteLength` after the
run: one memory, shared, so it does not grow with the worker count.

100 000 nodes, repeat 3, load 12.95 → 16.98 (1 min):

| layout | workers | median ms | min–max ms | speed-up | equal | memory MiB |
|---|---:|---:|---|---:|---|---:|
| particle_mesh | 1 | 8 726.3 | 7 858.9–8 771.9 | 1.00× | true | 108 |
| particle_mesh | 2 | 7 071.0 | 6 961.2–7 071.6 | 1.23× | true | 108 |
| particle_mesh | 4 | 5 513.3 | 5 432.1–5 793.6 | 1.58× | true | 108 |
| particle_mesh | 8 | 4 061.4 | 4 052.3–4 280.8 | 2.15× | true | 108 |
| barnes_hut | 1 | 26 205.2 | 22 482.2–28 937.6 | 1.00× | true | 158 |
| barnes_hut | 2 | 20 822.4 | 18 395.3–21 604.4 | 1.26× | true | 158 |
| barnes_hut | 4 | 15 032.8 | 13 940.3–15 124.0 | 1.74× | true | 158 |
| barnes_hut | 8 | 9 569.2 | 9 512.0–9 614.9 | 2.74× | true | 158 |

1 000 000 nodes, `particle_mesh` only, repeat 1, load 21.11 → 22.03 (1 min). Barnes-Hut at 1M was
not run: its serial wasm stage is several minutes per cell.

| layout | workers | ms | speed-up | equal | memory MiB |
|---|---:|---:|---:|---|---:|
| particle_mesh | 1 | 231 627.9 | 1.00× | true | 776 |
| particle_mesh | 2 | 155 417.8 | 1.49× | true | 776 |
| particle_mesh | 4 | 125 137.1 | 1.85× | true | 776 |
| particle_mesh | 8 | 93 401.1 | 2.48× | true | 776 |

One wasm thread is 3.4× the native stage at 1M (231.6 s against 68.5 s,
`docs/measurements/perf-p2-pm.md`). The plan's P3 exit, at least 3× over serial wasm at 200k, is
**missed**: 2.15× (PM) and 2.74× (BH) at 100k, 2.48× (PM) at 1M, all at a load of 13–22 on 20 cores,
so eight threads never had eight free cores.

## Why it stops near 2.5×: the serial part of a tick

callgrind on the native PM tick at 100k (`scripts/orch/profile.sh`-style run:
`valgrind --tool=callgrind graph-cli tick --layout particle-mesh --n 100000 --ticks 3 --warm 1`,
1 397.3 M instructions, inclusive):

| part of the tick | Ir (M) | share | threaded |
|---|---:|---:|---|
| `Mesh::solve` (place, kernel refresh, deposit, two `fft2`) | 726.0 | 52.0 % | no |
| of which `Plan::fft2` (both directions, and the kernel's once) | 639.6 | 45.8 % | no |
| collide gather (`collide::Gather::step_range`) | 370.6 | 26.5 % | yes |
| link pass (`step::LinkPass::step_range`) | 164.4 | 11.8 % | yes |
| charge read (`charge::Interpolate::step_range`) | 60.4 | 4.3 % | yes |
| collide grid build (`collide::Grid::build`) | 39.9 | 2.9 % | no |

About 43 % of the instructions are in passes the `Runner` threads; the rest is serial. Amdahl caps
eight threads near `1 / (0.57 + 0.43 / 8) ≈ 1.6×` on these native proportions; wasm measured 2.15×
at 100k, so its gathers weigh more than they do natively, but the cap is the same in kind. More
threads do not fix this: the FFT has to be threaded and pruned, which is the next slice
(`perf-p3-pm-fft`): skip the all-zero rows of the forward transform's first pass and the unread rows
of the inverse's last pass, and run the row passes through the `Runner`, byte-identical.

## What it does not do

- **No browser yet.** Node only. A page needs cross-origin isolation (COOP/COEP) for a
  `SharedArrayBuffer`, and the coordinator must be a Worker: `gm_run_threaded` blocks in
  `memory.atomic.wait32`, which a page's main thread may not do.
- **A trap hangs the coordinator.** A helper that traps never finishes its part; the harness kills
  the process. A browser host must do the same (terminate the motor Worker).
- **Unstable target feature.** The build warns that `atomics` is an unstable target feature, and
  needs `RUSTC_BOOTSTRAP=1` for `-Z build-std`. Both are inherent to wasm threads on stable Rust.
- **Barnes-Hut sessions are not threaded.** `gm_run_threaded` runs a whole stage; the live
  `ForceSession` ticks still run on one thread.
- **The tree and mesh builds are serial**, as natively: threading them is plan P3's tree-build item
  and the FFT slice above.
