# Perf P3 browser: the studio ticks its live settle on the wasm thread pool

Measured 2026-10-03 on branch `perf-p3-browser` (from perf-p3-session-threads `903fb67`). Host:
dlesieur42, 20 cores, shared with a landing gate. Browser: `gm-chromium` (Chrome 154, SwiftShader).

Why: the live settle ticked serially in the motor worker. At 400 000 nodes that was one tick about
every 245 ms, and at 1M one every 690 ms (`docs/measurements/perf-pm-live.md`). The pool's live
tick (`gm_force_session_tick_threaded`, `docs/measurements/perf-p3-session-threads.md`) had no
caller in a page.

## Design

| Piece | Where | What it does |
|---|---|---|
| threaded loader | `crates/graph-sdk-js/src/threads.ts` | `createMotor(url, { threads: { helpers, spawn } })` compiles the threads artifact over one shared memory. It takes every helper's stack and TLS block before it spawns any helper, waits up to 5 s for them to join, and swaps `gm_force_session_tick` for the threaded export. Every other call stays serial. |
| helper body | `serveHelper` (same file), `packages/graph-studio/src/motor/helper.ts` | instantiates the module over the shared memory, sets its stack pointer and TLS, and parks in `gm_thread_serve` |
| how many | `packages/graph-studio/src/motor/threads.ts` | 1 unless the page is cross-origin isolated; otherwise `?threads=N`, or cores − 1 by default, clamped to 1..8 |
| fallback | `motor/worker.ts` `motorFrom` | if the threads artifact fails to load, the serial `graph_wasm.wasm` loads |
| staging | `scripts/studio.sh` | builds `scripts/orch/wasm-threads.sh` and copies its artifact to `app/public/graph_wasm_threads.wasm` |

## Gates

Rows: `scripts/orch/rows/perf-p3-browser.rows`, run through `scripts/orch/gate.sh`.

| Row | Expect | What it shows |
|---|---|---|
| `sdk-threads` | 0 | Through the SDK's published surface: x, y and alpha are byte-equal between the threaded and the serial motor, at 20 000 nodes, both engines, helpers {1,3,6}, 3 × 10 ticks (0 of 6 cells differ) |
| `negctl-sdk-threads` | exit 1 | `--break` (pool flag bit 0: the last part writes nothing) makes every cell differ (6 of 6). This also proves the helpers joined. |
| `sdk-typecheck`, `sdk-smoke` | 0 | the SDK still type-checks, and its serial surface is unchanged (321 ok) |
| `studio-check` | 0 | tsc, render 405/405, studio 551/551 (3 new `threads.test.ts` cases), ui 90/90, eslint, vite build. The build emits `helper-<hash>.js` next to the worker chunk. |
| `smoke`, `negctl-smoke` | 0, non-zero | the page loads clean over the serial module; the negative control goes red |

## Browser, live settle

`deploy/perf/live-tick.py N webgl2 S` with `LIVE_THREADS=N`. The page was served with COOP
`same-origin` and COEP `require-corp`, using a scratch wrapper around `nav.serve`. Those headers are
not served by any committed server yet; that is perf-p3-coi. One run per row.

A frame is not a tick (`perf-pm-live.md`). The long mode of the frame gaps is the tick, and the
short mode is the frame the loop posts without stepping.

| n | `?threads=` | isolated | helpers started | tick (long mode, deciles 6–9) | live frames | alpha at end | draws/s |
|---:|---:|---|---:|---|---:|---:|---:|
| 400 000 | 1 | yes | 0 | 238–252 ms | 114 in 15 s | 0.029 | 45.4 |
| 400 000 | 8 | yes | 7 | 69–76 ms | 223 in 15 s | 0.001 | 54.4 |
| 1 000 000 | 1 | yes | 0 | 657–758 ms | 66 in 25 s | 0.130 | 42.8 |
| 1 000 000 | 8 | yes | 7 | 158–170 ms | 223 in 25 s | 0.001 | 46.5 |

- At 400k, eight threads make the tick 3.4× faster. At 1M they make it 4.3× faster.
- The 1M graph reaches the settle's stop (alpha 0.001) within 25 s. Serially it was at 0.13.
- The browser speed-up beats Node's 2.95× and 3.17× (`perf-p3-session-threads.md`). One likely
  reason is that the serial tick in the browser also pays the draw's share of the cores. This run
  did not separate the two.
- The short mode at 1M is about 40 ms. That is the cost of a frame that does not step: the
  positions are copied and posted (perf-p6-live-copy).

## What it does not do

- **It is dormant in every committed server.** `scripts/studio.sh`, `vite preview` and `deploy/`
  do not send COOP/COEP yet. Without them `crossOriginIsolated` is false, the studio loads the
  serial module, and helpers is 0. perf-p3-coi adds the headers.
- The 1M tick is 162 ms. That misses the plan's browser target of ≤ 100 ms.
- A threads artifact that fails to load falls back to the serial one with no message to the page.
  The probe's `helpers` count is the evidence.
- Caveat: one run per row, under SwiftShader, on a host whose landing gate held about four cores.
  The ratios are far outside run-to-run noise, but the absolute times move with the load.
