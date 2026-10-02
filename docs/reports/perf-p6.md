# Perf P6 report — the studio draws only what changed

Branch `perf-p6`, base develop `a2c8569`, measured on `a801edd`. Plan: `prompts/perf-plan.md`
P6. Everything below was measured in Docker through `scripts/`; no result is quoted from a
run this job did not make.

## 0. What this phase was

At 4000 nodes the studio re-rendered every React component on every store change and on every
16 ms force bar, recomputed the O(n) and O(m) derivations on each render, and threw
`InvalidSessionError` when a graph was replaced while the force loop was running. P6 makes the
UI do work only when its inputs change.

The one number that is not noise: **the React renders at open fell from 3875 to 672**, at
every node count, with the render count independent of the graph size because it never was —
it was a fixed number of components redrawn per commit.

## 1. Before and after

Both runs: `scripts/studio.sh build`, then `scripts/studio-perf.sh --label <name> --cases
1000,2000,4000` (exits 1 by design with `--cases`; the table is the result).

| nodes | open ms (base → after) | JS mean ms | JS p95 ms | worst fps | longest ms | **React commits / renders at open** |
|---:|---|---|---|---:|---|---|
| 1000 | 202 → 283 | 1.007 → 1.100 | 2.0 → 3.2 | 11.5 → 11.5 | 282 → 290 | **17 / 3875 → 18 / 679** |
| 2000 | 815 → 854 | 1.165 → 1.320 | 2.3 → 2.7 | 6.0 → 6.0 | 576 → 580 | **17 / 3875 → 18 / 672** |
| 4000 | 3058 → 3207 | 1.875 → 2.147 | 2.9 → 3.8 | 4.5 → 4.3 | 1236 → 1380 | **17 / 3875 → 18 / 672** |

Source tables: `target/studio-perf/p6-base/table.md` (commit `a2c8569`) and
`target/studio-perf/p6-after/table.md` (commit `a801edd`). `perf-js` (p95 JS ≤ 4 ms at 2000
nodes) reads 2.3 ms PASS before and **2.7 ms PASS** after.

**Read the ms columns with care.** They are dominated by the motor's own layout, which P6 does
not touch, and the host is shared: four after-runs of this code gave open ms
384 / 1092 / 4643, then 225 / 1264 / 3191, then 281 / 856 / 3707, then 283 / 854 / 3207. The
three earlier ones were taken while other jobs were building in Docker, and the first of them
put `perf-js` at 4.6 ms **FAIL**; re-run alone, the same code reads 2.7 ms PASS, which is what
the ponytail note in `scripts/studio-perf.sh` says to do with a red perf row on a loaded host.
The table above is the alone run.

The render count is the reliable signal and it reproduced exactly across three after-runs
(679 renders at 1000 nodes, 672 at 2000 and 4000). An intermediate run, before the dock's
action forms were compared on what they draw, read **3564 renders** — that is where most of
the win came from. The last change to land, `useStudioSelector` passing its selection as the
`useSyncExternalStore` snapshot instead of the whole state, left the counted renders at 672:
the commits it saves are ones the reveal animation was already making.

## 2. What changed

### InvalidSessionError — the loop must never tick a released session

- `src/motor/live.ts`: `LiveForce.dead`, set by the session when its force session goes.
- `src/motor/session.ts`: `SessionDeps.onForget` and a module-level `forget()` that marks the
  port dead **before** `release()`, then releases, then tells the host. It never creates a
  session to ask whether there is one.
- `src/motor/liveLoop.ts`: `ForceHost.forget()` halts the loop and drops it at once;
  `ForceLoop.frame()` returns without touching a dead port; `halt()` does not unpin a dead one.
- `src/motor/worker.ts`: wires the notice over one cell, because the session is made before
  the host that could stop it.
- Regression test in `tests/force-loop.test.ts`, **observed RED against the pre-fix code by
  hand**: the first new test threw `InvalidSessionError: step on a released session` out of
  the frame that was already scheduled, and the second failed on `host.forget is not a
  function`. Both are the production bug, not a compile error.

### Selectors, slices and memo

- `src/ui/useStudio.ts`: `useStudioSelector(studio, select, isEqual)` over
  `useSyncExternalStore`, holding the last selected value so an equal selection keeps its
  reference; `sameWhenEqual` is the rule, exported for its test. `useStudioState` stays for
  the two callers that really need everything.
- `Shell.tsx` reads eight slices (`useSlices`) instead of the whole state; `Legend`, `Hud`,
  `Inspector`, `Toast`, `NodeMenu`, `Search` are `React.memo` and take only what they draw.
  `Dock` and `Console` subscribe for themselves: every action's `available` and every
  parameter's `value` are arbitrary functions of the whole state.
- `useShortcuts` takes `busy: boolean` instead of the state.

### Derived data, memoised on identity

- `Legend.tsx`: one pass over the nodes for all groups, each query parsed once; the counts and
  `legendOf` behind a `WeakMap<GraphMeta, …>` that also compares the settings it was built
  from, so a miss is a rebuild and never a stale read.
- `matches.ts` / `Search.tsx`: the lowercased label index behind a `WeakMap` keyed on
  `meta.labels`; a keystroke re-ranks against it instead of lowercasing every label again.
