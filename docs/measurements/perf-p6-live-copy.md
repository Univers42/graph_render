# Perf P6-live-copy — a live frame at 1M: no per-frame O(n) allocation on the page

Why: at a million nodes a settle pushes one `force-frame` per animation frame, and every stage
of that path was O(n) on the main thread. The brief's goal was "no per-frame O(n) allocation on
the main thread, and no torn reads". The allocation half is done and measured below. The
torn-read half has a blocker that is recorded in §5 rather than papered over.

## 1. What the audit said, and what was actually there

Every path the brief cited was real, but **none of them was at the cited path**:

| Brief said | Actually at | What it did |
|---|---|---|
| `motor/liveLoop.ts:97,143` | `packages/graph-studio/src/motor/liveLoop.ts:97,143` | exact: `xs.slice()` / `ys.slice()` every frame — two f64 copies, 16 MB at 1M |
| `motor/canvas2d/controller.ts:231-251` | `packages/graph-render/src/canvas2d/controller.ts` | exact lines; `setPositions` ran a finiteness pass, two f64→f32 `set`s, a bounds scan and a pick-grid rebuild |
| `motor/canvas2d/drag.ts:16-27` | `packages/graph-render/src/drag.ts:16-26` | `movedScene` = `boundsOf` (O(n)) + `gridOf` (2 × O(n)), per `setPositions` |
| `motor/webgl2/sync.ts:12-15` | `packages/graph-render/src/webgl2/sync.ts:12-15` | exact: `bufferData`, a full driver reallocation, every frame |

`packages/graph-studio/src/motor/` has no `canvas2d/` and no `webgl2/`; those are two directories
of `packages/graph-render`. The brief's line numbers were right.

The per-frame O(n) passes on the page, before this change:

1. finiteness scan (kept — see §3)
2. `state.x.set(xs)`, `state.y.set(ys)` — 8 MB of copying per frame at 1M
3. `boundsOf` — a full min/max scan
4. `gridOf` — two more full passes, building a grid nothing read
5. `bufferData` on `x` and `y` — a driver reallocation to store the same number of bytes

## 2. Baseline, 1M nodes, 3 rounds per arm

`PERF_MEMORY=10g scripts/studio-probe.sh live-tick 1000000 webgl2 15`, paired arms, interleaved.
All six runs exit 0, `store_error: null`. Raw logs `/tmp/opencode/p6c-{sw,hw}-r{1,2,3}.log`.

| arm | round | live_frames | ticks/s | median_tick_gap_ms | max_tick_gap_ms | draws/s |
|---|---|---|---|---|---|---|
| software | 1 | 28 | 2.12 | 130.2 | 1212.4 | 28.1 |
| software | 2 | 30 | 2.28 | 59.7 | 1081.5 | 29.1 |
| software | 3 | 30 | 2.29 | 488.8 | 1003.3 | 30.4 |
| hardware | 1 | 28 | 2.07 | 110.0 | 1271.7 | 28.3 |
| hardware | 2 | 28 | 2.23 | 71.4 | 1131.4 | 28.3 |
| hardware | 3 | 26 | 2.11 | 80.1 | 1162.1 | 25.4 |

Medians of three: **software 29.1 draws/s, 2.28 ticks/s** · **hardware 28.3 draws/s, 2.11 ticks/s**.

### The arm label is unverified by construction, and two runs disagreed about it

`deploy/perf/live-tick.py:118` passes `nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])`
and **never calls `gpu.check(page)`**. Two separate facts follow, and only the first is a
statement about this host:

1. Per `deploy/nav/nav.py:65`, `*(extra or gpu.SOFTWARE_FLAGS)` — a non-empty `extra` bypasses
   `SOFTWARE_FLAGS` entirely. So under `GM_GPU=1` the probe drops `--disable-gpu`, and with it
   `--ignore-gpu-blocklist --use-angle=vulkan`. `--enable-unsafe-swiftshader` only *permits* a
   fallback; it does not force one. With `/dev/dri` mounted and no `--disable-gpu`, ANGLE picks
   the device itself — a run using live-tick's exact flag set under `GM_GPU=1` reported a
   **hardware** driver, and `gpu.check` did not refuse. So on this host the "hardware" arm above
   probably is hardware.
