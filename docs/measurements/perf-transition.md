# A layout switch: what it costs, and what it costs now

The path of one layout switch at 2 000, 20 000 and 200 000 nodes on both backends, timed from the
click to the settled frame, before and after the fix.

## What is measured, and how

`scripts/studio-probe.sh transition <nodes> <backend> [from-layout] [rounds]` drives the studio
through its own action registry — the same path a dock click takes — on a graph of `nodes` nodes
(`degree 2`, `seed 1`, `random`; two links per node, so 399 996 edges at 200 000). It switches
`layout.random` → `layout.grid` → `layout.random` three times each way, so every round starts from
the same settled picture, and reports the median of the three.

Five points come off four user-timing marks. `gm:transition:request` and `gm:transition:bytes` are
put down by the studio around the motor round trip (`studio/pipeline.ts`, `arrange`); `gm:transition:moved`
and `gm:transition:settled` by the render side on the tween's first moving frame and on the frame
that ends it (`transition.ts`, `markTween`). The gaps between animation frames are sampled over the
tween window, and the GPU uploads are counted by wrapping `bufferData`/`bufferSubData` on the page,
so "one upload a frame" is a number read off the driver and not a claim about the source.

Host: 20-core x86-64, Chrome 154.0.8037.92, viewport 1920x1080 at DPR 1, WebGL2 on SwiftShader
(the software rasteriser: no GPU on this host, so the numbers rank builds on one machine and are not
what a GPU would show; `GM_GPU=1` measures that as a different arm).

## Before

| nodes | backend | click | worker | bytes → first frame that moves | tween | click → settled | frames drawn | uploads | upload KiB | frame p50 | frame p95 | paint p50 | paint p95 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2 000 | webgl2 | 0.1 | 20.6 | 4.0 | 615.5 | 626.5 | 26 | 60 | 512 | 16.67 | 33.34 | 18.40 | 21.46 |
| 20 000 | webgl2 | 0.1 | 20.1 | 10.0 | 600.8 | 637.5 | 28 | 62 | 5 117 | 16.67 | 33.34 | 16.57 | 22.07 |
| 200 000 | webgl2 | 0.2 | 189.5 | 66.0 | 558.9 | 818.1 | 24 | 68 | **55 859** | 16.67 | 33.34 | 15.83 | 20.56 |
| 2 000 | canvas2d | 0.1 | 3.8 | 4.2 | 602.0 | 616.4 | 36 | 0 | 0 | 16.66 | 16.67 | 1.00 | 1.10 |
| 20 000 | canvas2d | 0.1 | 20.5 | 6.5 | 611.7 | 641.1 | 31 | 0 | 0 | 16.67 | 33.34 | 2.56 | 2.74 |
| 32 768 | canvas2d | 0.1 | 35.1 | 12.9 | 604.4 | 648.4 | 20 | 0 | 0 | 33.33 | 50.00 | 4.13 | 4.53 |
| 65 536 | canvas2d | 0.1 | 60.8 | 16.2 | 613.0 | 691.5 | 15 | 0 | 0 | 33.34 | 50.00 | 4.45 | 6.00 |
| 131 072 | canvas2d | 0.1 | 197.8 | 31.6 | 599.9 | 833.5 | 6 | 0 | 0 | 100.00 | 133.33 | 9.32 | 12.64 |
| 200 000 | canvas2d | 0.1 | 173.0 | 52.5 | 672.5 | 875.1 | **3.5** | 0 | 0 | **208.3** | 241.7 | 20.73 | 23.06 |

All times in milliseconds. `worker` is the motor round trip between the two studio marks; `bytes →
first frame that moves` is what is left after the snapshot arrives: decode, `frameFrom`, the scene
and the first tween frame. `frames drawn` counts the animation frames inside the tween window —
36 is the whole 600 ms at 60 Hz.

The first baseline run had a defect in the probe's round hygiene, which showed a returning tween's
marks leaking into the next round and one 200 000-node round reading `click → settled` of 173.4 ms
with no tween at all. The probe was fixed (the way back now settles before the marks are cleared),
the affected row re-measured, and the one round that is still not a tween is dropped here: its
neighbours in the same case are 834.6 and 915.5, which is the 875.1 above.

### What the numbers name

