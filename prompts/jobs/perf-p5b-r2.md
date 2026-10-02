# Job perf-p5b, round 2 (same session, same worktree): act on your two decisions

Your round 1 holds: the GPU picture lost (median 77 048 ms against 36 995 ms kept-2D at 1M), it stays reverted,
and the probes plus `docs/measurements/perf-p5b.md` are the deliverable so far. Both decisions are approved as
you recommended. This round must end in a commit past f0fd331.

1. `view.stats()` exposes the still's state (`refining: boolean`, or the name the stats already use for that
   kind of flag). `deploy/perf/probes/settle.js` waits on it instead of comparing `drawnEdges` against `edges`.
   Add a unit test that the flag is true while chunks remain and false once the picture is full.
2. The fill is 977 frames × ~42.5 ms because the chunk is pinned at `MOVING_FLOOR` 2048 (`webgl2/plan.ts:117`):
   the per-frame fixed cost (one `transferToImageBitmap`) always exceeds `SLOW_MS`. The lever is the number
   of readbacks. First time a frame that draws chunks without the readback, and paste the number. Then give the
   still its own floor or budget, so it draws many chunks per readback and only the final frame, or one frame
   every K ms, shows the picture. Constraints:
   - Input still wins. A pan, zoom or drag during the fill drops the still at once, as today. Show it with a
     probe that pans mid-fill and reads the frame time.
   - The picture's pixels are unchanged. `scripts/studio-backend.sh` parity stays at its current threshold.
   - Each heuristic (floor, budget, K) carries a `Ponytail:` or `Caveat:` line naming what it gets wrong.
3. Re-measure with `deploy/perf/settle.py` at 1M, 3 interleaved rounds before and after (`ab.sh`), and at 200k.
   Keep the change only if the median time to a full settled picture at 1M drops by more than 3%, with no
   frame-time regression on the mid-fill pan. Otherwise revert it and record the numbers.

Done when: perf-p5.rows green (`scripts/studio.sh check`, `scripts/studio-backend.sh` and its BREAK negctl,
`scripts/studio-smoke.sh` and its BREAK negctl). Paste every exit code, the before and after tables, and the
screenshot path with its error read.
