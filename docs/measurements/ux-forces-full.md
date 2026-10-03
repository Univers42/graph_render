# ux-forces-full — the nine force knobs, measured

What the studio's live-force panel does to a dense drawing, measured before and after the five
new knobs, on the clustered fixture and a 10 000-node source. Every number below comes from a
command in this file; nothing here was pasted into a console.

Date: 2026-10-03. Branch tip: `e5cdfa4`. Chrome 154.0.8037.92, viewport 1400x900, software
raster, gm-chromium container, shared host.

## The probe

`deploy/nav/overlap.py` (rows in `deploy/nav/overlaprows.py`, driver `scripts/studio-overlap.sh`):
it serves a built studio on 127.0.0.1, drives it in headless Chromium over CDP, settles a graph
from random positions, fits the whole drawing, and counts the node pairs whose **drawn discs**
overlap on screen. A pair overlaps when the distance between the two screen centres is under the
sum of the two screen radii, read the way the canvas painter draws them (`frame.r`, else half the
larger box side, else the style radius) times the camera scale, floored at the painter's
`MIN_SCREEN_RADIUS` = 1.25 px. Pairs are found with a uniform grid one disc wide, so a pile-up
costs O(k²) in its own cell — which is the thing being measured.

The same probe counts the live loop's frame rate at 10 000 nodes: it hooks `view.setPositions`
and timestamps every frame the page receives, over three 3 s windows, and takes the median.

Sources: `fixtures/force/clustered.json` (60 nodes) and the studio's own synthetic generator at
`{nodes: 10000, degree: 2, seed: 1, shape: "random"}`, both reached through the registry
(`source.fixture`, `source.synthetic`). The defaults rows run on a freshly loaded page, so the
knobs are the motor's own defaults; the `spread` rows press `forces.spread` on the drawing
already on screen, the way a user does, then settle again.

Re-run (three builds, interleaved, because the host is shared):

```
scripts/studio.sh build
STUDIO_OVERLAP_DIST=app/dist                  STUDIO_OVERLAP_LABEL=after  scripts/studio-overlap.sh
STUDIO_OVERLAP_DIST=target/dist-before        STUDIO_OVERLAP_LABEL=before scripts/studio-overlap.sh
STUDIO_OVERLAP_BREAK=1                         STUDIO_OVERLAP_LABEL=negctl scripts/studio-overlap.sh
```

