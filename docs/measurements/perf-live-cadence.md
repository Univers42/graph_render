# Perf live cadence: one motor step every frame, paced from the frame's start

Measured 2026-10-03 on branch `perf-live-cadence` (from `3c6c283`). Host: dlesieur42, i5-13600KF,
20 cores, 31 GB. Browser from `scripts/studio-probe.sh` (gm-chromium, WebGL2 on SwiftShader).

Why: at 400k nodes the live settle took about 10.6 s for the motor's 112 ticks, while one tick cost
about 60 ms. The worker loop (`packages/graph-studio/src/motor/liveLoop.ts`) had two rules that
together halved the simulation rate:

- after a tick longer than 8 ms, the next frame re-published the positions without stepping;
- every frame waited a fixed 16 ms after the previous one ended.

So a 60 ms tick was followed by a 16 ms wait, a frame with no step, another 16 ms wait, then the next
tick: two frames per step. The probe shows it directly: the before arm draws 223 frames to reach
alpha 0.001, the after arm 112, which is the motor's own tick count (`ForceParams::TICKS`).

## Change

| Piece | Where | What changed |
|---|---|---|
| the loop | `packages/graph-studio/src/motor/liveLoop.ts` | every frame steps; the next frame is scheduled `max(0, period − elapsed)` after this one started, so a slow tick is followed at once and a fast one keeps a 16 ms period |
| the scheduler | `packages/graph-studio/src/motor/worker.ts` | `pacedFrame`: a delay above 0 is a `setTimeout`, a delay of 0 is a `MessageChannel` post (a 0 ms `setTimeout` is clamped to 4 ms after a few nests) |
| tests | `packages/graph-studio/tests/force-loop.test.ts` | a slow motor schedules `[16, 0, 0]`, a fast one `[16, 15, 15, 15, 15, 15]` |
| probe | `scripts/studio-probe.sh` | forwards `LIVE_PROFILE` and `LIVE_THREADS` into the container |

The motor is untouched: same ticks, same bytes. Only the rate at which the worker asks for them moved.

## Bench

```
scripts/studio.sh build                         # once per arm, copied to dist-before / dist-after
PERF_MEMORY=10g scripts/studio-probe.sh live-tick 400000  webgl2 20
PERF_MEMORY=10g scripts/studio-probe.sh live-tick 1000000 webgl2 20
```

One probe at a time, arms alternated (before, after, before, …), three runs per arm per size, a
20 s window after the first frame. The arm is swapped by copying its `app/dist`, and the script
refuses to run when the served worker is not the arm it asked for (the `after` worker is the only one
holding `MessageChannel`). Layout `layout.forceatlas2.barnes_hut` (the studio's default; it runs the
particle-mesh tick at this size), 7 helper threads, cross-origin isolated.

`ticks_per_s` in the probe output counts **frames**. In the before arm a step takes two frames, so its
steps/s is half the probe's figure; in the after arm the two are equal. The alpha column confirms it:
at a per-tick factor of 0.9407 (0.94 → 0.001 in 112 ticks), 223 frames to alpha 0.001 is 112 steps.

| nodes | arm | frames/s, 3 runs | steps/s, 3 runs | median steps/s | median settle | alpha at 20 s | draws/s | load avg |
|---|---|---|---|---|---|---|---|---|
| 400 000 | before | 21.11, 20.77, 21.36 | 10.56, 10.39, 10.68 | **10.56** | **10.56 s** | 0.001 (all 3) | 57.0–57.1 | 12.9–15.0 |
| 400 000 | after | 13.44, 16.52, 17.41 | 13.44, 16.52, 17.41 | **16.52** | **6.78 s** | 0.001 (all 3) | 55.9–58.6 | 13.5–14.0 |
| 1 000 000 | before | 4.24, 7.81, 8.16 | 2.12, 3.91, 4.08 | **3.91** | not reached | 0.090, 0.010, 0.008 | 19.5–36.9 | 14.2–16.8 |
| 1 000 000 | after | 5.71, 5.83, 4.66 | 5.71, 5.83, 4.66 | **5.71** | ≈ 19.2 s | 0.0011, 0.0010, 0.0049 | 39.1–50.8 | 16.1–16.6 |

