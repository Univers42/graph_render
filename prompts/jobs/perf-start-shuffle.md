# Job perf-start-shuffle (agent build: "Animate" really restarts the settle, in O(n), with no snapshot)

Goal: Animate (`force.start`) restarts the live settle from the session's own start positions, the
way Obsidian's Animate replays its layout. Today it runs a whole layout whose result nothing reads,
and then only reheats.

Facts (verified on develop b2cbbcd9):
- `packages/graph-studio/src/motor/liveSession.ts:90-95` `shuffle` calls `deps.scatter(deps.handle)`
  and then `session.reheat(1)`.
- The scatter is `motor.layout(handle, SCATTER)` (`session.ts:202`, with `SCATTER = "layout.random"`
  at `settle.ts:16`). It writes the graph handle's layout, not the force session's columns. The
  session owns its own columns and seeds them itself: `ForceSession::new` → `seeded` →
  `Sim::new(topology, params, SEED)` (`crates/graph-core/src/layout/force/session.rs:114,133`).
  So the scatter is dead work: O(n) plus the layout stage at 1M, for nothing. Animate is a plain
  reheat from where the nodes are.
- The comments that say otherwise are false:
  - the `shuffle` doc at `live.ts:44-47` ("Throws the nodes back to random positions");
  - the Ponytail at `liveSession.ts:10-11`;
  - `liveLoop.ts:187-190`.
- The ABI already has every verb a real restart needs, so no ABI change is involved:
  - `gm_force_session_release` (`crates/graph-wasm/src/exports/session.rs:221`);
  - `gm_force_session_create` and `gm_force_session_create_mesh` (`:39`, `:48`);
  - `gm_force_session_set_params` (`:84`).
  A new session starts at the deterministic seed positions.
- The studio creates the session once per built graph: `built.forced ??= motor.forceSession(handle,
  undefined, built.engine)`, and caches the port, because the loop compares ports by identity
  (`session.ts:194-203`). `forget()` in `session.ts` is the existing release path.

Do, in order:
1. Write a failing test first, in `packages/graph-studio/tests/live-session.motor.test.ts` (it runs
   the real motor):
   - Create a session. Tick k = 20. Call `shuffle()`.
   - Assert that `positions()` now equals a fresh session's start positions, byte for byte.
   - Assert that the alpha it returns is that fresh session's alpha.
   - Assert that the knobs set before the shuffle are still in `params()`.
   - It must fail on develop.
2. Replace `scatter` in `MotorForceDeps` with a `restart` dependency. `restart` releases the current
   motor session and returns a new `ForcePort` over the same handle and engine. Inside
   `createLiveForce`, the session becomes the one the latest restart returned. `shuffle()` then:
   - calls `restart`;
   - re-applies the current knobs with `setParams`;
   - reapplies nothing else (pins are dropped by the loop, `liveLoop.ts:190`);
   - returns the new session's alpha.
   The `LiveForce` object keeps its identity, so the loop does not restart itself.
3. In `session.ts` `forcesOf`:
   - Supply `restart` through the existing create and release calls, and update `built.forced`.
   - Delete the `scatter` dependency and the `SCATTER` import if nothing else uses them. `settle.ts`
     keeps `SCATTER` for `planRun`.
4. Correct the three false comments to say what happens now.
5. Measure Animate at 1M in the browser, before and after, with performance marks around
   `force.start` handling in the worker. Run 3 alternated rounds on the gm-chromium hardware arm,
   claim medians only, and print the load average.

Out of bounds: `crates/` (no ABI change is needed: if you find one is, stop and report),
`packages/graph-render`, `pipeline.ts` (perf-p6-data-path), `liveLoop.ts` beyond its comment.
No new dependency. House limits: 40 lines a function, 300 a file, 4 parameters, no type assertions.

Paths you may edit: `packages/graph-studio/src/motor/{liveSession,live,session}.ts`, the comment
at `packages/graph-studio/src/motor/liveLoop.ts:187-190`, `packages/graph-studio/tests/`,
`docs/measurements/perf-start-shuffle.md`.

Done when:
- The step-1 test fails on develop and passes on the branch.
- `scripts/studio.sh check` exits 0.
- `scripts/studio-live.sh` exits 0 and `STUDIO_LIVE_BREAK=1 scripts/studio-live.sh` exits non-zero.
- `scripts/studio-smoke.sh` exits 0 and `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits non-zero.
- `docs/measurements/perf-start-shuffle.md` holds the before/after table with its load and states
  what the change does not do (the first load still builds a snapshot: perf-snapshot-build).
