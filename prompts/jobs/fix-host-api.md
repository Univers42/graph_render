# Job fix-host-api — clear the BLOCK in docs/reviews/review-host-api.md

Branch `host-api` (develop and `review-host-api` merged in). The review's verdict is BLOCK: three
HIGH and nine MEDIUM findings. Each table row there gives `file:line`, the failure scenario and the
fix. Read that file first; it is the spec for this job. When a line number has moved, find the code
by the quoted symbol.

## Allowed paths

`packages/graph-studio/**`, `deploy/nav/embed*.py`, `deploy/nav/smokerows.py`, `app/embed.html`,
`app/src/embed.ts`, `app/eslint.config.js`, `docs/contract/host-api.md`,
`scripts/orch/rows/host-api.rows` (new). Do not touch `deploy/nav/smokecdp.py`: other gates share it.
Anything else is a "decisions needed" item.

## Tasks, in order (F-numbers are the table rows, top to bottom)

1. **F1 (HIGH), `break-health` cannot bite.** Give `embed-no-store-error`, `embed-no-overlay` and
   `embed-drew-nodes` faults of their own, injected after the element is defined: force
   `state.error`, show a banner, and serve an empty frame. In break mode, a targeted row passes only
   when it is `FAIL` for its own reason. Carry the `why` field and treat `NOT-RUN` as a control that
   did not bite.
2. **F2 (HIGH), supersede race.** In `packages/graph-studio/src/studio/pipeline.ts`, carry a per-call
   token (a generation number) through `apply()`. Check it at the commit sites (`:151`, `:184-193`)
   and before `draw()`. Derive the "superseded" decision from that token, not from the global
   `rig.client.busy()` or `cancel()`.
   - A superseded call rejects with `CancelledError`.
   - A newer call is never rejected by an older call's cancel.
   - Add a `node:test` case in `packages/graph-studio/tests/` that calls `loadGraph(B)` from inside
     the `graph-load` handler of `loadGraph(A)`. Assert that A's promise rejects `CancelledError`, B
     resolves with B's counts, and exactly one `graph-load` carries B.
   - Run that case once with the token check removed: it must fail. Then restore the check.
3. **F3 (HIGH), `embed-overlap`'s control removes the overlap.** In `deploy/nav/embedgestures.py`,
   add a probe that supersedes re-entrantly (`el.loadGraph` called inside the `graph-load` handler).
   Its break is the task 2 regression, injected as a fault. Keep the sequential break for the counts
   only.
4. **F4 (MEDIUM), `embed-load-refused`.** Add a `break-name` fault that dispatches `graph-error`
   with a different `error` from the rejection's `name`. Keep it separate from `break-composed`.
5. **F5.** In `studio.ts` `destroy()`, cancel the reveal scheduler first (`studio/reveal.ts:21`).
6. **F6.** In `host/api.ts` `documentText`:
   - wrap `JSON.stringify` so any throw maps to an `IngestRefusal` whose `name` equals the
     `graph-error` it dispatches;
   - add a `node:test` case with a `toJSON` that throws `RangeError`.
7. **F7.** Rewrite the header of `styles/studio.css.ts` to describe `adoptedStyleSheets` and why the
   shadow root is `open`. Leave the mode as it is.
8. **F8.** In `host/api.ts` `focusNode` and `selectNodes`, `await` the dispatch and return `entry.ok`.
   Keep the synchronous fast path only while the frame's id set is unchanged.
9. **F9.** In `app/eslint.config.js:91`, narrow the `no-restricted-syntax` exemption to the exact
   paths that need it.
10. **F10.** Give `broken_wasm` its own break entry in `embed.py`, with no page fault, keeping
    `embed-no-console-error`, `embed-no-exception` and `embed-host-load`.
11. **F11.** `embed-composed` reads `Event.composed` and `Event.bubbles` from a listener the harness
    installs over CDP, not from page-recorded fields. `frozen` is checked by a harness probe.
12. **F12.** Add one `Caveat:` line to each of the ten bounds listed: the failing input, the
    direction of the error, and the escape hatch. Change `mount.ts:63`'s `Ponytail:` to `Caveat:`.
13. **LOW, house limits.** `embed.py:60` `run(...)` takes a `RunSpec` dataclass (4 parameters or
    fewer). `embed.py:181` catches `Exception` and exits 2. Fix `docs/contract/host-api.md:155` to
    point at `mount.ts:66`. Leave the other LOW rows alone, and list them under "findings" as
    follow-ups.
14. Create `scripts/orch/rows/host-api.rows` with exactly these four rows:
    ```
    studio-check|0|scripts/studio.sh check
    studio-build|0|scripts/studio.sh build
    studio-embed|0|scripts/studio-embed.sh
    negctl-studio-embed|0|STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh; test $? -eq 1
    ```
    Run each in that order yourself (they are not timed gates). Put each exit code, plus the break
    run's per-fault FAIL lines, in the return block.
15. Update the "As built" section of `docs/contract/host-api.md`:
    - the per-call token (verdict 7);
    - the new faults in the `studio-embed` row's negative-control cell;
    - the `focusNode` return.

## Done when

- `scripts/studio.sh check` exits 0, and `scripts/studio-embed.sh` exits 0.
- `STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh` exits 1, with every targeted row `FAIL` for its own
  `why` and no `NOT-RUN` among targeted rows.
- The task 2 test fails with the token check removed and passes with it in place.
- Every HIGH and MEDIUM row of the review has one line under "changed" or "findings" naming its
  `file:line` fix.
