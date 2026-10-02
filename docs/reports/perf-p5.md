# Perf P5 report — the GPU layer, 1M nodes that pan and zoom

Shape: `prompt.md` §12. Plan: `prompts/perf-plan.md` P5. Branch `perf-p5` (with the gate slice
`perf-p5-gate` merged into it), worktree `$GM_SCRATCH/wt/perf-p5`, base develop `4b8023b`. The
numbers are in `docs/measurements/perf-p5.md`; this report covers what changed and what was checked.

## 0. What this phase was

Make a 1M-node graph pan and zoom in the studio. The 2D painter keeps small scenes; from
`BULK_THRESHOLD` nodes plus edges (or on `?backend=webgl2`) a WebGL2 layer on an OffscreenCanvas
draws the edges and nodes, and the 2D canvas blits it. Under SwiftShader, panning went from
17.3 to 59.3 fps at 1M and from 20.5 to 58.5 fps at 200k; Canvas2D went from 19.8 to 50.3 fps
at 20k. One exit target is missed by 1.5 fps (WebGL2 ≥ 60 at 200k), and every exit number comes
from a CPU rasteriser, not a GPU.

## 1. Authorization compliance

| Plan item (P5 Changes) | File | Status |
|---|---|---|
| Backend chosen by feature detection, each failure falls through | `webgl2/plan.ts` (`backendOf`, `bulkWanted`), `webgl2/hook.ts` (`failure`), `graph-studio/src/element.ts` | done for WebGL2 → Canvas2D |
| WebGPU backend | – | **not built** (deviation 1) |
| One positions buffer updated in place | `webgl2/sync.ts` (re-uploads only changed columns) | done |
| Instanced quads for nodes, edges as index pairs | `webgl2/layer.ts`, `draw.ts`, `shaders.ts` | done: points below a size, instanced quads over on-screen nodes above it or when a moving frame's on-screen nodes fit its budget |
| LOD: points below a pixel threshold, edges density-binned | `webgl2/plan.ts` (spread order, moving budget) | points done; edges are a spread-order prefix, not density bins (deviation 2) |
| Labels: culled top-K | `labels.ts` (`followLabels`), `canvas2d/loop.ts` | the existing culled plan, followed under the camera from 131 072 nodes |
| GPU ID-buffer picking | – | **not built** (deviation 3) |
| Canvas2D cheap wins | `canvas2d/pace.ts`, `edges.ts` | moving edge budget paced by the frame gap; the edge-plan cache and the impostor LRU **not done** (deviation 6) |
| Ingest at 1M (needed to open 1M at all) | `graph-wasm/src/ingest/ids.rs`, `at.rs`; `graph-sdk-js/src/wasm.ts` | done (deviation 4) |
| Gate: parity at 2k, fallback rows, negative control | `scripts/studio-backend.sh`, `deploy/nav/backend*.py`, `imagery.py` | done |

Deviations, each named:

1. **No WebGPU.** The container has no GPU, so a WebGPU backend could not be measured here, and
   the plan's host-browser measurement has no host browser on this machine. Not attempted; the
   chain is WebGL2 → Canvas2D.
2. **Edges are a prefix, not density bins.** A moving frame draws the first `budget` edges of a
   spread order whose every prefix covers the graph evenly (`webgl2-pace.test.ts`); a settled
   frame fills every edge into a kept picture over several frames (`still.ts`).
