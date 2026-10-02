# Job perf-p5b (agent build; 1M nodes settle fast: the GPU layer keeps its picture on the GPU)

Why: the goal is 1M nodes fast and interactive. After perf-p5 (`docs/measurements/perf-p5.md`), panning
at 1M runs at 59.3 fps and zoom glides at 50.9/42.0 fps, but the **first settled picture at 1M takes
10.5-11.1 s** to fill, and 200k pans at 58.5 fps against a 60 target. Scope: `packages/graph-render`
only (the motor, the studio and `app/` belong to other jobs).

Facts (develop):
- `packages/graph-render/src/webgl2/still.ts` (108 lines, read its header): a settled frame draws one
  chunk of edges with WebGL2, then copies it to a kept 2D `OffscreenCanvas` through
  `transferToImageBitmap`; before the chunking, 99% of a 3.9-4.5 s settled frame was inside
  `transferToImageBitmap` (SwiftShader). The chunk is paced by `nextBudget` (`webgl2/plan.ts`).
- Moving frames: `webgl2/glide.ts` (glide over the kept picture), `webgl2/draw.ts` (budgeted prefix).

Do:
1. Baseline first, on this tree, interleaved with your build later: time-to-complete-settled-picture at
   1M (add the probe to `deploy/perf/` in the style of `zoom.py`: open 1M on `layout.random`, wait for the
   still to report full, print ms), and `studio-perf.sh --cases 200000,1000000 --backend webgl2`.
2. Keep the settled picture on the GPU: accumulate the edge chunks into a framebuffer texture (no clear
   between chunks, one textured quad to show it; the nodes in a second texture), and drop the
   `transferToImageBitmap` copy. The dim (focus/fade) becomes the quad's alpha, as today. A lost
   context must still fall back to Canvas2D (`scripts/studio-backend.sh` checks it). If texture size
   limits bite (`MAX_TEXTURE_SIZE`), say how you handle it.
3. Only then, if time remains: the 200k pan gap (58.5 → 60), with a profile that names the row.
4. `docs/measurements/perf-p5b.md`: before/after, 3 interleaved runs each, commands, load average.

Every UI claim needs a screenshot and an error read (store error, console exceptions, overlay text);
title-only checks are not evidence. SwiftShader numbers rank builds on this host only: say so.

Done when: `scripts/orch/rows/perf-p5.rows` green (studio check, backend gate and its negctl, smoke and
its negctl), the settled-picture time at 1M measured before and after, and the measurement committed.
If the GPU picture does not win, revert the code and commit the numbers: they are the result.
