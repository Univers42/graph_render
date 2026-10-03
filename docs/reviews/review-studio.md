# review-studio — the studio draws the motor's geometry

Status: **review only**. No code change. Job: `prompts/jobs/review-studio.md` (2026-10-02).
Scope: `packages/graph-render/src/**`, `packages/graph-studio/src/**`.
Reference for the wire format: `docs/contract/binary-layout.md` (format 0.4, authoritative).
Reference for the drawing: `SciGraphs/` (the design reference, the source of the Python oracles).

Counts: **0 BLOCKER, 6 MAJOR, 7 MINOR** (13 rows, every one with evidence: a command and its
output, or the reference `file:line` the code disagrees with); **11 unverified** (listed in full,
not counted as findings).

---

## How the evidence was produced

Two independent sources, both reproducible.

**1. A scratch test over the real painter** (written, run, not committed — the brief allows this;
it has been deleted and `git status --porcelain` is empty). Six cases, one per claim about the
geometry:

```
$ docker run --rm -v "$PWD:/w" -w /w/packages/graph-render node:22-slim \
    node --test --test-reporter=tap --experimental-strip-types tests/zz-scratch-review.test.ts
ok 1 - R1 a Box frame drawn through a sphere look loses w/h and becomes a round sprite
ok 2 - R2 a 3D frame drops Box and Polyline/Curve geometry
ok 3 - R3 the fitted camera crops Polyline vertices outside the node hull
ok 4 - R4 a Curve of degree 4 is drawn as a polyline through its control points
ok 5 - R5 a truncated snapshot is refused, not zero-filled
ok 6 - R6 worldToScreen puts +y down the screen, the reference puts it up
# tests 6
# pass 6
# fail 0
# skipped 0
```

Each case carries its own control (R1 draws the same Box frame with and without spheres; R2 draws
the same frame 2D and 3D; R3 asserts the frame bounds never saw the vertex before asserting the
camera crops it), so a case that passed for the wrong reason would show up.

**2. Screenshots of the running studio.** The `pw` MCP only reaches `127.0.0.1:5174`
(`http://127.0.0.1:5184/` and `http://localhost:5184/` both return `ERR_BLOCKED_BY_CLIENT` — an
allowlist, not a network fault), and the container on 5174 mounts
`/home/dlesieur/Documents/graph_render`, not this worktree. Before using it I proved the two trees
are the same program:

```
$ diff -rq goinfre/wt/review-studio/packages/graph-render/src Documents/graph_render/packages/graph-render/src   # no output
$ diff -rq goinfre/wt/review-studio/packages/graph-studio/src Documents/graph_render/packages/graph-studio/src   # no output
$ diff -rq goinfre/wt/review-studio/crates Documents/graph_render/crates                                       # no output
$ sha256sum */app/public/graph_wasm.wasm
060e15382a0d25a650683cd4d8257431991d064161dfdb077d9fd6110dabfa79  goinfre/wt/review-studio/app/public/graph_wasm.wasm
060e15382a0d25a650683cd4d8257431991d064161dfdb077d9fd6110dabfa79  Documents/graph_render/app/public/graph_wasm.wasm
$ git -C … rev-parse HEAD   # both 701b46a2ad9a832a875485863ab1581b29755e3d
```

Same commit, same sources, same wasm bytes, so a 5174 screenshot is a screenshot of this tree.
Recorded as a decision taken, not a silent substitution.