**On WebGL2 the tween's dominant cost is the upload, and it is enormous.** 55 859 KiB — 54.6 MiB —
went to the driver during a single 600 ms switch at 200 000 nodes, over 68 calls: the two position
columns, once per frame, for as long as the tween ran. That is 1.5 MiB a frame of `bufferSubData`
that the CPU had just written, and the frames still took 20.6 ms at p95. The job's own fact sheet
predicted this; the measurement puts a number on it. `paint p50` on WebGL2 is high even at 2 000
nodes (18.4 ms) because SwiftShader charges its raster to `paint()`, so the upload's share cannot be
read out of `paint` directly — it is visible in the bytes, and in `paint p95` once it is gone.

**On Canvas2D the tween stops being a tween somewhere around 32 768 nodes**, and the curve is
smooth and monotone: 36 frames at 2 000, 31 at 20 000, 20 at 32 768, 15 at 65 536, 6 at 131 072 and
**3** at 200 000. A 600 ms tween that gets three frames is a slideshow with a 208 ms frame gap, and
the user waits 835 ms to see a picture they could have had at once.

The two mechanisms are disjoint, which is why they need disjoint fixes.

## The fix

### WebGL2: the four columns go up once, the shader moves the nodes (`webgl2/`)

`Tween` (`layer.ts`) carries the `from` and `to` columns and the eased fraction; `Pace` carries it
beside the counters the layer already took. `syncNodes` uploads all four columns on a tween's first
frame and then nothing, and `shared` sets one float, `u_eased`, per frame. The node, point and edge
vertex shaders read the position through `place()`, which returns `vec2(a_x, a_y)` untouched when
`u_eased` is 1 — so a settled frame's arithmetic is exactly what it was, and the mix never draws a
still picture. The edge lines read the same four columns through the same index list, so they follow
for free.

`positionsDue` (`sync.ts`) is that gate on its own, extracted because node has no WebGL2 context and
a *missing* upload is invisible to every other test in the package: the layer would still draw a
moving tween, just from the columns it happened to hold. `tests/webgl2-sync-tween.test.ts` asserts
"a tween uploads once, and the frame that ends it uploads nothing either" — the last row, and the
reason the settled frame after a switch does not re-upload the target columns.

Two things ride along because they were in the same per-frame path. `placed` is now recorded on
every tween frame rather than only on an upload, which is what lets the frame that ends the tween
find the target columns already in `buffers.x`. And `syncEdges` holds its `measurePairs` at whatever
the columns said when the tween started: the chords it holds are the ones the tween is walking
between, and the result only feeds `sampleStep`, the thinning estimate for moving frames. The
settled frame after it measures again and is exact. Both carry a `Ponytail:` line saying so.

The quad pass gathers its two `from` columns only while a tween is in flight; at `u_eased` of 1
`place()` returns before it reads them.

### Canvas2D: a node budget, above which it snaps (`canvas2d/loop.ts`)

`TWEEN_BUDGET = 32_768`. Above it the 2D painter snaps to the new layout on its first frame rather
than easing, because a tween it cannot finish is worse than no tween. The budget is read off the
before table: 20 000 held 16.7 ms frames and drew 31 of the 36 the tween wanted, 32 768 drew 20 and
its frame gaps were already 33 ms, 65 536 drew 13, 131 072 drew 6 and 200 000 drew 3 at 208 ms a
frame. `overBudget` also asks whether the GPU layer had the frame (`counts.bulk`), so a tween over
the budget on WebGL2 still runs — the budget is the 2D painter's alone, and the mix is one float a
frame there.

`TWEEN_BUDGET` and `overBudget` are exported and narrow so `tests/transition-marks.test.ts` can read
the number rather than a hard-coded copy of it.

### Timing marks

`markTween` (`transition.ts`) puts `gm:transition:moved` on a tween's first moving frame and
`gm:transition:settled` on the frame that ends it, once each, keyed by the tween's own clock so the
view carries no flag for it. A tween the 2D painter snapped over its budget has no moving frame at
all and marks `settled` alone — the end of that switch is a real time, and a probe waiting for it
must not wait for a beginning that never comes. Both are asserted, with the controls that a *second*
switch marks again and that a snap after a real tween marks both ends.

## After

