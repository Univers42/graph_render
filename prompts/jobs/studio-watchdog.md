# Job studio-watchdog (agent build, studio follow-up to studio-live)

Why: studio-live (landed 2026-10-01) reported one HIGH finding it did not fix. If the motor worker dies
mid-settle, the progress strip stays visible forever and the forces panel keeps claiming a live session
(`packages/graph-studio/src/motor/bridge.ts`, `src/element.ts`).

Do:
1. Read `bridge.ts`, `liveLoop.ts`, `liveSession.ts`, `ui/progress.ts` and `ui/ProgressBar.tsx` yourself first.
2. Add a watchdog: when no force-state message arrives for a bounded time while the strip is shown, hide
   the strip, mark the session dead and show one console line naming the cause. The bound goes in one
   named constant with a `Caveat:` line (a slow but live motor on a very large graph can trip it).
   A worker `error` event triggers the same path at once.
3. A unit test in `packages/graph-studio/tests/` with a fake worker that goes silent and one that errors.
4. A row in `scripts/studio-live.sh` (`live-dead-worker`): terminate the worker over CDP mid-settle and
   assert that the strip is hidden within the bound. `STUDIO_LIVE_BREAK=1` must still turn every row red.

Paths: `packages/graph-studio/**`, `scripts/studio-live.sh`, `deploy/nav/**`. Nothing else.

Done when: `scripts/studio.sh check` exits 0, `scripts/studio.sh build` then `scripts/studio-live.sh`
exits 0 with the new row PASS, and `STUDIO_LIVE_BREAK=1 scripts/studio-live.sh` exits non-zero. The return
block pastes each command's real exit code.