| screenshot (`/out/`) | what it shows | fixture / command |
|---|---|---|
| `review-studio-box.png` | Box kind, 2D: rectangles of the decoded `w`/`h`, centred | `layout layout.treemap.squarified` on the synthetic vault |
| `review-studio-circle.png` | Circle kind, 2D: per-node `r`, and the `packing.approximate` note surfaced in the log | `layout layout.packing.circle` |
| `review-studio-curve.png` | Curve kind, 2D: the `b→a` link bends through its own control points | `layout layout.dag.sugiyama` + `edges post.style.bezier` |
| `review-studio-polyline-2d.png` | Polyline kind, 2D: the routed links show their grid bends | `synthetic 200 3 1 vault` + `edges post.route.grid` |
| `review-studio-3d-route.png` | the same graph, the same `post.route.grid`, in 3D: every link is a straight chord, the bends are gone | `layout layout.basic3d.helix` (HUD reads `canvas2d 3D`) |
| `review-studio-polyline.png` | a fitted frame whose right-hand end runs under the Controls dock | `fixture post/obstacles.json` |
| `review-studio-fit-before.png` | the same defect at a larger scale: the drawing leaves the frame on the right | `layout layout.grid`, `fit` |
| `review-studio-3d.png`, `review-studio-tree.png` | 3D discs; tidy tree | supporting |
| `review-studio-filter-nofit.png`, `review-studio-filter-fitresults.png` | `search Schema` hides 187/200, `fit to 13 results` — camera behaves, no defect | supporting |

---

## Verdict, per geometry kind

| kind | field the contract defines | decoded? | drawn as specified? | verdict |
|---|---|---|---|---|
| node `Point` (tag 0) | `x`, `y` (and `z` when `dim=1`); no third column | yes, `decode.ts:225-226,230` | yes, as a disc of `style.radius` | **OK** |
| node `Circle` (tag 1) | `x`, `y`, `r` (`x, y, z, r` in 3D), `r ≥ 0` | yes, `decode.ts:227-229` | yes, `r` reaches the drawn radius through `scene.ts:30-31` | **OK 2D** / **BROKEN 3D** (M1) |
| node `Box` (tag 2) | `x`, `y`, `w`, `h`, centre anchor, `w,h ≥ 0` | yes, `decode.ts:231-233` | size and centre anchor are right (`nodes.ts:35-38`); 2D **OK**, 3D **BROKEN** (M1), impostor path **BROKEN** (M2) | **OK 2D flat** |
| edge `Line` (tag 0) | no edge bytes; endpoints from the node columns | yes, `decode.ts:239` | yes, `edges.ts:119,124` | **OK** |
| edge `Polyline` (tag 1) | `offsets[m+1]`, then `pts` in `edge` order, `p = (pts[2p], pts[2p+1])` | yes, `decode.ts:244-246` | vertex order preserved on every hop (decode → `frame.ts:72-75` → `edges.ts:74-75`), no reversal anywhere; **2D OK**, 3D **BROKEN** (M1), and a vertex outside the node hull is cropped by the fit (M3) | **OK 2D** |
| edge `Curve` (tag 2) | one `degree ≥ 1`, then offsets + `pts`; `pts[0]` is c1, `pts[1]` is c2 | yes, `decode.ts:240-246` | c1 then c2, not swapped (`edges.ts:70,72`), 2D **OK**; `degree ≥ 4` silently degrades to a polyline (m1); 3D **BROKEN** (M1) | **OK 2D, degrees 2-3** |
| 3D `z` column (`dim=1`) | after `y`, before the sizes; the reader computes every position from byte 14 | yes, `decode.ts:230` and the offsets shift correctly (the pinned 3D bytes read back) | the projection, the sort and the depth are real; the painter keeps only Point/Circle-as-disc and Line (`paint3d.ts:50-57,96-97`) | **BROKEN** (M1) |
| arrow side | the contract carries **no** direction flag and no side | n/a | heads are symmetric about the chord, so no side is chosen; a head is drawn on every edge, directed or not (`arrows.ts:9-10`, `arrows.ts:49-50`); a routed edge's head points along the source→target chord, not along its last segment (`arrows.ts:34-36`) | **MARKED, not a defect against the contract** — see the note below |
| y axis direction | the contract is silent; the reference flips (`SciGraphs/SciGraphs/core/visualization/text_overlay.py:231`) | n/a | `camera.ts:49-51` never negates y | **BROKEN against the reference** (M6) |
| camera fits the frame it shows | n/a | n/a | fits the node hull grown by `reach` — not the edge hull (M3) and not the area the chrome leaves visible (M4) | **BROKEN** (M3, M4) |
| decode of a short / corrupt snapshot | the whole refusal table in `docs/contract/binary-layout.md` | n/a | refused, never zero-filled (`decode.ts:100-103` and the 12 further guards); R5 | **OK** |
| every action through `src/actions/registry.ts` | `CLAUDE.md` "Architecture (studio)" | n/a | 68 ids all reach the registry, but five user gestures call `view.focus` / `view.select` directly (M5) | **BROKEN** (M5) |
| layer rules (no `src/` import, no React in graph-render) | `CLAUDE.md` "Architecture (studio)" | n/a | all four hold (commands below) | **OK** |