| nodes | backend | click | worker | bytes → first frame that moves | tween | click → settled | frames drawn | uploads | upload KiB | frame p50 | frame p95 | paint p50 | paint p95 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2 000 | webgl2 | 0.2 | 22.4 | 4.2 | 612.3 | 645.4 | 30 | **8** | **105** | 16.67 | 33.34 | 16.71 | **19.05** |
| 20 000 | webgl2 | 0.2 | 29.0 | 9.1 | 614.3 | 655.5 | 30 | **8** | **898** | 16.67 | 33.34 | 16.02 | **19.27** |
| 200 000 | webgl2 | 0.1 | 220.0 | 44.0 | 574.9 | 852.5 | 26 | **8** | **8 984** | 16.67 | 33.34 | 16.07 | **19.42** |
| 2 000 | canvas2d | 0.1 | 3.8 | 4.4 | 608.5 | 631.5 | 37 | 0 | 0 | 16.66 | 16.67 | 0.80 | 1.00 |
| 20 000 | canvas2d | 0.1 | 19.7 | 9.2 | 597.5 | 625.7 | 33 | 0 | 0 | 16.67 | 33.34 | 1.50 | 2.80 |
| 32 768 | canvas2d | 0.1 | 35.3 | 13.4 | 601.0 | 649.8 | 19 | 0 | 0 | 33.33 | 50.00 | 4.27 | 4.62 |
| 65 536 | canvas2d | 0.1 | 38.5 | — | snapped | **57.9** | 0 | 0 | 0 | — | — | — | — |
| 131 072 | canvas2d | 0.1 | 73.7 | — | snapped | **103.5** | 0 | 0 | 0 | — | — | — | — |
| 200 000 | canvas2d | 0.1 | 113.1 | — | snapped | **157.1** | 0 | 0 | 0 | — | — | — | — |

### Kept, with the margin over 3%

| what | before → after | change |
|---|---|---|
| webgl2 uploads per switch, 200 000 nodes | 68 calls / 55 859 KiB → 8 / 8 984 KiB | **−84 % bytes** |
| webgl2 uploads per switch, 20 000 nodes | 62 / 5 117 KiB → 8 / 898 KiB | **−83 %** |
| webgl2 uploads per switch, 2 000 nodes | 60 / 512 KiB → 8 / 105 KiB | **−80 %** |
| webgl2 `paint` p95, 2 000 / 20 000 / 200 000 | 21.46 / 22.07 / 20.56 → 19.05 / 19.27 / 19.42 | **−11 % / −13 % / −6 %** |
| webgl2 frames drawn during the tween | 26 / 28 / 24 → 30 / 30 / 26 | **+15 % / +7 % / +8 %** |
| canvas2d click → settled, 200 000 nodes | 875.1 → 157.1 | **−82 %** |
| canvas2d click → settled, 131 072 nodes | 833.5 → 103.5 | **−88 %** |
| canvas2d click → settled, 65 536 nodes | 691.5 → 57.9 | **−92 %** |

The 32 768 row is the control: it sits *at* the budget, so its tween still runs and nothing in this
job should have changed it. It moved by 3.4 % on `paint p50` and by one frame in twenty, which is
the run-to-run noise floor of this host and the margin every kept row above has to clear.

### Not kept, or not claimed

**The per-frame CPU blend is still there, and this is the honest limit of the fix.** `state.x` and
`state.y` remain the eased pose, because the label planner reads them (`labels.ts`, `planLabels` at
:133 and `followLabels` at :170), the edge pass reads them (`edges.ts`, `screenEnds` at :145-148),
the quad pass gathers its on-screen nodes from them (`plan.ts`, `onScreen`) and the hit test reads
whichever columns `scene.pickIn` is given. Blending on the GPU means the columns stop being the
eased pose for anything off the GPU, so the blend has to stay for all of them. What went is the
upload, which was the larger of the two: at 200 000 nodes it was 1.5 MiB a frame through the driver
against 0.4 M float writes in JS. Killing the blend too means changing `labels.ts`, `edges.ts` and
`scene.ts`, none of which is in this job's paths.

**`bytes → first frame that moves`** is 4-66 ms and did not move: 66 → 44 at 200 000 nodes is inside
the spread of that measurement (the worker round trip over the same case ranged 171-245 ms across
rounds). It is decode plus `frameFrom`, not the tween.

**The edge-shape measure frozen during a tween** has no isolated measurement of its own; it was
taken because it sat in the same per-frame path as the upload, and it is bounded by `sampleStep`'s
ceiling of one edge in eight.