3. **No GPU picking.** Hover and click keep the 2D uniform grid. Not measured as a bottleneck.
4. **Changes outside `packages/`.** Opening 1M nodes failed twice before any drawing: the SDK read
   wasm `u32` results as signed `i32` past 2 GiB ("Offset is outside the bounds of the
   DataView"), now read through `>>> 0` once in `wasm.ts`; and ingest validation spent 2.1 s of a
   15 s open in `BTreeSet<&str>`, now a `StringArena`. Neither touches graph-core.
5. **What a moving frame leaves out.** It draws no labels; edges are thinned to the budget; a
   glide (`glide.ts`) shows the kept picture magnified up to √2 softer, or bare background over up
   to a fifth of the view, until the next fresh frame. Each is a `Caveat:` in its file.
6. **Two Canvas2D items left.** Pacing the moving budget took 20k from 19.8 to 50.3 fps, past
   the 30 fps exit, so the edge-plan cache (`edges.ts`) and an LRU bound on the impostor cache
   (`impostors.ts`) were not started. Label sprites were already LRU-bounded (`sprites.ts`).

## 2. The ledger diff

None. No layout, analysis or post entry was added or changed.

## 3. The gate table

Studio rows, on the worktree before landing; the merge floor (quick.rows, with hashgate 8 and its
negative control) runs in `land.sh perf-p5` on the merged tree.

| Row | Expect | Exit | Verdict | Evidence |
|---|---:|---:|---|---|
| `studio.sh check` (tsc, 515 + 61 tests, 0 skipped, eslint `--max-warnings 0`, vite build) | 0 | 0 | PASS | `target/p5-check.log`, commit `daa45a0` |
| `sdk:typecheck` | 0 | 0 | PASS | `target/p5-sdk.log` |
| `studio-backend.sh` (6 rows) | 0 | 0 | PASS | `target/backend.log`, commit `5be9985` |
| `STUDIO_BACKEND_BREAK=1 studio-backend.sh` | non-zero | 1 | PASS | `target/backend-neg.log` |
| zoom probe errors (6 runs at 1M) | none | none | PASS | `target/zoom-{glide,noglide}-{1,2,3}.log` |
| `studio-backend.sh` shellcheck | 0 | 127 | **not run** | shellcheck is not in `ge-rust` |

## 4. The 4-way hash table

graph-core is unchanged. The wasm ingest changes refuse the same documents with the same
messages (`graph-wasm/src/ingest/tests.rs`, `at.rs` tests) and build the same topology, which
`hashgate-8` in `land.sh`'s rows checks native against wasm32.

## 5. Coverage

| Changed symbol | Exercised by |
|---|---|
| `backendFor`, element lists, colours (`plan.ts`, `colour.ts`) | `webgl2-plan.test.ts` |
| spread order, `nextBudget`, `largestHalf`, `onScreen` (with its early stop), `gathered` | `webgl2-pace.test.ts` |
| `paced`, `worthPacing` | `pace.test.ts` |
| `landing` (glide) | `webgl2-glide.test.ts` |
| still picture restart rules | `webgl2-still.test.ts` |
| `followLabels` | `labels.test.ts` |
| the GL calls (`layer.ts`, `draw.ts`, `sync.ts`, `still.ts` fill) | the backend gate's parity, drawn and context-lost rows; the 1M zoom screenshots |
| ingest ids and paths | `graph-wasm` ingest tests |
| SDK `>>> 0` on results | only the 1M opens (perf and zoom runs); no test reaches 2 GiB |

## 6. Caveat / Ponytail markers added

`plan.ts` (`BULK_THRESHOLD`), `layer.ts` (one-pixel lines, straight edges, blending), `still.ts`
(one dim alpha, the fill prefix), `glide.ts` (softness, bare area), `draw.ts` (the on-screen
pass), `canvas2d/pace.ts` (a GC halves the budget), `canvas2d/loop.ts` (`FOLLOW_FROM`
estimated), `backend.py` (the 2 % parity ceiling), `deploy/perf/zoom.py` (timer cap, GC in the gaps,
SwiftShader only ranks builds).

## 7. What could not be verified

| Item | State |
|---|---|
| WebGL2 ≥ 60 fps at 200k | **missed**: 58.5 worst pan under SwiftShader at load 10 |
| Any number on a real GPU | not measured: no GPU in the container |
| WebGPU | not built (§1, deviation 1) |
| shellcheck on `studio-backend.sh` | not run |
| Settled-frame fill at 1M | 10.5–11.1 s from open to a full picture; the view moves meanwhile, but the edges arrive over that time |

## 8. Stop-and-ask items

None. The ingest and SDK changes were the only way to open 1M nodes and change no motor byte.
