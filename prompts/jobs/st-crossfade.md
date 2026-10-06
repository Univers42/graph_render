# Job st-crossfade (agent build, studio transitions)

Why: the user, 2026-10-06: "the transitions between shapes, edges, colors and more should be faster".
A colour, filter, theme or reveal change swaps the drawing in one frame: `restyle`
(`packages/graph-studio/src/studio/pipeline.ts:105-110`) calls `view.setStyle`, and nothing eases it.
Opening another document (`draw`, `pipeline.ts:141-168`, `shown.fresh === true`) passes `animate: false`
(`pipeline.ts:151`), so the old drawing is replaced by the new one in one frame too.

Do: a screen-space cross-fade, additive to the View API.
1. RED first: a render test in `packages/graph-render/tests/` (node:test, the style of
   `tests/focus-tween.test.ts`) that calls `crossFade()` and then requires that frames inside the fade
   keep invalidating and that the fade ends by itself after its duration.
2. New module `packages/graph-render/src/canvas2d/crossfade.ts` (`src/fade.ts` is taken: that is the
   dim fade). It holds: `startCrossFade(state, ms)` — copy the 2D canvas (`state.ctx.canvas`) into an
   `OffscreenCanvas` of the same size with `drawImage`, store it with its start time; `paintCrossFade(state,
   now)` — `drawImage` the copy over the fresh frame at `globalAlpha = 1 - easeInOutCubic(t)`
   (`src/transition.ts`), in device pixels (reset the transform first, restore after); `crossFading(state,
   now)` — true while it runs; it drops the copy when done. Default duration 220 ms.
3. `canvas2d/loop.ts`: in `paint()` (`loop.ts:172-186`) call `paintCrossFade` after `paintFrame` and before
   `paintOverlay`; in `renderFrame` (`loop.ts:218-235`) add `crossFading(state, performance.now())` to the
   re-invalidate condition at line 233. `LoopState` (`loop.ts`) gains one field for the fade; its
   constructor is `newState` in `canvas2d/controller.ts:84-102` — set it there.
4. `View` (`src/view.ts:118`) gains `crossFade(ms?: number): void`. `view.ts` is at the 300-line cap:
   move a self-contained block out into a child module first (no behaviour change), then add the method.
5. Studio: `ViewFace` (`pipeline.ts:29-36`) adds `"crossFade"`. Call `rig.view.crossFade()`:
   - in `look` (`pipeline.ts:283-287`) before `showLook`;
   - in `reveal` (`pipeline.ts:289-292`) before `restyle`;
   - in `measure` (`pipeline.ts:213-235`) before each `restyle`;
   - in `drawOut` (`pipeline.ts:271`) before the lone `restyle`;
   - in `draw` before `setFrame` only when `shown.fresh` is true.
   Never on a layout draw (`shown.fresh === false`): the node tween already moves those, and a ghost of
   the old layout over a moving one reads as a double image.
6. Every fake View the studio tests build (`git grep -n "setStyle:" packages/graph-studio`) gains a
   `crossFade` no-op so the type checks.

Caveat to write in `crossfade.ts`'s header: the copy is screen-space, so a camera move during the fade
(the fit after a document switch) slides the new drawing under a still ghost for at most 220 ms.

Paths: `packages/graph-render/**`, `packages/graph-studio/**`. Nothing else.
Limits: ESLint `--max-warnings 0`, 300 lines per file, 40 lines per function, 4 parameters, no type
assertions. If an edit fails twice, read the whole file and write it once.

Done when: `scripts/studio.sh check` exits 0 with the new test passing; `scripts/studio.sh build &&
scripts/studio-smoke.sh` exits 0; `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits non-zero.
Confirm with the `pw` MCP on the dev server that a theme change fades (screenshot mid-fade to /out/).