## What is still not done

**Starting the tween on the first column that arrives.** Not attempted: the snapshot arrives as one
buffer and is decoded in one call (`studio/pipeline.ts`, `decodeSnapshot`), so there is no
column-by-column arrival to start from, and making it one is a change to the wire format in
`snapshot/` and `motor/client.ts`. The cost it would remove is the `bytes → first frame that moves`
column above. Recommendation: stream the position columns as they are produced and begin the tween
on `x`/`y` with `z` joining when it lands; that is a motor and wire change, not a renderer one.

**Hit-testing during a tween.** Picking is disabled mid-tween on purpose
(`canvas2d/controller.ts:248` returns −1 whenever `transitionStart >= 0`) and `scene.pickIn` reads
`scene.frame`, which is the *target*, not the eased pose — so if it were enabled it would hit-test
the wrong positions. Labels and edges are already correct (they read `state.x`/`state.y`, the eased
columns). Enabling it correctly means passing the eased columns into `pickIn` and lifting the guard
in `controller.ts`, and `scene.ts` builds its hit grid from the target frame as well.
Recommendation: pass `state.x`/`state.y` into `pickIn` and drop the guard, so a node is where it
looks during the tween; `scene.ts` and `canvas2d/controller.ts` are outside this job's paths.

**`focus()` mid-tween** (`camera-api.ts:125`) pans to the node's *target* world position while the
nodes are still easing there, so the camera and the drawing disagree for the length of the tween.
Same file, same reason it is not fixed here.

## pw MCP pass

`app/dist` served on 127.0.0.1:5173 (the pw server allows 5173-5175 only). `?backend=webgl2`, 20 000
nodes / 39 996 edges from `source.synthetic`, then three layout switches through the studio's own
`dispatch`, with the canvas captured from inside the page at `moved + 300 ms` and again 250 ms after
`settled` — a screenshot round trip is seconds, so a mid-tween capture taken from outside the page
always lands after the tween.

| switch | click → first frame that moves | tween | store error | console |
|---|---|---|---|---|
| `layout.circular.radial` | 57 ms | 598 ms | null | 0 errors, 0 warnings |
| `layout.spectral` | 3 310 ms | 602 ms | null | 0 errors, 0 warnings |
| `layout.grid` | 28 ms | 602 ms | null | 0 errors, 0 warnings |

`view.stats()` afterwards: `backend webgl2`, `backendFailure ""`, 20 000 of 20 000 nodes and 39 996
of 39 996 edges drawn, no alert element and no overlay text in the host. Screenshots in
`perf-transition/`: `pw-webgl2-<layout>-mid-tween.png` and `pw-webgl2-<layout>-settled.png` for each
of the three. `pw-webgl2-circular.radial-mid-tween.png` is a half-collapsed blob and
`pw-webgl2-circular.radial-settled.png` is the finished radial layout — the mix is drawing the eased
pose, not the target and not a stale frame.

## Gates

`scripts/studio.sh check` exits 0 (1 143 tests: 476 graph-render, 569 graph-studio, 98 render).
`scripts/studio-backend.sh` exits 0 — the parity row reads 0.5835 % of pixels differing by more than
32 of 255 against a 2 % ceiling, so the settled frame still matches Canvas2D through the new
`place()` — and `STUDIO_BACKEND_BREAK=1` exits 1. `scripts/studio-smoke.sh` exits 0,
`STUDIO_SMOKE_BREAK=1` exits 1, and `scripts/studio-probe.sh settle 200 webgl2 negctl-store 40 50
--break` exits 1. Those are the six rows of `perf-p5.rows`, run one at a time rather than through
`gate.sh`.

`scripts/studio-interact.sh` **exits 1 on this host, and not because of this job.** `int-node-drag`
and `int-box` fail. The same two rows were then run against a pristine build of `HEAD` (the same
sources restored with `git show`, rebuilt in a scratch tree): `int-node-drag` fails there too, at
145.89 px against this build's 146.16 px, and `int-box` reported 324 inside / 323 selected there on
two of three runs, the same numbers as here. Both are pre-existing on this host and one of them is
boundary-flaky; `deploy/nav/` is outside this job's paths and a gate row may not be weakened to go
green. `STUDIO_INTERACT_BREAK=1` exits 1.