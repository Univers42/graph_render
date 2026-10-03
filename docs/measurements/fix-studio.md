# fix-studio — the studio draws the motor's geometry

Job: `prompts/jobs/fix-studio.md` (2026-10-02). Source: `docs/reviews/review-studio.md`, ids
ST-1 … ST-13. Scope: `packages/graph-render/**`, `packages/graph-studio/**`,
`scripts/studio.sh` (ST-12 only). No Rust changed, so no conformance row can move.

Every row below has a test that fails on the pre-fix code. The RED line is the assertion that
went red, quoted from the run the sub-slice pasted.

| id | severity | verdict | test | file:line |
|---|---|---|---|---|
| ST-1 | MAJOR | fixed | "a 3D Box draws its own w x h rect and the 2D painter's rim"; "a 3D Polyline draws its interior points, not the chord"; "a 3D Curve goes through quadraticCurveTo at degree 2 and bezierCurveTo at degree 3" | `packages/graph-render/src/three/paint3d.ts:88` (`traceBox`, dispatches on `frame.nodeKind`), `:139` (`traceInterior`, dispatches on `frame.edgeKind`); interior-point projection in `src/three/paths.ts`; `Drawn` gains `boxHalf`/`points`/`ppu` at `src/three/projection.ts:34` |
| ST-2 | MAJOR | fixed | "a Box frame is drawn as rects with or without sphere bases in the style"; "a Circle frame under sphere bases still blits one sprite per node" | `packages/graph-render/src/canvas2d/nodes.ts:135` (`impostorOf` returns false for a `Box` frame) |
| ST-3 | MAJOR | fixed | "a routed vertex outside the node hull is in the bounds a fit uses"; "a fit over a routed drawing keeps every interior vertex on screen" | `packages/graph-render/src/frame.ts:65` (`hullWith`, folded into `bounds` at `:107`) |
| ST-4 | MAJOR | fixed | "with no panels the safe area is the whole canvas"; "a dock on the right takes its 280px off the visible box"; "a fit centred in the safe box keeps the drawing out from under the dock" | `packages/graph-render/src/camera.ts:47` (`FitArea`/`FitOptions`), `:126` (`fitCamera` centres in `area`), `src/canvas2d/controller.ts:110` (`setSafeArea`); studio side `packages/graph-studio/src/ui/safeArea.ts`, wired at `src/element.ts:220` |
| ST-5 | MAJOR | fixed | 8 rows in `tests/ui/registry-gestures.test.tsx`, each firing the real gesture and asserting `seen.calls` is empty while the log carries the command | `packages/graph-studio/src/ui/Inspector.tsx:51,69,89`, `src/ui/NodeMenu.tsx:34`; new actions `view.unselect`/`view.pin`/`view.hide` in `src/actions/view.ts` |
| ST-6 | MAJOR | **deferred** | — | `packages/graph-render/src/camera.ts:78` — see "decisions needed" below |
| ST-7 | MINOR | fixed | "a degree 4 curve is not its control polygon: every chord is off the polygon"; "a degree 4 curve passes through its own midpoint, which the polygon does not" | `packages/graph-render/src/canvas2d/edges.ts:94` (`traceHigher`, de Casteljau in `src/edges2d/curve.ts`) |
| ST-8 | MINOR | fixed | "a 3D edge's stroke width follows style.edgeWidth and edges.scale" | `packages/graph-render/src/three/paint3d.ts` (`strokeWidth` reads `style.edgeWidth` over `drawn.ppu`) |
| ST-9 | MINOR | fixed | "a batch with one non-finite coordinate is refused whole" | `packages/graph-render/src/canvas2d/controller.ts:231` (`setPositions`) |
| ST-10 | MINOR | fixed | "two results fit at the renderer's ceiling, not at limits.max"; "a non-finite coordinate is refused, not fitted" | `packages/graph-studio/src/studio/fitResults.ts:50` (calls `fitCamera`; `cameraFor` deleted) |
| ST-11 | MINOR | fixed | "the same relative spread paints the same order at every scale"; "a frame past the bucket cap still paints by depth, not by dense index" | `packages/graph-render/src/three/sort.ts:43` (`depthOrder`; `bucketCount`/width at `:81`) |
| ST-12 | MINOR | **false** | — | Already covered on this tree: `app/.gitignore` lists `public/graph_wasm.wasm` and `public/fixtures/` (added in 9983b6f, before this job). `git status --porcelain` after `studio.sh wasm|check|build` shows nothing under `app/public`. The review's evidence was read on an earlier tree. |
| ST-13 | MINOR | fixed (doc) | — | The missing `Ponytail:` line is added to the block at `packages/graph-render/src/canvas2d/edges.ts:9` (behaviour unchanged; `tests/paint.test.ts:77` already pins the settling draw) |

