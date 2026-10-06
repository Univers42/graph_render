# Picking a moving node: hit-testing, `focus()` and labels during a layout switch

`perf-transition` left three things on the table because they were outside its paths: hit-testing
was off for the whole tween, `focus()` aimed at the target pose, and `planLabels` runs on every
tween frame. Its recommended answer to the first two was yes, and it is adopted here. The third
is measured and left alone.

## What is measured, and how

Two numbers, both from `packages/graph-render/tests/bench-tween.ts`, run on the host that builds
this:

```
scripts/orch/node-slim.sh node --experimental-strip-types packages/graph-render/tests/bench-tween.ts
```

It is not a `.test.ts`, so `scripts/studio.sh` never runs it as a gate; it prints and asserts
nothing. The graph is the studio's own shape — `degree 2`, one palette entry, a label per node,
spread over 1 000 world units, viewport 1920x1080 at DPR 1 — built through the same `sceneOf` the
view builds, so the pick grid and the style under the measurement are the real ones. Sixty timed
rounds after thirty warm-up calls, p50 and p95 reported.

**The warm-up is 30 calls, not a handful, and that is a finding.** The eased scan is bimodal
across processes on this host: runs that tier the loop up land near 4 ms and runs that do not sit
near 8, and a five-call warm-up measures whichever tier the timed rounds happen to start in. Four
consecutive short-warm-up runs gave 4.18, 4.21, 4.19 and 8.16 ms. Thirty calls of a 200 000-node
scan is about 0.25 s and buys a number that means the same thing twice.

The browser arm is a pw pass against the built studio, described at the end.

## 1. One mid-tween pick at 200 000 nodes

The pick grid is built from the target frame (`scene.ts`, `sceneOf` passes the `Frame` itself to
`gridOf`), and every node is somewhere else until the tween ends, so the grid cannot answer a
mid-tween query at all. `pickEased` (`scene.ts`) scans the eased pose instead: one pass, the
position mixed per node from the two halves and the eased fraction the loop already publishes for
the shader, no column allocated and nothing to free.

| 200 000 nodes, one hover query | p50 | p95 |
|---|---|---|
| eased scan, as shipped | **1.66 ms** | 1.84 ms |
| eased scan, no axis early-out | 8.06 – 8.28 ms | 8.16 – 8.92 ms |
| settled grid (`pickIn`), the path outside a tween | 0.018 ms | 0.034 ms |

**The axis early-out is most of the cost, and it is one subtraction.** The scan rejects a node on
`dx` alone — past `tolerance + max(scene.reach, floor)`, the same span `pickNode` spans its cells
with — before it reads the second column, the extent or the hidden flag. Without it the scan is
two float columns, four flops and a `hypot` a node; with it, most nodes never get that far. 1.66 ms
is 10% of a 16.67 ms frame, once per frame, only while a tween is in flight; without it, 8 ms is
half a frame for the same query.

**What this is not.** The scan is O(nodes) where the grid is O(cells the query touches), so the
grid is ~90x faster here and stays the path outside a tween — that is what
`canvas2d/controller.ts:pickAt` does, and `tests/pick-tween.test.ts` asserts the settled frame
picks through the grid again so a build that always scanned could not pass. And a hover is one
`pickAt` per `pointermove` event, which a browser coalesces to roughly one per frame but does not
guarantee: a 1 000 Hz mouse can fire several `pointermove`s inside one frame, and each one is a
full 1.66 ms scan at 200 000 nodes. That is the honest ceiling of this approach and the reason it
is scoped to the 600 ms of a tween.

## 2. `focus()` mid-tween

**The choice: a focus centres the position the node settles at, not the position it is passing
through** (`camera-api.ts`, `focusApi`).

The camera is placed there in one step and stays there. `moveTo` is a hard cut — there is no camera
tween anywhere in `src/`, and `focus` sets `fitted = false`, so a later safe-area change re-fits
nothing over it. The node therefore walks into the middle of the frame for the rest of the tween
instead of the middle chasing it. Aiming at the eased pose instead would mean re-aiming on every
frame of the tween to land anywhere at all, and it would still be wrong on the frame it stopped.

This was already the behaviour; `perf-transition` recorded it as a defect because the camera and
the drawing disagree for 600 ms, which is true of the camera and not of the focus. What was
missing was the test, and `tests/focus-tween.test.ts` is it — including the negative control, which
aims `focus` at `state.x` and fails the first two rows.

## 3. Labels during a tween — measured, not changed

`canvas2d/layout-key.ts:54`'s `travelling ||` forces a full `planLabels` on every tween frame.
The job's rule: change it only if it is more than 3% of the frame. 3% of a 16.67 ms frame is
**0.50 ms**.

`planLabels` p50 in ms, at three node counts, three camera scales and both focus states. "idle" is
no focus; "hover" is a focus set, which is what hovering a moving node does — and with `lit` set
there is no invisible node to break on, so it walks the whole rank order however few labels it
places.

| nodes | idle ×0.9 | idle ×8 | idle ×32 | hover ×0.9 | hover ×8 | hover ×32 |
|---|---|---|---|---|---|---|
| **20 000** | 0.048 | 0.149 | **0.262** | 0.047 | 0.047 | 0.047 |
| 200 000 | 0.020 | 0.153 | 3.012 | 0.840 | 0.475 | 0.472 |
| 1 000 000 | 0.011 | 0.184 | 5.545 | 2.309 | 2.291 | 2.300 |

**At 20 000 nodes the worst case in the whole sweep is 0.262 ms — 1.6% of a frame. `layout-key.ts`
is not changed.** That is the whole finding, and it agrees with `perf-transition`'s "not dominant".