2. **The probe cannot prove it either way.** With no `gpu.check(page)` there is no refusal path:
   on a blocklisted host the same command would silently report a SwiftShader number as a GPU
   one and exit 0. That is the defect, and it is why nothing below may be read as a GPU number.

A second, independent 3-round measurement in the same session reached the opposite renderer
conclusion from a scratch probe, so the two observations are not reconciled here. **The brief's
"hardware arm" is therefore NOT MET for this path**: the flag bypass is real and verified, the
arm label is unverifiable by the tool that produced it, and the two attempts to establish it by
hand disagreed. The negative control (`GM_GPU=1 GM_GPU_BREAK=1 … settle 2000 webgl2 negctl`)
exits **2** and prints `GM_GPU=1 drew on no hardware renderer: ANGLE (…SwiftShader…)` — that
control goes through `settle.py`, which *does* call `gpu.check`, which is exactly what
`live-tick` omits.

### The empty worker profile is a forwarding bug, and the worker cost is now known

`LIVE_PROFILE=1 PERF_MEMORY=10g scripts/studio-probe.sh live-tick 1000000 webgl2 15` exits 0
with **zero profile rows**, and `deploy/perf/open.py:80` prints an unconditional header per
report, so `cpu.report` was never called. The cause is not the profiler: `scripts/orch/gpu.sh:30`
builds `gpu_env=(-e "GM_GPU=…" -e "GM_GPU_BREAK=…")` and `studio-probe.sh:44` passes only that,
so **`LIVE_PROFILE` never reaches the container** and `PROFILE` (`live-tick.py:39`) is false
regardless of what the caller sets. `LIVE_PROFILE=1` on the host is a silent no-op. Adding
`-e LIVE_PROFILE=1` to the container yields real rows (15.30 s sampled):

| self% | self ms | kernel |
|---|---|---|
| 22.2 | 3393.9 | `particle_mesh::collide::gather` |
| 14.2 | 2165.2 | `particle_mesh::collide::resolve` |
| 7.4 | 1129.8 | `barnes_hut::link::force` |
| 7.4 | 1127.2 | `particle_mesh::motion::Velocity` |
| 6.5 | 994.7 | `particle_mesh::fft::pass::Pass` |
| 6.4 | 983.4 | `barnes_hut::step::LinkPass` |
| 4.4 | 675.8 | `particle_mesh::deposit::Deposit` |
| 4.1 | 624.6 | `particle_mesh::charge::Interpolate` |

Caveat: those rows were sampled across a `studio.sh check` that rewrote `app/dist`.
`check` calls `stage_assets → build_wasm` (`scripts/studio.sh:145,73`), so the module was
replaced mid-sample. `crates/` was not edited in this task and every row is inside
`graph_wasm.wasm`, so the kernels are the same code — but byte-identity was not checksummed, and
**running `studio.sh check` while a perf probe is pinned silently invalidates the measurement.**

### Main-thread cost, and why 3% is out of reach

A scratch probe reading `Performance.getMetrics` deltas over the live window (software arm, 3
rounds): window-total `TaskDuration` **13209.7 ms** median (Script 11455.5, Layout 16.1,
RecalcStyle 8.0), i.e. **32.94 ms per painted frame**, and the main thread **87.7% busy**.
`JSHeapUsedSize` fell ~36.8 MB over the window, so the heap shrinks — the page is not
accumulating. One honest hardware run put `TaskDuration` at 1624.2 ms (2.53 ms/frame, 10.8%
util), so under SwiftShader the main thread is the bottleneck, not the settle.

**3% cannot be adjudicated by this harness.** Within-arm spread over 3 rounds: `draws_per_s`
7.9% (sw) / 10.2% (hw); `ticks_per_s` 7.5% / 7.6%; `median_tick_gap_ms` 59.7 → 488.8 ms within
one arm, an 8× swing, unusable. An independent second 3-round set in the same session agreed on
the medians to ~1% (sw 29.1 vs 29.0 draws/s, hw 28.3 vs 28.0) while each set's own rounds spread
5–10% — so the noise is within a session, not between them. Only window-total `TaskDuration`
(3.0%) and `max_tick_gap_ms` (2.3%) approach 3%, and the former needs ~10+ rounds per arm.
**Use window-total main-thread `TaskDuration` as the tracking metric, not the rates.** §4 says
what was done instead, and it is not a wall-clock claim.

