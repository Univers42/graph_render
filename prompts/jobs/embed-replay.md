# Job embed-replay: the embed example streams deltas from a JSONL replay (service DoD step 5)

Worktree `~/goinfre/wt/embed-replay`, branch `embed-replay`, from studio-pack d26e4c11 (develop + the pack; studio-pack is landing).
DoD step 5 (`~/goinfre/orch/handoff/service-dod.md`) asks for a host page that loads columns,
listens to `node-open`, resolves previews through `resolve`, **and streams deltas from a JSONL
replay**, under one browser gate. The first three exist (`app/src/embed.ts`, gate
`scripts/studio-embed.sh` → `deploy/nav/embed.py` + `embedrows.py`). The replay does not.
Dispatch at most 2 subagents.

## Facts (read them in the tree; cite file:line in the return block)

- `<graph-studio>.applyDeltas(batch)` resolves `{applied}`: `packages/graph-studio/src/element.ts:104`
  and `:128-137`, the verb in `src/actions/batch.ts` (`createDeltas`). `applied` is the number of the
  batch's nodes that went in (`src/motor/client.ts:38-40`), so 8 per replay line. A refused batch
  rejects with the motor's own typed error, and the same name is sent as `graph-error`; a duplicate
  id is refused by the motor's ingest. Measure that name once (it is expected to be
  `IngestInvalid`, `docs/contract/delta.md:141`), and cite where it is raised. Hosts feature-test
  with `"applyDeltas" in el` (`docs/contract/host-api.md` condition 8).
- `deploy/nav/deltarows.py`: `record(node_id)` (`:36`) is the ingest-v1 node record, and
  `row_refused` (`:195`) shows how a rejection's `name` is caught inside one evaluate.
- A batch is the ingest JSON v1 document (`docs/contract/delta.md` "The batch"): an endpoint may
  name a node already in the graph or one in the same batch; a repeated node id refuses the whole
  batch with nothing changed.
- The page loads `fixtures/force/clustered.json` (60 nodes, 138 edges; ids `c<k>_<i>`).
- `deploy/nav/delta.py` + `deltarows.py` already judge a grown drawing (`deltas-drawn`). "Drawn" is
  the element's `view.frame().nodeCount` (`deltarows.py:85-88`, `wait_grown` `:91`): read the same
  thing on the embed page's element, and reuse `wait_grown`'s polling if its `studio` handle fits;
  if it does not, say why in "decisions taken".
- `deploy/nav/embed.py`: `STEPS` (`:51`) and the break runs (`RunSpec`, `:84-109`). A break is a
  fault injected over CDP or in the bytes served, never a product switch; each break run reports
  only the rows it targets (`keep`).
- `STUDIO_EMBED_PACK=<dir>` reruns the same gate over the built pack (`deploy/nav/embedpack.py`);
  the replay must pass there too.

## Tasks, in order

1. **The replay file**, `fixtures/embed/replay.jsonl`, data written once by hand (no generator
   committed). Six lines, one ingest-v1 batch each, keys in the order `delta.md` shows:
   - lines 1-5: 8 nodes each (`r<b>_<i>`, b 0-4, i 0-7) and 12 edges each (`re<b>_<j>`), every
     endpoint a `c*` node of the fixture or an `r*` node of the same or an earlier line;
   - line 6: one node `r0_0` (already added by line 1) and no edges: the refusal case.
   Check with a one-off command that no id collides with the fixture, and paste it.
2. **`app/src/embed.ts`**: a "Replay" `<button id="replay">` in `app/embed.html`. On click, the page:
   - feature-tests `"applyDeltas" in element`;
   - fetches `fixtures/embed/replay.jsonl`, splits it on `\n`, skips blank lines, and `await`s
     `applyDeltas(JSON.parse(line))` for each line in order, never in parallel;
   - records it on `window.__embed.replay`: `{ state: "idle"|"running"|"done"|"failed <msg>",
     applied: number[], refused: string[] }` (a refusal pushes the error's name and the replay goes
     on to the next line).
   No style attribute, no inline script (the page runs under `HOST_CSP`, `embed.py:42`). Keep
   `embed.ts` under 300 lines and every function under 40; put the replay in a new
   `app/src/embedReplay.ts` if it does not fit.
3. **The gate step**, in a new `deploy/nav/embedreplay.py` (keep `embed.py` under 300 lines; it
   only registers the step and the break run). Step `replay` runs last, after every other step of
   the `plain`, `isolated` and `csp` runs. It clicks `#replay` with a real CDP mouse event, waits
   for `done` with a cap, then writes three rows:
   - `embed-replay-applied`: five batches applied, each answering `applied` 8;
   - `embed-replay-refused`: exactly one refusal, line 6, with the measured error name, and
     one `graph-error` heard with the same name;
   - `embed-replay-drawn`: the drawing settles at 100 nodes (60 + 5 × 8), read by the reader
     `deltarows.py` uses.
4. **The negative control**, `RunSpec("break-replay", ...)`: serve `replay.jsonl` with line 3
   removed (a fault in the bytes served, like `broken_wasm`). Its `keep` is
   `embed-replay-applied` and `embed-replay-drawn`; both must FAIL (4 batches; 92 nodes), so
   `STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh` still exits 1.
5. **Docs**: in `docs/contract/host-api.md`'s embed example section (or a new 5-line section if
   there is none), say the example replays `fixtures/embed/replay.jsonl` through `applyDeltas`, one
   batch at a time, and name the three rows. Update the header of `scripts/studio-embed.sh` (its
   "Rows:" paragraph) in the same commit.
6. Run by hand, in order, and paste each exit code and its last 3 lines:
   `scripts/studio.sh wasm`; `scripts/studio.sh check`; `scripts/studio.sh build`;
   `scripts/studio-embed.sh`; `STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh`;
   `scripts/studio-pack.sh target/pack`; `STUDIO_EMBED_PACK=target/pack/graph-studio-0.1.0 scripts/studio-embed.sh`.

## Allowed paths

- New: `fixtures/embed/replay.jsonl`, `app/src/embedReplay.ts`, `deploy/nav/embedreplay.py`.
- Edits: `app/src/embed.ts`, `app/embed.html`, `deploy/nav/embed.py` (register only),
  `deploy/nav/embedrows.py` (only if a shared helper must move), `scripts/studio-embed.sh` (header),
  `docs/contract/host-api.md`, `target/**`.
- Nothing under `packages/`, `crates/`, `src/`, `harness/`, the root `package.json`/lockfile. If the
  replay needs a change in `packages/graph-studio` (for example, `applyDeltas` refuses something
  `delta.md` says it accepts), STOP and report the exact change under "decisions needed".

## Return block

- the replay's six lines' node/edge counts and the collision check's output;
- for `plain`, `isolated`, `csp` and the pack run: the `report.json` PASS/FAIL/NOT-RUN counts and
  the three replay rows' measured strings;
- the break run's two rows' measured strings;
- each command's exit code;
- "decisions taken" / "decisions needed".

## Done when

`scripts/studio-embed.sh` exits 0 with the three replay rows PASS in all three runs, the pack run
exits 0, and `STUDIO_EMBED_BREAK=1` exits 1 with both `break-replay` rows FAIL for the missing batch.