The scale sweep is not decoration. `planLabels` breaks at the first node whose zoom alpha is
invisible, so a camera far enough out that nothing is labelled reads one node and stops: the first
version of this bench fitted the camera and reported 0.090 ms at every size, including a million
nodes, which would have called the line free at every size. The expensive end is zoomed *in*.

**The 200 000 and 1 000 000 rows are over the 3% line and are not a Canvas2D tween frame anyway.**
Above `TWEEN_BUDGET` (32 768, `canvas2d/tween.ts`) the 2D painter snaps instead of easing, so a
tween on the 2D painter at those sizes does not exist; on WebGL2 the GPU layer has the frame and
`overBudget` returns false, so the tween runs but the loop is not what paces it. The zoomed-in
×32 column is also the pathological case — 1.6% of the screen is covered by the drawing, which is
what makes the rank scan walk to its end.

## What changed

- `scene.ts`: `EasedPose` and `pickEased`, the mid-tween scan. `pickIn` is untouched.
- `canvas2d/controller.ts`: `pickAt` asks the scan while `bulk.tween` is set and the grid
  otherwise; the `transitionStart >= 0 → -1` guard is gone. `showFrame`'s non-animated branch now
  clears `bulk.tween`, so "a tween is in flight" means one thing — before, a snap or a resize left
  a stale fraction for the shader to mix and would have sent the scan after it.
- `canvas2d/loop.ts`: `advance` is exported so a test can step a tween to a given fraction without
  a browser's animation frames. Nothing outside that file calls it.
- `camera-api.ts`: the choice above, in a comment.

## Tests

`tests/pick-tween.test.ts` and `tests/focus-tween.test.ts`, 9 rows. The headline row clicks a node
at 50% of a tween on **both** backends and gets that node's id, with the pose computed from the
fraction the loop published rather than from the test's own arithmetic. Its controls: the target
frame is *not* what is drawn (a row that found the node at its target would prove the grid
answered), and the settled frame picks through the grid again.

The differential row is the one worth naming: at `eased` of 1 the eased pose *is* the frame, so 600
seeded queries over a 200-node graph — half within a node radius of a centre, half anywhere — must
return exactly what `pickIn` returns, ties included. That is what the axis early-out needs: it
rejects on `dx` alone before reading `dy`, the extent or `hidden`, and a span one unit too small
shows up there as a miss near the tolerance rather than as a wrong node. Shrinking the span by one
unit fails that row and no other.

Negative controls, run and observed: restoring `if (state.transitionStart >= 0) return -1` fails
the headline row and the tolerance row; aiming `focus` at `state.x` fails both focus rows;
shrinking the early-out's span by one fails the differential row.

## pw MCP pass

`app/dist` served on 127.0.0.1:5173. 2 000 nodes / 3 996 edges from `source.synthetic`, layout
switches through the studio's own `dispatch`. A screenshot round trip is seconds, so the mid-tween
capture is taken **from inside the page** at the click instant: the canvas is read back and POSTed
as the click is dispatched, in the same task as the `pointermove`/`pointerdown`/`pointerup` triple
so that one frame of movement cannot come between them. The tween's own clock is the wait — the
`gm:transition:moved` mark, then 320 ms — and `gm:transition:settled` is read at the click to prove
the tween had not ended.

The node clicked is the one that has travelled furthest from where it started among those on
screen, which is the clearest case a target-frame grid would get wrong.

| backend | switch | clicked | node | moved | pick at the eased pose | pick at the node's start point | store error | console |
|---|---|---|---|---|---|---|---|---|
| canvas2d | `layout.circular.ring` | 329 ms in | 704 | 1 302 world units, 212 px | **704** | **-1** | null | 0 errors |
| webgl2 | `layout.spiral` | 313 ms in | 0 | 46 px | **0** | **205** (a different node) | null | 0 errors |

On canvas2d the node's starting point held nothing at all; on webgl2 it held a different node. Both
are the target grid being wrong and the eased scan being right. `view.stats()` afterwards on both:
`backendFailure ""`, 2 000 of 2 000 nodes and 3 996 of 3 996 edges drawn, no `[role=alert]`,
`.error` or `[data-error]` element with text, no overlay text. The only console output anywhere in
the pass is four `GPU stall due to ReadPixels` **warnings**, which are this pass's own
`canvas.toDataURL` captures, not the app's.

Screenshots in `perf-transition-pick/`: `pw-canvas2d-selected-mid-tween.png` and
`pw-canvas2d-selected-settled.png`, `pw-webgl2-selected-mid-tween.png`. Both mid-tween frames show
the selected node ringed with its neighbourhood lit and labelled, on a drawing that is visibly
between two layouts.

## Gates

`scripts/studio.sh check` exits 0 — 485 graph-render tests, 569 graph-studio, 98 render.

`scripts/studio-interact.sh` **exits 1 on this host, and not because of this job.** `int-node-drag`
fails: the dragged node lands 145.89 px from the pointer where the row wants 150 ± 0.5. That was
verified against the baseline rather than assumed — the four source files this job touches were
restored from `d8093153` (the commit this worktree is cut from), the app rebuilt, and the gate
re-run: `int-node-drag` fails there too, at **146.16 px**, and `int-box` failed on one of the two
baseline runs (324 inside, 323 selected) and passed on the other, which is the boundary flakiness
`perf-transition` recorded. `deploy/nav/` is outside this job's paths and a gate row may not be
weakened to go green. `STUDIO_INTERACT_BREAK=1` exits 1.