- `src/studio/adjacency.ts` (new) + `pipeline.ts`: the neighbour lists are built once per
  `Ends` instead of scanning every edge per question, in the old scan's order — first
  encounter, duplicates folded, self-loops dropped both ways. `tests/adjacency.test.ts`
  compares the new order against a copy of the old algorithm over a graph with self-loops,
  duplicate edges and a reversed duplicate.
- Every cache carries a `Caveat:` comment naming what it gets wrong (identity-keyed: an
  equal-but-new object misses and is rebuilt).

### ForcesPanel and the bar

- `src/ui/frameThrottle.ts` (new): `frameScheduler` uses `requestAnimationFrame` when the
  host has both halves of it and a 16 ms timer when it has neither (the node rig and the
  motor's worker); `throttledBar` collapses publishes into one redraw a frame.
- The panel-level `useSyncExternalStore` that discarded its value is gone; a `BarLine` child
  reads the store, throttled. The panel keeps a snapshot subscription whose value is the
  reason plus the drawn knob values, so the worker's answer still redraws the reason line —
  without redrawing on a frame. Unit-tested against a scheduler the test drives
  (`tests/frame-throttle.test.ts`), because `renderToStaticMarkup` runs no effect and no frame.
- `ActionForm` and `ForcesPanel` are compared on what they draw (`drawnOf` in `draft.ts` and
  in `ForcesPanel.tsx`), which is where most of the render drop comes from.

### rowOf

- `src/motor/liveSession.ts`: a `Map<string, number>` built once per id order instead of
  `indexOf` per pin. A duplicated id keeps its first row, which is what `indexOf` answered.

## 3. Files changed

Motor: `src/motor/{live,liveLoop,liveSession,session,worker}.ts`.
Studio: `src/studio/{adjacency,pipeline}.ts`.
Chrome: `src/ui/{ActionForm,Console,Dock,ForcesPanel,Hud,Inspector,Legend,NodeMenu,Search,Shell,Toast,draft,matches,useShortcuts,useStudio}.tsx|ts`,
`src/ui/frameThrottle.ts`.
Tests: `tests/{adjacency,force-loop,frame-throttle,live-rows}.test.ts` (new except
`force-loop`), `tests/ui/{forces-panel,legend-cache,search-cache,selectors}.test.tsx` (new),
`tests/ui/{action-form,console,dock,hud,inspector,node-menu,panels,toast}.test.tsx` (updated
for the narrower props).

Nothing outside `packages/graph-studio/**` and this report was touched; no dependency was
added; the default layout id is unchanged.

## 4. Commands and results

| command | exit |
|---|---:|
| `scripts/studio.sh check` | 0 |
| `scripts/studio.sh build` (inside `check`) | 0 |
| `scripts/studio-smoke.sh` | 0 |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 (the negative control must fail) |
| `scripts/studio-perf.sh --label p6-after --cases 1000,2000,4000` | 1 (by design with `--cases`) |

`check` covers tsc over four configs, 375 + 533 unit tests, 90 render tests, eslint with
`--max-warnings 0` and the vite build, with no skipped test in any suite.

## 5. Not done, and why

1. **No render count per commit in a unit test.** The render tests run through
   `renderToStaticMarkup`, whose server renderer has no `Profiler.onRender` (the Fizz build
   has no `onRender` at all), runs no effect, and does not re-render a child when a parent
   updates — a render-phase update re-runs the component and keeps its children's output. A
   per-commit render count is therefore not observable in this rig and counting renders would
   have meant a new dependency or a hand-rolled dispatcher. What the tests do hold: the
   selection rule keeps its reference (`tests/ui/selectors.test.tsx`), a panel never reads a
   field it does not draw (a state whose field throws when read), the panels are memoised, and
   the caches are identity-stable. The render count itself is the measurement above.
2. **The Dock still re-renders on every store change.** Its action specs read `settings`,
   `meta`, `run`, `catalog` and `graph` through arbitrary functions typed over the whole
   `StudioState`; narrowing what they are handed would mean changing that type or casting, and
   the job's paths allow neither. Its children are compared on what they draw, which is what
   removed the ~200 renders a commit.
3. **No change to the reveal animation or the layout.** The open milliseconds are the motor's
   default layout; P6 leaves it alone by instruction, which is also why they did not move.
4. **A first after-run was taken on a loaded host** and read `perf-js` 4.6 ms FAIL; re-run
   alone the same code passes at 2.7 ms. The table quotes the alone run, and the loaded one is
   recorded here rather than dropped.

## 6. Decisions taken

- The forces panel keeps a subscription, but to a **snapshot** (`drawnOf`: the reason and the
  drawn knob values) rather than to the bar. Dropping it entirely left the reason line on
  "not asked yet" until some other change redrew the dock, because `available()` reads the
  worker's last answer. The snapshot is a string, so a frame changes the fraction and nothing
  else and the panel is not redrawn.
- `dock` and `Console` subscribe for themselves rather than being handed the state, so the
  shell can hold slices only.
- `loweredLabelsOf`, `drawnOf` and `legendViewOf` are exported so the cache is observable by
  reference from a test; each is documented at its definition.