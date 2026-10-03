# Job perf-p6 (agent build; P6 of `prompts/perf-plan.md`, branch perf-p6, merged into develop by the orchestrator)

Why: at 4000 nodes the studio re-renders every React component on every store change and on every
16 ms force bar, recomputes O(n) and O(m) derivations on each render, and throws
`InvalidSessionError` when a graph is replaced while the force loop is running. P6 makes the UI do
work only when its inputs change. The motor (`crates/`) and the renderer (`packages/graph-render`)
belong to other jobs: do not touch them.

Facts (verified on develop a2c8569, in this worktree):
- Baseline (`scripts/studio-perf.sh --label p6-base --cases 1000,2000,4000`, `target/studio-perf/p6-base/table.md`):
  opening a graph costs **17 React commits and 3875 component renders** at 1000, 2000 and 4000 nodes alike.
  Open ms 202 / 815 / 3058 (most of it is the motor's default layout, not React; leave the layout alone).
- `src/state/store.ts` `createStore`: one value; every `set` notifies every listener.
- `src/ui/useStudio.ts`: `useStudioState(studio)` returns the whole `StudioState`. There is no selector.
- `src/ui/Shell.tsx` (about lines 64-102) subscribes to the whole state and passes `state` to Inspector,
  Toast, Dock, Legend, Hud, NodeMenu and Console; Search gets `meta`. No component uses `React.memo`,
  and the package has no `useMemo`.
- `src/ui/ForcesPanel.tsx` (about lines 108-125): `useSyncExternalStore(bar.onBar, bar.bar, bar.bar)` at the
  panel level, so the whole panel re-renders on every bar update (one per force frame, about every 16 ms).
- `src/ui/Legend.tsx`: `countOf(meta, group)` walks every node for each group on every render, and
  `legendOf` is recomputed on every render.
- `src/studio/pipeline.ts:246` `neighboursOf(ends, node)` scans every edge for each call.
- `src/motor/liveSession.ts` `rowOf` uses `ids.indexOf(id)`: O(n) per pin and per drag move.
- The InvalidSessionError, root cause:
  - `src/motor/session.ts` `replace()` builds the next graph and then calls `forget()`, which runs
    `built?.forced?.release()` and sets `built.forced = null`, `built.port = null`.
  - `src/motor/liveLoop.ts` `createForceHost` (lines 170-196) halts the old `ForceLoop` only inside `handle()`,
    that is, only when the next force *request* arrives.
  - Meanwhile the old loop's scheduled `frame()` (lines 75-86) still runs: `this.batch()` steps a released
    session, and the SDK (`crates/graph-sdk-js/src/force.ts` `#dead()`) throws `InvalidSessionError`.
  - `forcesOf()` (session.ts:186-200) creates the force session lazily (`built.forced ??= motor.forceSession(...)`);
    calling it only to check liveness would create a session as a side effect. Do not do that.
- Tests: `node:test` with `node:assert/strict`, under `packages/graph-studio/tests/` (motor-free) and
  `tests/ui/` (React markup through `tests/ui/desk.ts`: `markup`, `studioWith`, `DRAWN`, `META`, `IDLE`).
  The loop tests are in `tests/force-loop.test.ts` with a fake port (`fake(decay, clock)`).
  Files named `*.motor.test.ts` need the wasm; leave them as they are.
- The package has no `node_modules`; it is checked with the toolchain pinned in `app/package.json`.
  The only check command is `scripts/studio.sh check` (tsc, unit and render tests, eslint, vite build).

Do, in this order. One task at a time: run `scripts/studio.sh check` after each one and fix it before the next.
1. Read every file named above first, plus `src/studio/studio.ts` and `src/motor/worker.ts`.
2. **InvalidSessionError.** The loop must never tick a released session.
   - Have the session tell the host when its force session goes away: for example an `onForget` callback in
     the session's deps, or a `release` notice the force host listens to, that halts the loop and clears it
     at once. Choose the smallest change that fits the existing code, and make no session as a side effect.
   - Belt and braces: `ForceLoop.frame()` must stop, without throwing, if its port was released (`halt()`
     already exists).
   - Regression test in `tests/force-loop.test.ts`: start the loop on a fake port, release or replace the
     session, run the scheduled frame; expect no throw, no `step` call on the old port after the release,
     and the loop stopped. The test must fail against the code before your fix: check that once, by hand,
     and say so in the report.
3. **Selectors.** In `src/ui/useStudio.ts`, add
   `useStudioSelector<T>(studio, select: (s: StudioState) => T, isEqual: (a: T, b: T) => boolean = Object.is): T`
   over `useSyncExternalStore`, caching the last selected value so that an equal selection returns the same
   reference (otherwise React loops). Keep `useStudioState` for the callers that really need everything.
   - Shell and each panel read only the slices they draw. Wrap the panels in `React.memo`.
   - UI test in `tests/ui/`: changing a field a panel does not read (for example `selected`, for Legend and Hud)
     does not re-render it. Count renders with a wrapper or React's `<Profiler>`; do not add a dependency.
4. **Derived data, memoized on identity.**
   - Legend: counts and `legendOf` computed once per (`meta`, `settings.groups`) with `useMemo`, or a
     `WeakMap` keyed on `meta` when the value is shared. One pass over the nodes for all groups, not one per group.
   - Search: the index built once per `meta` (`Search.tsx`, `matches.ts`), not on each keystroke.
   - `neighboursOf`: replace the O(m) scan with an adjacency built once per graph (a `WeakMap` keyed on the
     edge ends array), with the same output order as today. Add a unit test that compares the old order on a
     small graph with duplicate and self edges.
   - Every cache carries a `Caveat:` comment saying what it gets wrong (for example: keyed on identity, so an
     equal but new object misses).
5. **ForcesPanel.** Move the bar subscription into a small child component that renders only the bar, and
   throttle what it reads to at most one update per animation frame (`requestAnimationFrame`, with a
   fallback when it is absent in tests). The rest of the panel must not re-render on a bar update.
6. **`liveSession.ts` `rowOf`.** A `Map<string, number>` built once per order instead of `indexOf`.
7. **Measure.** `scripts/studio.sh build`, then
   `scripts/studio-perf.sh --label p6-after --cases 1000,2000,4000` (it exits 1 with `--cases`; that is expected,
   read `target/studio-perf/p6-after/table.md`). Compare the React commits and renders at open with the baseline
   above. Write `docs/reports/perf-p6.md`: before and after table, the commands, the files changed, and what was
   not done, with the reason.

Paths you may change: `packages/graph-studio/**` (sources and tests), `docs/reports/perf-p6.md`.
Do not change: `packages/graph-render/**`, `crates/**`, `app/**`, `deploy/**`, `scripts/**`, any
`package.json` or lockfile, `.claude/**`, or the default layout id in `src/state/settings.ts`. Add no dependency.
If a task cannot be done inside these paths, stop that task, say which file you would need, and go on with the next.

House limits: functions of at most 40 lines, at most 4 parameters, files of at most 300 lines (split into a
child module when a file would grow past it). No `eslint-disable`. No `any`. Comments say why, not what.

Done when:
- `scripts/studio.sh check` exits 0, with no skipped test;
- `scripts/studio-smoke.sh` exits 0;
- `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits non-zero;
- `docs/reports/perf-p6.md` holds the before and after numbers.

The return block pastes each of those commands with its real exit code, the React commits/renders line from
`p6-after/table.md`, and the list of changed files. A command you did not run is written "not run".