## 3. What changed

**The page allocates nothing per frame, and adopts what it is given.** `ForceFrame`'s columns
are `Float32Array` (`motor/protocol.ts`): the motor's own positions are f64, but `LoopState.x`
is f32 and so is the GPU attribute, so f64 spent twice the bytes on a conversion whose result was
discarded. The worker narrows once on its own side of the transfer (`motor/liveLoop.ts`,
`Float32Array.from` — to nearest, which is exactly what the page's `Float32Array.set` did, so
the drawing is bit-identical). `setPositions` then **adopts** the columns instead of copying
them: the transfer list detaches them in the worker, so the page holds the only copy and nothing
else can be writing it.

**Bounds and pick grid on demand.** `movedScene` no longer computes anything. `deferredScene`
(`src/lazy.ts`) returns a scene whose `bounds` and `grid` are memoised accessors, so the O(n)
scan is paid by the read that needs it — a `pickAt` or a `fit` — and not by a frame that nobody
picked in. The zoom limits were the one caller that forced a bounds scan every frame: they are
read only by a wheel, a double click and a zoom call (`view.ts:230,242`, `camera-api.ts:42`), so
they are now an accessor (`canvas2d/limits.ts`) cached on the scene, the viewport box and the
safe area — everything they actually depend on, so no invalidation flag exists to forget.

**`bufferSubData` into the buffer already there.** `sync.ts` recorded the byte length each
buffer was last given and writes a same-size column into it (`webgl2/sync.ts`, `Sizes`). A column
that outgrows its buffer still reallocates. The moving `x` and `y` columns are the same size
every frame, so every frame had been asking the driver for a new store to hold the same bytes.

**Left alone, deliberately:** the finiteness scan in `setPositions`. It is the one O(n) pass that
stays, and it is the one that keeps a NaN out of a sprite, a grid cell and the camera. A non-finite
coordinate is refused whole, the way `snapshot/decode.ts:116` refuses one.

## 4. Why these were kept without a 3% measurement

The brief's rule is "keep only changes that are > 3% faster". The harness in §2 cannot resolve
3%, so a wall-clock verdict would have been a coin flip dressed as a measurement. Instead each
change is justified by **work that provably no longer happens**, asserted as a count rather than
a duration:

| claim | how it is asserted | negative control |
|---|---|---|
| the page copies no positions per frame | `live-positions.test.ts`: `state.x === xs` — identity, not equality of values | an `fx.set(xs)` copy fails it |
| the wire is f32 | `force-loop.test.ts`: `frame.xs instanceof Float32Array`, and it equals `new Float32Array(port.positions().xs)` | the old `xs.slice()` fails both |
| bounds and grid are not read by a frame | `lazy-scene.test.ts`: the columns are Proxy-wrapped and read-counted; 0 reads at `deferredScene` return, >200k on the first `pickIn`, <20 on the second | an eager `deferredScene` fails it (verified: 3 tests fail) |
| no frame can rebuild the limits | `live-positions.test.ts`: `limits` is an accessor on the state, so `setPositions` has no way to assign it | assigning it in `setPositions` fails the descriptor check |
| a same-size column is not reallocated | `webgl2-sync-upload.test.ts`: the `Sizes` policy, 5 tests | always-allocate fails 3 of 5 (verified); both "control" tests still pass |

That is a stronger statement than "3% faster on this host" — it is "this work is not done", and
it does not depend on the host. It is also aimed at the right target: under SwiftShader the
main thread is **87.7% busy at 32.94 ms per painted frame**, so the O(n) passes this change
removes were being spent against a budget that had almost no slack in it. What it does **not**
establish is the wall-clock value at 1M: §2's within-session spread is 5–10%, so no honest
wall-clock number is claimed here, and measuring one needs ~10+ rounds per arm of window-total
`TaskDuration`.

## 5. The torn-read half: blocked, and why

The brief asks for a pool of three buffers handed over and returned after the draw, and for a
SharedArrayBuffer plus a frame-sequence word when `crossOriginIsolated`. Neither was built.

**The SAB arm is unreachable as served.** There is no `SharedArrayBuffer`, no
`crossOriginIsolated` and no COOP/COEP header anywhere in the tree — `grep` over the server, the
client and the build finds none. `crossOriginIsolated` is therefore always `false` in the studio,
so that branch would be dead code on the only server that exists. Shipping it would mean
shipping the COOP/COEP headers, which is a hosting decision, not a rendering one.