**On the arrow side, so it is not read as a missed check.** `docs/contract/binary-layout.md` has no
direction bit on an edge: "Line (`0`) — none — endpoints come from node geometry". The renderer
cannot know which end is the target, so it draws a head at the declared target on every edge and
`arrows.ts:9-10` marks exactly that as a `Ponytail:` heuristic, which is what the house rules
require of a heuristic. The head being symmetric about the chord (`arrows.ts:49-50`) is also
correct for that reason. This is the documented behaviour, not a defect; the unfixable part is
that the contract has no word for direction at all, which is a contract question, not a renderer
one.

**Layer rules, run:**

```
$ grep -rn "from ['\"]\(\.\./\)*src/\|graph-engine\|@osionos" packages/graph-render/src   # exit 1, no hits
$ grep -rln "react\|React" packages/graph-render/src                                     # exit 1, no hits
$ grep -rn "fetch(\|XMLHttpRequest\|require(" packages/graph-render/src                   # exit 1, no hits
$ grep -rn "graph-engine" packages/graph-studio/src | wc -l                              # 0
$ grep -rn "from ['\"][^'\"]*src/" packages/graph-studio/src | grep -v "graph-render/src\|graph-sdk-js/src" | wc -l   # 0
$ find packages/graph-render/src packages/graph-studio/src -name '*.ts*' | xargs wc -l | sort -rn | head -1
  297 packages/graph-studio/src/source/ingest.ts      # house limit is 300; no file over it
```

`app/` imports only `graph-studio` (`app/src/main.ts:5`, `app/src/parity.ts:6`).

---

## Findings

