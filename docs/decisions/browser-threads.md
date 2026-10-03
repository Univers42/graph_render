# ADR — Browser threads: one shared memory, a pool of wasm threads (model a)

Status: **accepted** (2026-10-02). The user asked for both models to be built and measured before
one is picked (`prompts/perf-plan.md` P3), and gave full autonomy for the day, so the pick is made
here on the numbers. It is reversible: both builds are behind their own cargo feature, and the
default `graph_wasm.wasm` imports nothing in either.

## Context

The studio runs the motor in one Web Worker on one wasm instance. Plan P3 builds two ways to
spread a tick over several cores, each on graph-core's `Runner` seam, each byte-identical to
`Serial`:

- **(a)** `graph-wasm --features threads`: one shared, imported `WebAssembly.Memory`; helpers are
  the same module instantiated in workers over it, parked on a `Mutex`/`Condvar` pool, and each
  part writes straight into its span of the caller's column
  (`docs/measurements/perf-p3-wasm-threads.md`, branch `perf-p3-wasm-threads`).
- **(b)** `graph-wasm --features replicas`: one full instance per worker; after every pass each
  rank publishes its span to a `SharedArrayBuffer` and reads the whole column back
  (`docs/measurements/perf-p3-wasm-replicas.md`, branch `perf-p3-wasm-replicas`).

## The numbers

Both under Node in `scripts/orch/gr` on host dlesieur42 (20 cores, load 13–35), the whole stage,
the gate model at the gate's degree, median wall time; speed-up against the same build at one
worker. Neither measurement includes the threaded FFT or the threaded serial part
(`perf-p3-pm-fft`, `perf-p3-pm-serial`), which landed in the native tick afterwards.

| n | layout | workers | (a) speed-up | (b) speed-up | (a) memory | (b) memory |
|---:|---|---:|---:|---:|---:|---:|
| 100 000 | particle_mesh | 2 | 1.23× | 1.16× | 108 MiB | 678 MiB |
| 100 000 | particle_mesh | 4 | 1.58× | 1.34× | 108 MiB | 1 357 MiB |
| 100 000 | particle_mesh | 8 | 2.15× | **0.92×** | 108 MiB | 2 713 MiB |
| 100 000 | barnes_hut | 4 | 1.74× | 1.48× | 158 MiB | 1 357 MiB |
| 100 000 | barnes_hut | 8 | 2.74× | 1.30× | 158 MiB | 2 714 MiB |
| 1 000 000 | particle_mesh | 8 | 2.48× | not runnable | 776 MiB | ~26 GiB (3.2 GiB × 8) |

- (b) pays a copy of the whole column and two barriers **per pass**; its `gather %` reaches 24–35 %
  at four to eight ranks, and eight ranks are slower than one on particle_mesh.
- (b)'s memory is the serial build's times the rank count. At 1M one rank already holds 3.2 GiB of
  its 4 GiB address space, so model (b) cannot run 1M nodes on two ranks.
- (a) shares one memory, so its footprint does not grow with the worker count, and a pass costs
  one wake and one join.

## Decision

**Model (a).** It is faster at every cell where both ran, and it is the only one that runs at 1M.
Model (b) stays on its branch as the measured alternative and is not merged.

## Consequences

- The plan's P3 browser exit (at least 3× over serial wasm at 200k) is **not met** by either
  model on the measured builds: (a) reached 2.15× (PM) and 2.74× (BH) at 100k, on a host that never
  had eight free cores. The native serial part has since fallen from 14.1 % to 2.83 % of a 1M tick
  (`docs/measurements/perf-p3-pm-serial.md`); (a) is re-measured once it carries those slices.
- A page needs cross-origin isolation (COOP `same-origin`, COEP `require-corp`) for a
  `SharedArrayBuffer`, in `app/vite.config.ts` and in `deploy/`. Without it the studio stays on the
  serial artifact; the fallback is the default build, which imports nothing.
- The coordinator must be a Worker: `gm_run_threaded` blocks in `memory.atomic.wait32`, which a
  page's main thread may not do. The studio's motor already runs in one.
- A helper that traps never finishes its part, so the host terminates the motor Worker on a
  timeout; (a)'s report records this.
- The threads build needs `-Z build-std` with `RUSTC_BOOTSTRAP=1` on the pinned toolchain
  (`docker/wasm-threads.Dockerfile`). That is a pinned-toolchain exception, scoped to this one
  artifact; the default artifact is still built on stable.

## Update 2026-10-03: the studio uses model (a) for its live settle

The SDK loads the threads artifact through `createMotor(url, { threads })`, and the studio's motor worker
starts the helper Workers (`docs/measurements/perf-p3-browser.md`). In a cross-origin-isolated
page, eight threads tick the live settle 3.4× faster at 400k (245 → 72 ms) and 4.3× faster at
1M (690 → 162 ms). The bytes are equal to serial at every helper count (`sdk-threads` row).
The path stays dormant until a server sends COOP/COEP (perf-p3-coi).