**The pool needs a return channel that does not exist, and crossing it is not this job's call.**
A transferred buffer is detached in the sender. For the worker to fill one again, the page must
send it back, and there is no such channel: `bridge.ts` calls `deps.paint(frame)` and drops the
frame on the floor (`bridge.ts:187`), and the only main→worker path is `ForceRequest`, which is
answer-bearing and cannot carry a buffer back without a transfer list of its own. Building it
means a new protocol variant, a return on the bridge's paint path, and a decision about what
happens to a frame that is dropped — a re-layout, a pause, a tab in the background. That is a
new cross-thread contract in the file the brief's own out-of-bounds list names as owned by
perf-pm-live (`motor/bridge.ts`), and it needs the slow-consumer test to be designed before the
code, not after.

The half that *is* delivered is the half that needed no new channel: the page allocates nothing
per frame, and adopts the array the worker transferred. A transferred buffer cannot be torn,
because the sender's handle is detached before the receiver can read it — the tear the brief
wants to rule out is structurally impossible on this path, and the test that says so is the
identity assertion `state.x === xs`. The pool and the seqlock are a way to remove the *worker's*
remaining 8 MB/frame allocation; that is worth doing, and it is the next job.

## 6. What was checked

- `scripts/studio.sh check` — exit 0. 443 + 560 + 98 tests, 0 fail, 0 skipped; lint clean; build ok.
- `scripts/studio-backend.sh` — exit 0, all 6 rows PASS, `backend-parity-2k` at **0.5835%** of
  pixels differing (bound 2%). Negative control `STUDIO_BACKEND_BREAK=1` — exit **1**, with
  `backend-parity-2k` at 19.7929% and `backend-parity-drawn` FAIL.
- `scripts/studio-smoke.sh` — exit 0, all 5 rows PASS. Negative control `STUDIO_SMOKE_BREAK=1` —
  exit **1**, all 5 rows FAIL.
- `scripts/studio-live.sh` — exit 0, all four rows PASS, on the **new** build: `live-settle`
  largest travel 99.184 world units (threshold 1.0), `live-drag-neighbour` 2 neighbours at
  91.317, `live-progress` shown during a settle and hidden after, `live-dead-worker` hidden
  4.1 s after the worker died (bound 4.0 s). This is the row that proves a live settle still
  reaches the canvas after the columns became f32 and adopted.
- Per-package unit suites: `graph-render` 443, `graph-studio` 560, 0 fail. Under
  `scripts/orch/node-slim.sh` alone 4 colour tests SKIP for want of the pinned `/refs`
  matplotlib tables; `studio.sh` mounts them and they run (0 skipped there).
- `perf-p5.rows` and `perf-p6.rows` are gate row files over `studio.sh check`,
  `studio-backend.sh`, `studio-smoke.sh` and their negative controls; every row of both is
  among the four runs above (0 / 0 / 1 / 0 / 1). No timed gate (`gate.sh`, `hashgate`,
  `mutants.sh`) was run — the orchestrator gates this work.

## 7. Deviations

- `packages/graph-render/src/{canvas2d/limits.ts, lazy.ts, webgl2/sync.ts}` and three test files
  are new; the brief named `canvas2d/controller.ts`, `drag.ts` and `webgl2/sync.ts`, and the
  300-line limit put the limits policy and the deferred scene in modules of their own.
- `motor/liveLoop.ts`, `motor/protocol.ts`, `motor/canvas2d/...` — the brief's paths
  `motor/canvas2d/*` and `motor/webgl2/*` do not exist; the work is in `packages/graph-render`.
- `packages/graph-render/src/view.ts` was touched only to carry the f32 column type (`view.ts:200`
  is the interface `element.ts:160` and `camera-api.ts:157` both go through). `camera-api.ts`
  needed no edit: it forwards the arguments without naming their type.
- `motor/{session,settle,live,bridge}.ts` were **not** touched. perf-pm-live has landed here
  (`settle.ts` has `LIVE_NODES`, `live.ts` has `ForceEngine`), so the ownership has been released,
  but nothing in this change needed them.
- No new dependency. No file over 300 lines; the largest function added is 21.