| id | severity | file:line | defect | failing input | evidence | proposed fix |
|---|---|---|---|---|---|---|
| ST-1 | MAJOR | `packages/graph-render/src/canvas2d/paint.ts:44` → `packages/graph-render/src/three/paint3d.ts:50-57,96-97` | The 3D painter keeps only a disc and a straight chord. `Box` (its `w`/`h` and rim), `Polyline`, `Curve` of every degree, the Line bend and the arrow heads are all dropped, with no error and no note. A 3D snapshot is legal and common: `docs/contract/binary-layout.md` "Edge paths stay 2D whatever `dim` is" | `synthetic 200 3 1 vault` + `edges post.route.grid`, then `layout layout.basic3d.helix` — the routed polylines the 2D view draws as grid bends are 597 straight chords | R2 (ok 2, 0 `bezierCurveTo` and 0 `rect` in 3D against 1 and 2 in 2D) and `/out/review-studio-3d-route.png` beside `/out/review-studio-polyline-2d.png` | Dispatch on the kinds, not only on `space` (`paint.ts:44`): give `paint3d` a `traceBox` and a `traceInterior` of its own, or refuse a z-carrying frame whose kinds the 3D path cannot draw, with a `Ponytail:` marker naming the loss |
| ST-2 | MAJOR | `packages/graph-render/src/canvas2d/nodes.ts:143` (`impostorOf` at `:127-128`, `paintSphere` at `:96-113`) | The impostor branch never reads `nodeKind`. A `Box` drawn through a look that carries `spheres` is blitted as a round sprite of side `2 * extent`; `w` and `h` are never read, so the aspect and the rect are both gone, and the `Box` rim stroke at `nodes.ts:73-77` is skipped. The flat path at `nodes.ts:61-68` dispatches on the kind correctly — the loss is local to the impostor pass | A `Box` frame (`w = 160, h = 40` and `w = 40, h = 160`) with `style.spheres = [base]` | R1 (ok 1): the same frame draws `rect ×2` with `spheres: null` and `drawImage ×2` with `spheres: [base]`, the sprite size `160` for both boxes | Refuse the impostor path for a `Box` frame (`impostorOf` returns false on `frame.nodeKind === "Box"`) and keep the flat rect; a rect impostor is a second sprite to bake for a case the flat path already draws |
| ST-3 | MAJOR | `packages/graph-render/src/frame.ts:107` (`boundsOf` at `:39-54`, called for the node columns only) | The frame's bounds are the min/max of the **node** `x`/`y` only. A `Polyline` or `Curve` vertex outside the node hull is not in them, so the fit cannot see it and the drawn link runs off the frame. `scene.ts:79` then grows those bounds by `reach`, which widens the node padding but never adds the edge hull | A 2-node `Box` frame at `x = 100..400` with a `Polyline` whose `pts` reach `x = 9000` | R3 (ok 3): `frame.bounds.maxX` is `400`, and `fitCamera(bounds, 1200×800)` puts that vertex at screen `x > 1200` | Fold `frame.pts` into the bounds the camera fits (`frame.ts:107`), or give `Frame` an `edgeBounds` that `setFrame` unions into `local.bounds`. Without it, a routed link is invisible by construction |
| ST-4 | MAJOR | `packages/graph-render/src/camera.ts:69-75` + `packages/graph-studio/src/styles/studio.css.ts` (`.gs-canvas { position:absolute; inset:0 }` over `.gs-root { position:absolute; inset:0 }`) | The camera fits the **canvas**, which is the whole window, while the Controls dock, the legend, the camera bar and the console are absolutely positioned **over** it. A fit that is correct for the canvas puts a third of the drawing under panels the user cannot see, so the graph does not fit the frame the studio shows | `fixture post/obstacles.json` (20 n, 5 e, sugiyama): after the automatic fit the right-hand column of nodes is cut by the dock edge; `synthetic 200 3 1 vault` + `layout layout.dag.sugiyama` + `post.route.grid`: the whole band runs off the right | `/out/review-studio-polyline.png` and `/out/review-studio-fit-before.png`; the fit uses `state.viewport` (`loop.ts:165`) and the chrome is `position:absolute` over the same inset:0 canvas | Give the studio a "safe area" (dock width, console height, legend, camera bar) and pass it as the fit viewport, with the camera centring in the safe area rather than the canvas. This is a studio bug, not a renderer one: the renderer is doing what it is told |
| ST-5 | MAJOR | `packages/graph-studio/src/ui/Inspector.tsx:47`, `:65`, `:85`, `packages/graph-studio/src/ui/NodeMenu.tsx:27`, `packages/graph-studio/src/studio/pipeline.ts:155` | Five user gestures call `view.focus` / `view.select` directly instead of going through `dispatch` → `registry.resolve`. `view.focus` and `view.clear` already exist as actions doing exactly this (`actions/view.ts:116-118,125-132`), and `ui/Search.tsx:53` already uses the action for the same operation — so a click in the inspector or the node menu is not logged, cannot be typed in the console, and is the one path with no argument validation. `CLAUDE.md` "Architecture (studio)" requires every user action to go through `resolve` | Click "Centre …" on a node in the inspector, or "focus" in the node menu: the log stays empty while `search Schema` + the same focus logs a line | `grep -rn "view\.focus(\|view\.select(" packages/graph-studio/src --include=*.ts --include=*.tsx \| grep -v actions/` → the five lines above; `grep -rn "view.focus\|view.select\|view.clear" packages/graph-studio/src/actions/view.ts` → the two actions | Route all five through `dispatch("view.focus", { node })` and `dispatch("view.clear")`. One line each |
| ST-6 | MAJOR | `packages/graph-render/src/camera.ts:49-51` | `worldToScreen` is `world * scale + offset` with no y negation, so a larger world y lands **further down** the canvas. The reference flips y on the way into pixels, and the renderer's own `edges2d/curve.ts:7-8` says the same ("SciGraphs' world frame, which is y-up: … flips y only on the way into pixels"). The studio therefore draws every snapshot mirrored vertically against the reference | `worldToScreen({x:0,y:0,scale:1}, {x:0,y:1})` → screen `y = 1`; the reference formula at `text_overlay.py:231` (`y_px = (1 - norm_y) * height`) puts the same point at the *top* | R6 (ok 6); `SciGraphs/SciGraphs/core/visualization/text_overlay.py:231`; `packages/graph-render/src/edges2d/curve.ts:7-8` | Negate y once, in `worldToScreen` and `screenToWorld` (`camera.ts:49-55`), and in `three/orbit.ts:134-135` so `up` is `-y`; then drop the per-caller `y: -ends.ay` fixups in `edges.ts:108-109` and `parity/scene.ts:55-56`. **Caveat:** no bundled layout makes it visible — the motor puts the layer axis on `x` for sugiyama — so this is a convention defect proved by the transform and the reference, not by a screenshot |
| ST-7 | MINOR | `packages/graph-render/src/canvas2d/edges.ts:68,73-76` | A `Curve` whose point count is not `degree - 1`, and **every** `Curve` of `degree ≥ 4`, falls into the polyline branch and is drawn as straight segments through its own control points. The contract admits any `degree ≥ 1` ("A curve's `degree` is one value for the whole snapshot (2 quadratic, 3 cubic, …) and must be at least 1"), and there is no `Ponytail:` marker on the fallback | A `Curve` frame with `degree = 4` and 3 interior points | R4 (ok 4): 0 `bezierCurveTo`, 4 `lineTo` | Either evaluate the general degree (de Casteljau on `degree - 1` control points) or mark the fallback with a `Ponytail:` line naming the degrees it gets wrong |
| ST-8 | MINOR | `packages/graph-render/src/three/paint3d.ts:88` | The 3D edge stroke is `max(1 / dpr, 1)`, ignoring `style.edgeWidth` and `edges.scale`, so a "link thickness" setting has no effect in 3D. `globalAlpha` is `1` throughout, so the lit/dim passes of the 2D painter (`nodes.ts:81-88`) and the edge gradient (`edges.ts:231-233`) are missing too, and there is no edge cull margin (`edges.ts:27,56-58`) | `appearance.thickness` set to its maximum, then a 3D layout | `packages/graph-render/src/three/paint3d.ts:88` read against `packages/graph-render/src/canvas2d/edges.ts:221-225`; R2 shows the 3D frame drawing 1 `lineTo` and no gradient batch | Read `style.edgeWidth` and `edgeColour` in `paint3d`, or fold them into ST-1's dispatch refusal |
| ST-9 | MINOR | `packages/graph-render/src/canvas2d/controller.ts:193-194` | `setPositions` copies the live motor columns into the renderer's `Float32Array` with no `isFinite` check, and no other finite guard exists on that path (`decode.ts:116` guards the snapshot face only). A non-finite motor coordinate becomes a drawn sprite, a pick-grid entry and a NaN camera | Any live session whose `positions()` carries a NaN — e.g. a force knob outside its own domain, which `liveSession.ts:50-62` forwards with no range or finiteness check | `packages/graph-render/src/canvas2d/controller.ts:188-194` (length guard present, finite guard absent) against `packages/graph-render/src/snapshot/decode.ts:115-118`; `packages/graph-studio/src/motor/liveSession.ts:50-64` | Filter non-finite values in `setPositions` and refuse the whole batch (one bad coordinate is a motor bug, not a drawing), the way `decode.ts:116` does |
| ST-10 | MINOR | `packages/graph-studio/src/studio/fitResults.ts:20-26` | `cameraFor` re-implements the fit with `FIT_MARGIN` (`1.12`, from `look/presets.ts`) instead of `FIT_PADDING` (`64 px`, `camera.ts:43`), and drops `fitCamera`'s "never past 2" ceiling (`camera.ts:87`) and its degenerate-frame floor. It also has no `isFinite` guard, while `camera.ts:45-46`'s `clamp` propagates NaN, so a NaN coordinate yields a NaN camera **and** the success message at `:39` | `search` for a term matching 2 nodes: `fit to 2 results` zooms to `limits.max` (40), where `fit` (⤢) would stop at 2 | `packages/graph-studio/src/studio/fitResults.ts:20-26` against `packages/graph-render/src/camera.ts:69-89`; observed in the browser: "fit to 13 results" with no camera change (`/out/review-studio-filter-fitresults.png`) | Add `fitCamera(bounds, viewport, { maxScale, padding })` to graph-render and have `cameraFor` call it with the look's margin; refuse a non-finite box instead of reporting success |
| ST-11 | MINOR | `packages/graph-render/src/three/sort.ts:18,80-83` | The depth sort is a counting sort with one bucket per **world unit** of depth, so two nodes within a unit of each other are painted in dense-index order rather than by depth. Deterministic (D2/D3 hold: a fixed reverse loop then a forward dense scan, no `Map`) but visibly wrong at coarse scale. The same file's `?: 0` on typed-array reads is dead code | A 3D frame whose nodes are all within one world unit of depth — e.g. `layout layout.basic3d.helix` with `appearance.maxpx` raised | `packages/graph-render/src/three/sort.ts:18,46-57,80-83` | Scale the bucket width to the depth range (`(max - min) / buckets`) so the quantiser is resolution-independent |
| ST-12 | MINOR | `scripts/studio.sh:74-77` | `stage_assets` writes `app/public/{graph_wasm.wasm,fixtures/}` into the **tracked** tree, and no `.gitignore` rule covers `app/public` (`.gitignore` lists `/target` and `verify/` only). Every `scripts/studio.sh wasm|serve|build` leaves untracked build products that the next `git status` and the orchestrator's commit have to notice. Found while running this review | `scripts/studio.sh wasm`, then `git status --porcelain` | `git check-ignore -v app/public` → no match; `ls app/public` after the build → `fixtures  graph_wasm.wasm`; `scripts/studio.sh:74-77` | Add `/app/public` to `.gitignore`, or stage it under `target/` and point vite's `publicDir` there |
| ST-13 | MINOR | `packages/graph-render/src/canvas2d/edges.ts:124` | While the layout is still settling (`!input.settled`) a `Polyline`/`Curve` is drawn as a straight source→target line and its `pts` are ignored. The `Ponytail:` markers at `edges.ts` and `arrows.ts:9-12` do not cover this one, and a heuristic without a marker breaks the house rule | Any routed drawing during the settle, which is every drawing for the first frames after a layout change | `packages/graph-render/src/canvas2d/edges.ts:124`; the `bend`/`traceInterior` split at `:118-124` | Add the `Ponytail:` marker ("a settling frame draws routed links straight; the bends appear when it settles") or draw the `pts` regardless of `settled` |