`target/dist-before` is `origin/develop` (`git archive origin/develop` into `target/before-src`,
its own `graph-wasm` built from develop's `crates/`, then `vite build`). It is needed because
develop's SDK wants `gm_force_session_create_mesh`, which this branch's `crates/` does not export:
serving develop's studio over this branch's wasm dies with "module lacks
gm_force_session_create_mesh". Output lands in `target/studio-overlap/<label>/`.

## Before and after

Overlap: pairs of drawn discs that cover each other / nodes in one overlapping pair.
fps: the live loop's frames per second at 10 000 nodes, the median of three 3 s windows inside one
settle, three probe runs per build. The plain number is the median of the three runs; `pooled` is
the median of that build's nine windows. Both aggregations are in the table because the band this
has to stay inside is ±3%.

| measurement | before (`origin/develop`) | after (this branch) | change |
|---|---|---|---|
| overlap, clustered, defaults | 0 pairs, 0/60 nodes | 0 pairs, 0/60 nodes | — |
| overlap, 10 000 nodes, defaults | 1833 pairs, 2590/10000 | 1833 pairs, 2590/10000 | 0 |
| overlap, 10 000 nodes, after `spread` | not run: no `forces.spread` | 317 pairs, 584/10000 | −82.7% |
| overlap, clustered, after `spread` | not run: no `forces.spread` | 0 pairs, 0/60 nodes | — |
| fps, 10 000 nodes, defaults | 40.95 (pooled 41.06) | 42.30 (pooled 42.30) | **+3.3%** (pooled +3.0%) |
| fps, 10 000 nodes, after `spread` | not run | 43.02 | +1.7% over this build's defaults |

Per-run fps medians, so the spread of the host is visible:

| build | run 1 | run 2 | run 3 | median |
|---|---|---|---|---|
| before | 40.95 | 40.71 | 42.18 | 40.95 |
| after, defaults | 42.30 | 40.29 | 42.34 | 42.30 |
| after, `spread` | 43.02 | 42.48 | 43.38 | 43.02 |

Reading the frame rate: the loop is **3.3% faster** after the change (3.0% pooling all nine
windows of a build), so nothing regressed, but the ±3% band is crossed by 0.3 points in the fast
direction and that is inside the noise — the nine windows of the "after" build span 37.9 to 43.2
fps and the "before" build 39.4 to 43.0. The knobs themselves are free: inside the same
build, `spread` (nine parameters pushed to the motor, collide radius 16 → 21, repel 90 → 270)
moves the median by +1.7%, which is the run-to-run spread. Two caveats on this comparison:
`origin/develop` carries a different motor (its `crates/graph-wasm` adds a force-mesh session
export and a thread pool), so "before" is not "the same motor without the knobs"; and the counts
at the defaults are bit-identical between the builds (1833 pairs both), which says the defaults
path is unchanged by this branch.

The overlap numbers are the actual finding. At the defaults a 10 000-node random graph draws
1833 overlapping pairs over 2590 nodes — the pile-up the panel is for — and `spread` cuts that
to 317 pairs over 584 nodes. The 60-node clustered fixture has **no** overlap at the defaults
(0 pairs, largest disc 11.2 px at scale 1.01), so its before/after row is 0 → 0: it is not the
case the presets are for, and the 10 000-node source is.

## Step 3: the collide radius is in layout units, the user sees pixels

The conversion is in the committed code, and it was confirmed in the browser.

- `packages/graph-studio/src/state/keepForces.ts:63` — `drawnRadius` returns the largest drawn
  disc as a radius in world units: the frame's own radius column, else half the larger box side,
  else the style's `maxRadius`, and never below `MIN_SCREEN_RADIUS / scale`, because a disc
  smaller than 1.25 px is not drawn at all. A live settle paints the motor's own coordinates, so
  world units are layout units from then on.
- `packages/graph-studio/src/actions/forces.ts:131` — `spacingFor` turns that into the collide
  radius: `Math.min(400, Math.ceil(drawn * (1 + margin) * 2) / 2)`, i.e. one drawn radius plus
  `SPREAD_MARGIN` = 0.5 of a radius, rounded **up** to the slider's 0.5 grid and clamped to
  `KNOB_LIMITS.collideRadius.max` = 400, which is the motor's own range
  (`crates/graph-core/src/layout/force/session/live_params.rs:113`, `finite, 0..=400`). Two nodes
  then clear each other with half a radius of margin, because the motor pushes apart any pair
  closer than `2 * collide_radius` (`crates/graph-core/src/layout/force/barnes_hut/collide.rs:63`).

Measured in the pw pass on `force/clustered.json` after a settle and a fit: largest world extent
11.071 layout units, camera scale 0.711 (so the 1.25 px floor is 1.758 world units and the extent
wins), and `forces.spread` set `collideRadius` to **17** — `11.071 × 1.5 = 16.607`, rounded up to
17 on the half-unit grid — with `charge` 90 → 270. On the 10 000-node source the same preset said
`node spacing 21`, i.e. a drawn radius of 14.0 layout units. The value is read at the zoom of the
press, so pressing it again after a zoom re-sizes the spacing; that is documented on
`spacingFor` and is deliberate (one spacing for every node, sized to the largest disc).

## Gates, with the exit codes this job ran

| command | exit | note |
|---|---|---|
| `scripts/studio.sh check` | 0 | types, 427 + 574 unit tests, 100 render tests, lint, build |
| `scripts/studio-live.sh` | 0 | 4 rows PASS |
| `STUDIO_LIVE_BREAK=1 scripts/studio-live.sh` | 1 | negative control, 4 rows FAIL |
| `scripts/studio-interact.sh` | 1 | `int-node-drag` FAIL — see below |
| `STUDIO_INTERACT_BREAK=1 scripts/studio-interact.sh` | 1 | negative control, `int-hover` and `int-node-drag` FAIL |
| `scripts/studio-overlap.sh` (`app/dist`) | 0 | 7 rows PASS |
| `STUDIO_OVERLAP_BREAK=1 scripts/studio-overlap.sh` | 1 | the three `spread` rows NOT-RUN |

**`int-node-drag` is a pre-existing failure, not this job's.** The row drags a node 150 px and
reads the node's model position back: it lands 146.16 px from the pointer here and **145.89 px on
`origin/develop`'s own build** (measured with the same probe against `target/dist-before`), so the
row is red before the knobs existed. The drag is only pinned in the live session when
`element.ts:145` finds no refusal; a non-null reason (no `force-state` absorbed yet, or no live
session) makes every drag view-only, and the node then stays where the settle left it — which is
the ~150 px minus a few px of residual drift that both builds measure. `element.ts`,
`motor/bridge.ts` and `motor/session.ts` belong to the open `perf-pm-live` branch and are out of
this job's paths, so the row is reported, not fixed.

`int-box` failed once on this build (324 inside the box, 323 selected — a node on the box edge)
and passed on the next run of the same build (323/323) and on `origin/develop`: a borderline
pick, not a regression.

The negative control for `scripts/studio-interact.sh` exits 1 as it must (`int-hover` expects a
fade of 0.13, which the app does not make), but it is a weak control while `int-node-drag` is
already red: the control has to fail on its own row, not on the one above.

## Browser pass (pw MCP)

`app/dist` served on 127.0.0.1:5173, the clustered fixture loaded, a settle, a fit, then
`forces.spread` and a second settle. Screenshots:
`docs/measurements/ux-forces-full/clustered-defaults.png` (before) and `clustered-spread.png`
(after). Fault channels after both steps: `Runtime.exceptionThrown` 0, console errors 0, console
warnings 0, `store.error` null, the `.gs-alert` banner null, `busy` empty. Before and after the
change are visually what the numbers say: the clusters touch at the defaults and stand apart after
`spread`.