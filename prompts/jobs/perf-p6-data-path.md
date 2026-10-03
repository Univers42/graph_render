# Job perf-p6-data-path (agent build: a live reveal step costs only what changed, at 1M)

Goal: while an AI process grows the graph, the studio reveals nodes step by step. Each step should
cost O(changed rows), not a rebuild of every per-node style array. Measure first, then fix only
what the measurement shows.

Facts (verified on develop b2cbbcd9):
- `packages/graph-studio/src/studio/pipeline.ts:258-261` `reveal(count)` patches the store and
  calls `restyle`. `restyle` (`:95-99`) calls
  `styleFrom(styleInputOf({ meta, appearance, filter, groups, analysis, reveal }))`.
- `look/styleOf.ts:92-106` `styleInputOf` rebuilds everything on every call:
  - `colouringOf(input)`, the colours and the palette;
  - `weightsOf(input)` (`:84-90`), a fresh `Float32Array.from` over every node for `degree`;
  - `hiddenOf(meta, filter)`, then `withReveal(...)`.
  A reveal step changes only `reveal`, so only `hidden` can change.
- `styleFrom` is `packages/graph-render/src/style.ts:126`. That package is peer-owned: read it, do
  not edit it.
- `pipeline.ts:91-93` `sameSource` compares two sources by `JSON.stringify`. A `document` source
  carries the whole document text (`state/settings.ts:14-17`), so a comparison that misses the
  `a === b` fast path serialises the document twice.
- `pipeline.ts:163,225` stringify the filter, a small object; leave them unless the measurement
  says otherwise.
- `motor/session.ts:99-104` `sha256Hex` copies the snapshot (`bytes.slice()`) and hashes it once
  per layout (`:229`). The digest is part of every action's result (`pipeline.ts:243`) and of the
  recipe check (`state/recipe.ts:86-89`).

Do, in order:
1. Measure at 1M nodes in the browser, on the gm-chromium hardware arm, with performance marks:
   - one reveal step;
   - one appearance change (`colourBy`);
   - one filter change;
   - the digest at load.
   Run 3 alternated rounds, report medians, and print the load.
2. Write a failing test first (`packages/graph-studio/tests/`): after a reveal step, the colours
   and weights arrays are the same objects as before the step (identity, `===`), and `hidden`
   differs only in the revealed rows.
3. Memoise `styleInputOf`'s parts on the identity of their inputs:
   - colours on `(meta, appearance.colourBy, analysis, groups)`;
   - weights on `(meta, appearance.sizeBy, analysis)`;
   - the filter's hidden mask on `(meta, filter)`.
   A reveal then recomputes only `withReveal` over the memoised mask. Use a single-entry memo per
   part (the last inputs and result), not a growing cache.
4. Replace `sameSource`'s `JSON.stringify` with a field-by-field comparison per kind. Strings
   compare with `===`, which allocates nothing.
5. Digest: if step 1 shows it above 10 % of the 1M load, write that number in the report and stop
   there. Moving it off the critical path changes the action result's contract and needs its own
   decision.
6. Measure again, as in step 1.

Out of bounds: `packages/graph-render`, `crates/`, `motor/liveSession.ts`, `motor/live.ts` and
`motor/session.ts` (job perf-start-shuffle edits those). No new dependency. Studio house rules: no
type assertions, 40 lines a function, 300 a file.

Paths you may edit: `packages/graph-studio/src/look/`, `packages/graph-studio/src/studio/pipeline.ts`,
`packages/graph-studio/tests/`, `docs/measurements/perf-p6-data-path.md`.

Done when:
- The step-2 test fails on develop and passes on the branch.
- `scripts/studio.sh check` exits 0.
- `scripts/studio-filters.sh` exits 0, and `STUDIO_FILTERS_BREAK=1 scripts/studio-filters.sh` exits
  non-zero.
- `scripts/studio-smoke.sh` exits 0, and `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits
  non-zero.
- `docs/measurements/perf-p6-data-path.md` holds the step-1 and step-6 tables with load, the digest
  share, and what the change does not do (`styleFrom` itself still runs per restyle).