### Also checked, and clean

- **Decode is loud, not silent.** `decode.ts` refuses a short buffer before any allocation
  (`claim` at `:100-103`, reached from every column read), a trailing byte (`:279-281`), a
  non-finite value (`:115-118`), a negative `r`/`w`/`h` (`:124`), non-monotonic offsets
  (`:138-140`), a non-zero pad (`:153`), an endpoint `≥ n` (`:161-163`), a bad magic (`:206-208`),
  `dim ≥ 2` (`:191-192`), a stage count other than 1 (`:217`) and an unknown kind tag (`:197`).
  R5 cuts a valid 84-byte snapshot at 1, 4, 12 and 4-from-the-end bytes and every cut is refused
  with `truncated`; the full buffer decodes to `nodeCount 2`. Arrays are always views sized from
  the header counts, so there is no zero-filled or uninitialised path. The two silent
  degradations are deliberate and documented in the file's own header (`decode.ts:8-9`): an
  unknown note code reads as `name: null`, and an out-of-range `idAt` returns `""` (`:288`).
- **Curve control-point order.** `pts[2*from]` is c1 and `pts[2*from+1]` is c2, in the order
  `docs/contract/binary-layout.md` stores them, gated on `to - from === degree - 1`
  (`edges.ts:68-72`). Not swapped, not reversed.
