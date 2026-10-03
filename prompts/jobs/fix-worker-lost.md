# Job fix-worker-lost (agent build; a false MotorWorkerLost on small graphs)

Source: `docs/measurements/perf-p5c.md` §7 item 6: on both arms, graphs of ≤ 2 000 nodes raise the
`MotorWorkerLost` store error, and the fill never reports full; `scripts/studio-probe.sh settle 2000 webgl2 <tag>`
still exits 0 through it. A person sees a red error banner on a small graph that is fine.

Facts:
- The error is raised by `note` in `packages/graph-studio/src/studio/studio.ts:155`, fed by the
  watchdog `packages/graph-studio/src/motor/watchdog.ts`: `touch()` re-arms a `SILENCE_MS` = 4 000 ms
  bound on each worker message, `rest()` stops it, `lost()` fires at once on a worker `error` event.
  The wiring is in `packages/graph-studio/src/motor/bridge.ts:105,222` and `src/element.ts:158,197,243`.
- Hypothesis, unverified: a small graph's settle ends, the worker goes quiet as it should, and nothing
  calls `rest()`, so 4 s later the silence reads as death. Step 1 settles it.

Do:
1. Reproduce: run the settle probe at 2 000 nodes and read the store error, the console and the
   overlay text (a title alone is not evidence). Paste the error's `detail` (silence or worker failed).
2. RED: a unit test with the watchdog's injected `schedule` that drives the real sequence (session
   starts, messages, settle ends) and expects no `lost`. GREEN: fix the root cause in the one place
   that owns the session's end (bridge or watchdog), not a per-caller patch. A truly silent dead
   worker must still be declared lost: keep or add that test.
3. Make the error a failure where it was missed: the settle probe (or `scripts/studio-smoke.sh`,
   whichever reads the store) exits non-zero when the store holds an error after the run, with a
   negative control that injects one. Do not edit `deploy/perf/probes/settle.js`'s `maxFrameMs` or its
   pixel hash (perf-p5d owns those lines); add the store read beside them.
4. Re-run step 1: no error at 120, 2 000 and 10 000 nodes. Paste the three runs.

Out of bounds: `crates/`, `packages/graph-render/src/webgl2/` (perf-p5d). House limits as in
`prompts/jobs/fix-common.md`.

Done when: `scripts/studio.sh check` 0, `scripts/studio-smoke.sh` 0 and `STUDIO_SMOKE_BREAK=1` non-zero,
the new store-error negctl non-zero, `scripts/orch/rows/perf-p5.rows` green.