Settle is 112 steps over the run's steps/s. Median gap between steps in the after arm: 59.4 ms at
400k (deciles 51.9–105.5 ms) and 157.7 ms at 1M. In the before arm the median *frame* gap was
44.5–47.8 ms at 400k, about 95 ms per step.

**400k: ×1.56 steps/s, settle 10.56 → 6.78 s. 1M: ×1.46 steps/s; the after arm reached alpha 0.001
inside the window in 2 of 3 runs, the before arm in none.** Draws per second did not drop: the page
draws from its own frame loop and the worker's frames are only its input.

Raw output: `$GM_SCRATCH/bench/live-cadence/tick-<n>-<arm>-<run>.out`. A first set of 12 runs is in
`void/`: `app/dist` was root-owned after the docker build, the swap failed silently and every run
served the before build. The arm guard in `run.sh` exists because of it.

## Gates

| Check | Result |
|---|---|
| `scripts/studio.sh check` (tsc, unit and render tests, eslint, vite build) | PASS |
| `scripts/studio-smoke.sh` over `b5fe87a` | rc 0, 6/6 rows PASS |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` (negative control) | rc 1, `smoke-no-exception` FAIL as expected |

## Where the 1M step goes now

`LIVE_PROFILE=1 PERF_MEMORY=10g scripts/studio-probe.sh live-tick 1000000 webgl2 20`, after arm,
15.3 s sampled on the motor worker (the coordinating thread; each helper is its own worker and is not
in this table). Output: `prof-1m-after.out`. 67 frames, 4.67 steps/s, median gap 155.9 ms.

| function (self) | ms | share |
|---|---:|---:|
| collide `Gather` | 2189 | 14.3% |
| PM `Deposit` | 2004 | 13.1% |
| collide `resolve` | 1748 | 11.4% |
| motion `Velocity` | 1037 | 6.8% |
| `LinkPass` | 993 | 6.5% |
| `Condvar::wait` (the coordinator waiting on its helpers) | 972 | 6.3% |
| `link::force` | 947 | 6.2% |
| fft `line_into` | 723 | 4.7% |
| `collide::apply` itself (the serial counting sort in `Grid::build`) | 640 | 4.2% |
| `PoolRunner` closure | 585 | 3.8% |
| `Snapshot::new` + `StringTable::from_strs` (once, at `force.start`) | 441 + 338 | 5.1% |
| `Interpolate` / `Stencils` / `field_at` | 410 / 407 / 258 | 7.1% |
| `publish` (JS) | 296 | 1.9% |

Inclusive: the PM tick 13 604 ms, `collide::apply` 5349, `charge::apply` 4951 (of which
`Mesh::solve` 3639), link `pass_with` 2203.

The loop is no longer the limit; the step is. At 1M one step costs about 158 ms against the plan's
100 ms browser target (`prompts/perf-plan.md` P7). The next levers, by this table: collide (gather,
resolve and its serial counting sort, 30%), the deposit (13%, with a serial counting sort of its own
in `Rows::sort`), the helper wait (6.3%), and the 1.08 s snapshot that `force.start` builds only to
read random start positions.

## What this does not do

- It does not make a step cheaper. A 1M step is 158 ms before and after; the before arm simply asked
  for half as many.
- `Caveat:` the host was loaded the whole time (load average 12.9–16.8 on 20 cores, landers and
  OpenCode jobs alongside). The arms alternated, so they saw the same load, but single runs spread
  (400k after: 13.4–17.4 steps/s). Only the medians are claimed.
- `Caveat:` the browser is SwiftShader, so draws/s is a software rasteriser's figure, not a GPU's.
  The steps/s figures are motor-bound and do not depend on it.
- `Caveat:` a tick that always overruns the period now keeps the worker busy without a pause. A drag
  still reaches the motor, at most one tick late, because pins are applied at the start of each frame.