- **Polyline vertex order.** Preserved on every hop: `decode.ts:244-246` takes the `pts` window as
  stored, `frame.ts:72-75,105-106` copies it element by element, `edges.ts:74-75` walks it
  ascending. No reversal anywhere in the tree.
- **Box size and anchor.** `w`/`h` come from the decoded columns, not a constant
  (`nodes.ts:35-36`), floored only to one device pixel (`:40`); the anchor is the node's own
  centre (`left = x - w/2`, `top = y - h/2`, `:37-38`), which is the convention the motor's own
  treemap uses. `/out/review-studio-box.png`.
- **Point vs Circle.** They share one tracer (`nodes.ts:44-57`) and differ only because `extent`
  is resolved per kind upstream (`scene.ts:30-37`): a `Point` frame has `r = null` and takes
  `style.radius`, a `Circle` takes its own `r`. Correct per the contract, which gives `Point` no
  third column.
- **Every registered action is reachable.** 68 ids (59 literal, 4 generated at `forces.ts:76-79`,
  5 from `SECTIONS.map` at `portable.ts:27,32`), all spread into `studioActions()` at
  `all.ts:19-25`, none orphaned. `ui/` imports action *values* only from `Dock.tsx:4` and
  `KeyOverlay.tsx:7`; every other `actions/*` import under `ui/` is type-only. Every invocation
  goes `dispatch` → `resolve` (`ui/Console.tsx:89`, `NavBar.tsx:44`, `Toast.tsx:26`,
  `useShortcuts.ts:81,91,99`, `Search.tsx:47,53,91`, `ActionForm.tsx:54`, `ForcesPanel.tsx:39,70`;
  resolve at `studio.ts:134,197,200`). Arg validation is real: `resolve` → `argsFrom` →
  `numberFrom` refuses a non-finite or out-of-range number (`registry.ts:108-118`), observed live
  as `ActionRefusal … 'shape' is not one of: vault, random`. The gap is ST-5, not the registry.