## RED, per finding

- **ST-1** — `error: 'node 0 width 2.5 against 137.23434268773397'` (the disc's square, not `w x h`); `error: 'three interior points and the run to the target, not one chord  1 !== 4'`; `error: 'degree 2 is one curved edge  0 !== 1'`. 6 rows, `# pass 0 / # fail 6`.
- **ST-2** — `error: 'two rects, not two discs' / undefined !== 2`; `error: 'rects at 4096 nodes' / undefined !== 4096`.
- **ST-3** — `error: 'maxX 105.59461212158203 is short of the vertex at 2375.87890625'`; `error: 'vertex x -2171.8584995269775 is off the frame'`.
- **ST-4** — the module did not exist: `ERR_MODULE_NOT_FOUND … src/ui/safeArea.ts`; the ST-10 probe behind it `+ actual 29.76190476190476 / - expected 2`.
- **ST-5** — `error: the panel did not reach past the registry for the camera  + [ 'focus 1' ]  - []`; `+ [ 'togglePin 2' ]  - []`; `+ [ 'hide 2' ]  - []`. 3 rows, `# pass 96 / # fail 3`.
- **ST-7** — `error: 'midpoint missing: [[95,220],[170,220],[245,220],[320,20]]'` (it drew the control polygon); `expected: 16  actual: 4`.
- **ST-8** — `error: 'the drawing has a pixels-per-world-unit of undefined'`.
- **ST-9** — negative control re-run on the guard removed: `# pass 4 / # fail 1`, row 3 red, rows 1/2/4/5 green.
- **ST-10** — `+ actual 29.76190476190476 / - expected 2` and `The input was expected to not match /fitted to/. Input: 'fitted to 2 of 3 nodes'`.
- **ST-11** — `expected: [ 2, 1, 0 ]   actual: [ 0, 1, 2 ]` (dense-index fall-back inside one world unit); cap row `error: 'the furthest node is painted first'  3998 !== 3999`.

## Screenshots

Under `docs/measurements/fix-studio/`, driven in the real studio through `pw` on this worktree's
own dev server (the 5174 occupant mounts a different tree, so this one served on 5175 — recorded,
not a silent substitution). Console error list: **empty** (`Total messages: 3 (Errors: 0, Warnings: 0)`).

| file | what it shows |
|---|---|
| `fix-studio-st1-box-3d.png` | ST-1: a 3D Box frame (24 nodes) draws distinct `w x h` rects with rims, not discs; the drawing is centred in the safe area clear of the dock |
| `fix-studio-st1-polyline-3d.png` | ST-1: `post.route.grid` in 3D — the grid bends are drawn, where the review found 597 straight chords |
| `fix-studio-st1-curve-3d.png` | ST-1: `post.style.bezier` in 3D, degree 3 |
| `fix-studio-st1-box-2d.png`, `fix-studio-st1-curve-2d.png` | the 2D references the 3D path was matched against |
| `fix-studio-st2-box-impostor-budget.png` | ST-2: a Box frame of 40 nodes (well under `IMPOSTOR_BUDGET` 4096) with `style.spheres` set still draws treemap rects with rims, not round sprites |

## Commands and last lines

| command | exit | last lines |
|---|---|---|
| `scripts/studio.sh wasm` | 0 | `staged graph_wasm.wasm (1342752 bytes) and fixtures/ into app/public` |
| `scripts/studio.sh check` | 0 | `# tests 552 / # pass 552 / # fail 0 / # skipped 0`; render tests `# tests 98 / # pass 98 / # fail 0`; eslint clean; `✓ built in 386ms`; `[studio] ok` |
| `scripts/studio.sh build` | 0 | `dist/assets/studio-DTzgQihR.js 302.71 kB │ gzip: 94.32 kB` … `✓ built in 282ms` |
| `scripts/studio-smoke.sh` | 0 | 5 rows PASS: `smoke-no-exception`, `smoke-no-console-error`, `smoke-no-store-error`, `smoke-no-overlay`, `smoke-drew-nodes` (400 nodes drawn) |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 | all 5 rows FAIL (`graph_wasm.wasm 25 bytes served`) |
| `scripts/studio-nav.sh` | 0 | 18 rows PASS, incl. `nav-key-fit` (`edge pixels 45 → 0`), `nav-zoom-clamp` (`in: 40.0000, out: 0.0200`), `edge-gradient` |
| `STUDIO_NAV_BREAK=1 scripts/studio-nav.sh` | 1 | `edge-gradient` FAIL, `off by 83.7` |
| graph-render suite (`tests/*.test.ts`) | 0 | `# tests 423 / # pass 419 / # fail 0 / # skipped 4` |
| graph-studio suite (`tests/*.test.ts`) | 0 | `# tests 552 / # pass 513 / # fail 0 / # skipped 39` |
| `tsc --noEmit -p packages/graph-render/tsconfig.json` | 0 | — |
| `tsc --noEmit -p packages/graph-studio/tsconfig.json` | 0 | — |
| `eslint --max-warnings 0 app/src app/vite.config.ts packages` | 0 | — |

## Decisions taken

1. **ST-2, the impostor keeps the frame's node kind** (the job's judgement). `impostorOf` returns
   false for a `Box`; the sphere impostor is for `Point` and `Circle` only. The cached sprite is a
   disc of one radius and no rect impostor is baked, so a `Box` takes the flat rect path.
