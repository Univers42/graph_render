# Perf P3 — the live session ticks on the wasm thread pool

Measured 2026-10-03 on branch `perf-p3-session-threads` (perf-pm-live `3b25275` merged with
perf-p3-wasm-threads `ca243df`), host dlesieur42, 20 cores, shared with a running gate (the load
average is printed with the table). Node from `scripts/orch/gr`.

Why: the studio's live session ticks serially, so at 400 000 nodes the user watches one tick about
every 235 ms, and one every 700 ms at 1M (`docs/measurements/perf-pm-live.md`). The pool from
perf-p3-wasm-threads ran only whole stages (`gm_run_threaded`). The live tick did not use it.

## Design

- `crate::session::tick_with(id, ticks, runner, workers)` is `tick` over
  `ForceSession::step_with`. It covers both engines, Barnes-Hut and the particle mesh.
- `gm_force_session_tick_threaded(session, ticks, workers, flags)` is its export in the `threads`
  build. Its status word is `gm_force_session_tick`'s. Flag bit 0 is the pool's negative control:
  the last part writes nothing.
- `Pool::broadcast` waits out its helpers in a drop guard (`Joining`). Before this, a panic in
  the caller's own part ended the job's borrow while helpers still ran it. That was a
  use-after-free, and natively it showed as a SIGSEGV in the session's negative control (a
  debug assertion in the particle mesh's collide gather). wasm32 aborts on a panic, so the
  shipped artifact was never exposed.

## Gates

| gate | command | expect | exit |
|---|---|---|---|
| graph-wasm unit tests | `gr cargo test -p graph-wasm --lib`: `session::tests::threaded` runs both engines at workers {1,2,3,4,7} against `tick`, and Barnes-Hut under `skip_last` | 0 | 0 (144 passed) |
| threads artifact | `scripts/orch/wasm-threads.sh` | 0 | 0 |
| parity | `gr node harness/wasm-threads.mjs session`: 8 gate seeds, both engines, workers {1,2,3,4,7}, 30 ticks in calls of 10; x, y and alpha bytes against the default artifact's `gm_force_session_tick` | 0 | 0 (0 of 80 differ) |
| parity, negative control | the same with `--break` | 1 | 1 (64 of 80 differ: every cell with two parts or more) |
| fmt, clippy | `gr cargo fmt --all --check`; `gr cargo clippy --workspace --all-targets -- -D warnings`; `gr cargo clippy -p graph-wasm --features threads --lib -- -D warnings` | 0 | 0 |

The native negative control is Barnes-Hut only. In a debug build, a zeroed particle-mesh span
trips the collide gather's slot assertion (`collide/gather.rs`) before its bytes can differ. The
mesh's negative control is the release wasm32 row above.

## Bench, one live tick

`gr node harness/wasm-threads.mjs tick --n 400000,1000000 --workers 1,2,4,8 --ticks 5`. Each cell
is a fresh particle-mesh session on `gm_seed_handle(1, n)`, one warm-up tick, then the median of 5
single-tick calls. Load 3.75 → 4.13 (1 min).

| n | workers | median ms/tick | min ms | max ms | speed-up |
|---:|---:|---:|---:|---:|---:|
| 400 000 | 1 | 148.0 | 138.1 | 158.9 | 1.00× |
| 400 000 | 2 | 91.2 | 85.6 | 97.4 | 1.62× |
| 400 000 | 4 | 58.1 | 51.2 | 63.8 | 2.55× |
| 400 000 | 8 | 50.1 | 45.2 | 53.2 | 2.95× |
| 1 000 000 | 1 | 396.7 | 364.9 | 435.1 | 1.00× |
| 1 000 000 | 2 | 238.1 | 227.8 | 258.3 | 1.67× |
| 1 000 000 | 4 | 146.2 | 134.2 | 183.1 | 2.71× |
| 1 000 000 | 8 | 125.3 | 107.4 | 131.3 | 3.17× |

The serial tick here, 148 ms at 400k, is below the 235 ms the browser showed. The browser's figure
is a frame gap, taken under SwiftShader with the draw sharing the cores.

## What it does not do

- **No browser yet.** The studio still loads the default artifact. Using the pool in a page needs
  cross-origin isolation (perf-p3-coi) and a threaded loader in the motor worker
  (perf-p3-browser).
- **It stops near 3× at eight workers.** Going from four workers to eight gains only 1.16× at 400k.
  This run did not measure where the remaining serial time goes. Caveat: one run per cell on a
  shared host. The gate running alongside held about four cores.
- The plan's browser target, a 1M tick ≤ 100 ms, is not met: it is 125 ms here at eight workers.