- **Layer rules and the 300-line limit.** All clean; commands above.

---

## Unverified

Listed, not counted as findings: no evidence was produced, so neither pass nor fail is claimed.

1. **The impostor path in a real browser.** ST-2 is proved by the painter's own recorded calls
   (R1), not by pixels: no action in the tree sets `style.spheres` (grep over every `.ts`/`.tsx`
   returns only `parity/scene.ts:125`, which sets it to `null`, and the tests), so the studio UI
   cannot reach the branch and no screenshot of it exists. `nodes.ts:6-7` documents the branch as
   what a SciGraphs look does, so either the look path should set `spheres` or the branch's own doc
   is stale. Which of the two is the defect is not decided here.
2. **Every `spheres`-free look.** Whether the six `LOOK_NAMES` presets are meant to carry sphere
   bases (`look/presets.ts:35-43` gives them a flat `node` colour only) was not resolved against
   `SciGraphs/api/render.py:45-101`.
3. **The y flip's visible effect (ST-6).** No bundled layout puts a signed quantity on `y`, so
   the defect is proved by the transform and the reference, not by a drawing. Whether the motor
   is y-up or y-down per layout is a `graph-core` question, outside this scope.
4. **`Point` vs `Circle` as the default.** A `Point` frame is drawn at `style.radius`, so it is
   pixel-identical to a `Circle` of that radius. Whether that is the intent is not stated anywhere.
