# Job host-api-lows: close the open LOW rows of docs/reviews/review-host-api.md

Worktree `~/goinfre/wt/host-api-lows`, branch `host-api-lows`, started from the `embed-replay` tip
(0477aa77), because task 6 edits the `csp` run that embed-replay added a step to. The review's
findings table (`docs/reviews/review-host-api.md:15-37`) still has these rows open on develop; the
others are closed and must not be touched. Dispatch at most 3 subagents (disjoint files).

## Exact tasks, one fix plus one `node:test` case each (tests under `packages/graph-studio/tests/`)

1. **A stopped client rejects with `CancelledError`, not `Error`.** `packages/graph-studio/src/motor/client.ts:263`
   throws `new Error("the motor client is closed")`. After `stopMotor()` a host's next `loadGraph`
   therefore rejects with `name === "Error"`, while `docs/contract/host-api.md` names `CancelledError`
   (`client.ts:20-24`) for a call that will never complete. Throw `new CancelledError()` there. Test,
   in `tests/client.test.ts` (keep the file's style): close a client, call `layout`, assert the
   rejection's `name` is `"CancelledError"`. Then in `docs/contract/host-api.md`, in the section that
   lists the `@internal` members (`studio`, `view`, `stopMotor`, `watchdogBoundMs`;
   `src/host/contract.ts:62-78`), add one sentence: they are present at runtime, are not part of the
   contract, and a call after `stopMotor()` rejects with `CancelledError`.
   `client.ts` is 291 lines: it must stay at or under 300.
2. **The hover reset on a new graph goes through the frame scheduler.** `src/host/watch.ts:64`
   (`newGraph`) calls `hoverTo(deps, seen, null)` directly, outside the one-`node-hover`-per-frame
   budget that `hovered` (`:99-107`) enforces (contract verdict 10). Route it the same way
   `hovered` does: set `seen.node = -1` and schedule through `frames.next`, so a reset and a hover in
   the same frame announce once. `newGraph` needs the `frames` handle; pass it rather than adding
   module state. Test in `tests/host-events.test.ts` or `tests/frame-throttle.test.ts` (whichever
   already drives `watchHost` with a fake `FrameScheduler`): a hover then a new graph inside one frame
   emits at most one `node-hover`, and its `detail.id` is `null`.
3. **`frozen()` has a cycle guard.** `src/host/events.ts:4-10` recurses with no `seen` set; it is
   safe today only because `structuredClone` (`:22`) runs first, and a clone keeps cycles, so a
   cyclic `detail` overflows the stack inside `dispatchEvent`. Add a `WeakSet` of visited objects.
   Test in `tests/host-events.test.ts`: `emit` with a detail whose object refers to itself returns,
   and the listener's `detail` is frozen at both levels.
4. **A failed fresh load rolls the store back with the frame.** `src/studio/pipeline.ts:124-128`
   `clear(rig)` empties `meta`, `run`, `selected`, `selection`, `reveal` and the frame, but leaves
   `state.graph`, `state.analysis` and `settings.source` at the failed run's values, so the store
   claims a graph that is not on screen. Read where a fresh load sets those three
   (`pipeline.ts` around `:184-193` and the caller at `:264-279`), capture their values before the
   call, and restore them in the same `patch` as `clear` on the `fresh && rig.generation === token`
   path (`:192`). Keep `pipeline.ts` (287 lines) at or under 300: if it would pass, move `clear` and
   the rollback into a child module `src/studio/pipeline/clear.ts`. Test: a load that the motor
   refuses (copy how `tests/host-load.test.ts` makes one fail) leaves `graph`, `analysis` and
   `settings.source` equal to their values before the call.
5. **`mount()` is exception-safe.** `src/mount.ts:22` builds the worker, view, root, watchers and
   subscriptions; `element.ts` assigns `#mounted` only on success, so a throw part-way leaves the
   worker and the subscriptions with no handle (`disconnectedCallback` unmounts `null`). Wrap the
   wiring so that, on a throw, everything already created is released (call the same release steps
   `unmount` uses, `mount.ts:46`, on what exists) and the error is rethrown. Test with a `spawn`
   option (`StudioElementOptions.spawn`, `mount.ts:30`) or another injectable step that throws after
   the client exists, and assert the fake worker was terminated. If no step after the client can be
   made to throw without editing product code beyond `mount.ts`, write the fix, say so under
   "decisions taken", and test the release helper on its own instead.
6. **The `csp` run renders one inline-style site.** `deploy/nav/embed.py:84`
   `RunSpec("csp", ("load", "channels", "replay"), csp=HOST_CSP)` never opens the hover card, the
   legend or the node menu, which write React inline styles (`src/ui/HoverCard.tsx:55`,
   `ui/Legend.tsx:110,120`, `ui/NodeMenu.tsx:83`). Add to the `csp` run the existing step of `STEPS`
   (`embed.py:53`) that renders the hover card (the `resolve` step, if it does; read it and cite the
   line), placed before `replay`. Its rows must PASS in the `csp` run and `embed-no-console-error`
   must stay PASS there: that is the measurement that a host CSP without `'unsafe-inline'` admits
   those styles. If no existing step renders one, add none and report it under "decisions needed".
7. **Trailing newline.** `scripts/orch/gate.sh` and `scripts/orch/rows/svc-floor.rows` end without a
   final newline. Append exactly one `\n` to each, and change nothing else in either file
   (`gate.sh` is run by live gates from other checkouts; only the last byte may change).

Not done, on purpose (write each in the report's "decisions taken" with this reason):
- `src/host/lru.ts` `onEvict`: no holder needs it (the one cache holds frozen data), YAGNI.
- `mount.ts:82`, the 60 s object-URL revoke: revoking on unmount would cut a download that started.

## Allowed paths

- `packages/graph-studio/src/motor/client.ts`, `src/host/watch.ts`, `src/host/events.ts`,
  `src/studio/pipeline.ts`, `src/studio/pipeline/**` (new), `src/mount.ts`, `src/element.ts` (task 5
  only, and only if the fix needs it), `packages/graph-studio/tests/**`.
- `deploy/nav/embed.py` (the `csp` RunSpec only), `docs/contract/host-api.md`,
  `scripts/orch/gate.sh` and `scripts/orch/rows/svc-floor.rows` (the last byte only),
  `prompts/jobs/host-api-lows.md`, `target/**`.
- Nothing else: not `crates/`, `src/`, `app/`, the root `package.json`/lockfile. A fix that needs a
  path outside this list is a stop: report the exact change under "decisions needed".

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, no `eslint-disable`,
no `@ts-ignore`, no `as any`. A heuristic carries a `Caveat:` line.

## Run, in order, and paste each exit code and its last 3 lines

`scripts/studio.sh wasm`; `scripts/studio.sh check`; `scripts/studio.sh build`;
`scripts/studio-embed.sh`; `STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh` (expect exit 1).

## Return block

- per task 1-7: the file:line changed, the test name, and PASS/FAIL from `studio.sh check`;
- the `csp` run's `report.json` PASS/FAIL/NOT-RUN counts and the rows the added step wrote;
- `wc -l` of every changed source file;
- each command's exit code; "decisions taken" / "decisions needed".

## Done when

`scripts/studio.sh check` exits 0 with the new test cases counted and none skipped,
`scripts/studio-embed.sh` exits 0, and `STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh` exits 1.