2. **ST-4, `view.fit` fits the filter's survivors and `showAll` fits the whole graph** (the job's
   judgement). The safe area is a `FitOptions.area`, measured at runtime by `safeArea.ts` — a
   `ResizeObserver` plus a `MutationObserver` for the conditional console. The pure half
   (`safeAreaOf`) is what the tests drive.
3. **ST-5, every gesture goes through `resolve`.** Three new additive action ids were needed
   because the gestures had no registered action to call: `view.unselect` (alias `unselect`,
   because `view.clear`'s `deselect` also leaves the local graph and the Inspector's × must not),
   `view.pin` (`pin`) and `view.hide` (`hide`). `ViewFace` gained `hide`/`togglePin`/`pinned` so
   an action has something to call. `pipeline.ts:156` is recorded **false**, not fixed: it is not
   a user gesture — `draw` is reached only from `arrange`, reached only from `apply`, which *is*
   a resolved action's body, and `Rig` carries no studio, so routing it would re-enter `resolve`
   mid-run.
4. **ST-7 case (b)**, a `Curve` whose point count disagrees with the declared degree stays a
   polyline through exactly the stored points. Every other option fabricates a control point the
   writer never stored; the polyline invents nothing. Marked `Ponytail:`.
5. **ST-10**, `maxScale` was left absent rather than passed as `limits.max`: `fitCamera` already
   clamps against `limits.max` internally, so passing it *raises* the ceiling to 40 and
   re-creates the defect. Documented at the call site.
6. The review's ST-11 "the `?: 0` on typed-array reads is dead code" is **wrong** and was not
   applied: `noUncheckedIndexedAccess` is on, so `depths[node]` is `number | undefined` to the
   checker. The narrowing was restored with a comment saying why (`sort.ts:106`).

## Deviations

- `packages/graph-studio/tests/filters-actions.test.ts` — 3 lines added to `silentView` (a full
  `ViewFace` literal; tsc fails without them). Later moved verbatim into the new
  `tests/silent-view.ts` to get the file back under the 300-line limit.
- `packages/graph-studio/tests/ui/zzprobe.test.tsx` — **deleted**. A throwaway diagnostic from
  the ST-5 slice that the orchestrator committed mid-task (3d1d516); it asserted nothing and failed
  lint with 6 errors.