5. **`setPositions` with a real NaN (ST-9).** The guard's absence is read from the source; no
   motor input that produces a NaN was found, so the failing input is hypothetical.
6. **`three/sort.ts:80-83` bucket width (ST-11).** The one-world-unit bucket is read from the
   source; no measurement of visible z-fighting was taken.
7. **Labels and the fit.** `camera.ts` has no label metric at all, so a label on a boundary node
   can clip; no screenshot was taken of a clipped label.
8. **The `spheres` budget boundary.** `IMPOSTOR_BUDGET = 4096` (`nodes.ts:22`): a `Box` scene
   above it regains its shape, so the defect's blast radius is a small scene, not a large one. Not
   measured against a real 4097-node frame.
9. **`quick.rows` has no renderer or oracle row.** `grep -nEi "oracle|no-oracle|motor-alone|render" scripts/orch/rows/quick.rows`
   returns nothing; that file is 11 Rust/CLI rows. So the `no-oracle-import` and `motor-alone` gate
   rows named in `CLAUDE.md` "Architecture (studio)" were not located and no claim rests on them.
   The layer rules were checked by grep instead, and that is what the commands above are.
10. **`app/public/fixtures/` is a build product.** The job asks for one fixture per geometry kind
    under it; the directory does not exist at rest (`scripts/studio.sh:74-77` creates it) and the
    per-kind screenshots were driven from the console instead. No fixture in `fixtures/` names a
    `Circle` or `Box` case, because the fixture JSON is topology-only input and the kind comes
    from the layout. Whether a per-kind fixture set is wanted is a decision, not a finding.
11. **The `appearance.animate` doc drift.** `actions/display.ts:111` drives
    `context.animation.start` outside the `look()` path, which makes the `actions/appearance.ts:1`
    header ("none of these asks the motor for anything") false for that one action. Read, not
    run; and `actions/groups.ts:186` calls `context.look(settings)` directly, skipping the
    analysis guard at `actions/look.ts:7-10`. Both are one-line observations without a run.

---

## Counts

| severity | count |
|---|---|
| BLOCKER | 0 |
| MAJOR | 6 (ST-1 … ST-6) |
| MINOR | 7 (ST-7 … ST-13) |
| total rows | 13, every one with evidence |
| unverified | 11 |

Per area, every module in scope named with its finding count (0 is a valid count):

| module | findings |
|---|---|
| `graph-render/src/snapshot/decode.ts` | 0 |
| `graph-render/src/frame.ts`, `scene.ts` | 1 (ST-3) |
| `graph-render/src/camera.ts` | 2 (ST-4, ST-6) |
| `graph-render/src/canvas2d/nodes.ts` | 1 (ST-2) |
| `graph-render/src/canvas2d/edges.ts`, `arrows.ts` | 2 (ST-7, ST-13) |
| `graph-render/src/canvas2d/paint.ts`, `three/**` | 3 (ST-1, ST-8, ST-11) |
| `graph-render/src/canvas2d/controller.ts` | 1 (ST-9) |
| `graph-render/src/style.ts`, `look/**`, `colour/**`, `labels*/**`, `adjacency.ts`, `grid.ts`, `selection.ts`, `gesture.ts`, `drag.ts`, `pointer.ts`, `fade.ts`, `transition.ts`, `spacebar.ts`, `view-stats.ts`, `theme.ts` | 0 |
| `graph-studio/src/actions/**` | 1 (ST-5) |
| `graph-studio/src/studio/**` | 1 (ST-10) |
| `graph-studio/src/motor/**` | 0 (ST-9's missing guard is in graph-render's controller) |
| `graph-studio/src/ui/**` | 0 (ST-5's five call sites are counted under `actions/`) |
| `graph-studio/src/state/**`, `source/**`, `console/**`, `look/**`, `parity/**` | 0 |
| `scripts/studio.sh` | 1 (ST-12) |
