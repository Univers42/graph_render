# Job host-columns: the additive `loadColumns` host verb

Worktree `~/goinfre/wt/host-columns`, branch `host-columns`, from develop. Build the verb that
`docs/reviews/review-host-columns.md` ruled **PROCEED-WITH-CONDITIONS**. Its 9 conditions
(`review-host-columns.md:219-282`) are the acceptance criteria. Read that file first, whole. Dispatch
at most 3 subagents, each on a disjoint set of files.

## The design, fixed here

`loadGraph` reaches the worker as `studio.dispatch("source.host", { name, text })`
(`packages/graph-studio/src/host/api.ts:73`). That becomes `settings.source` =
`{ kind: "document", host: true }` (`state/settings.ts:24,117`), then the existing `load` request
(`motor/protocol.ts:120`), then `documentFor` in the worker (`motor/documents.ts:64-75`,
called at `motor/session.ts:270`). `loadColumns` takes **the same road**, with a new `Source` kind
and no new `Request` type:

- `Source` gains `{ readonly kind: "columns"; readonly name: string; readonly rows: ColumnRowsLike; readonly host: true }`
  (`state/settings.ts:20-24`). `ColumnRowsLike` already exists (`source/synthetic-columns.ts`);
  reuse it and declare no second copy.
- One action `source.host-columns`, registered once beside `source.host` (`actions/nodes.ts:110`),
  `section: null`.
- `documentFor` gains one branch, `columnar(source, assemble)`, beside `generated`
  (`documents.ts:42-52`). The worker assembles the bytes with the injected `Assembler`, so the
  main thread never imports the SDK.
- The typed arrays reach the worker by structured clone. `Port.send` (`protocol.ts:191-196`) and
  `workerPort.ts:5` keep no transfer list.

This is a deviation from condition 3's words. Condition 3 asks for "a new `Request` variant". This
design meets its intent instead: assembly in the worker, a copy and not a transfer, and a
`Caveat:`. Say so under "decisions taken", in one line.

## Exact tasks

1. **Contract and element** (conditions 2, 7):
   - Add `loadColumns(rows: ColumnRowsLike): Promise<LoadResult>` to `GraphStudioHost`, beside
     `loadGraph` (`host/contract.ts:51-53`). Re-export `ColumnRowsLike` from `element.ts`, so that
     `app/` can name it.
   - `HOST_API` stays `2` (`contract.ts:15`).
   - In `element.ts`, add the method beside `loadGraph` (`:86-88`). While the element is
     disconnected it rejects at once through `notConnected()` (`:37-39`), as `loadGraph` does.
2. **The verb** (conditions 1, 5, 8). In `host/api.ts`, `loadColumns` is a function of its own and
   not a fifth parameter on `loadGraph` (`api.ts:59`).
   - Validate at the boundary before anything is dispatched. `rows` is a non-null object. Each of
     `nodeCells`, `edgeCells`, `weights`, `versions` and `strengths` is the typed-array class that
     `ColumnRowsLike` names. `nodeCells.length % 8 === 0` and `edgeCells.length % 8 === 0`. The
     three float columns' lengths match the counts that `docs/contract/ingest-columns.md` states.
     Cite its line.
   - A failed check rejects with a `TypeError` that names the field. Give it no `graph-error`,
     the same as `loadGraph`'s call-boundary rule (`api.ts:64-66`).
   - `documentText` (`api.ts:47-57`) does not change. No format sniff is added to `ingest.ts`.
   - Add one `Caveat:` above `loadColumns` for the cap. The cap is the motor's `MAX_INGEST_BYTES` =
     1,073,741,824 (`crates/graph-core/src/ingest.rs:77`, refused as `IngestTooLarge`), not
     `MAX_DOCUMENT_CHARS` 2^28 (`source/limits.ts:16`). Say that this path writes no JSON string.
   - A motor refusal rejects with `name === "ColumnsRefusedError"`. Do it through the same
     `refusal(entry)` path that `loadGraph` uses (`api.ts:74`).
   - In `state/errors.ts`, `HINTS` (`:21-40`) gains a `ColumnsRefusedError` entry beside
     `BuildRefusedError` (`:22`).
3. **The worker side** (conditions 3, 4):
   - `columnar()` builds `Document.nodes` once, from `rows.strings` + `rows.nodeCells`. Mirror
     `table` and `nodeColumns` (`source/synthetic-columns.ts:83-116`). It never calls
     `normaliseIngest`.
   - Give `columnar()` a `Caveat:` that says its cost at 1M is unmeasured
     (`review-host-columns.md`, finding 3).
   - `edgeCount` is `edgeCells.length / 8`. `notes` is `[]`.
   - `documents.ts` stays ≤300 lines.
   - The `Source` variant carries a `Caveat:` saying the arrays are cloned: memory is twice the
     columns while the load is in flight.
4. **Every place that switches on `Source.kind`** handles `columns`. Find them with
   `git grep -n 'kind === "document"\|source.kind' -- packages/graph-studio/src`. That includes
   `studio/studio.ts:84`, `state/settings.ts:117` and `state/persist.ts:38`. `persist.ts` returns
   `null` for it: condition 7, nothing is persisted.
   - `source/document.ts:31` has a second `documentFor`. Check whether anything imports it
     (`git grep -n "source/document"`). If nothing does, delete the file and say so. If something
     does, give it the same branch.
5. **Cancel across both verbs** (condition 6). In `packages/graph-studio/tests/host-supersede.test.ts`,
   in that file's style, add two `node:test` cases:
   - a `loadColumns` that overtakes a `loadGraph`;
   - a `loadGraph` that overtakes a `loadColumns`.

   In each case exactly one `graph-load` fires, and the overtaken promise rejects with
   `name === "CancelledError"`.
6. **Unit tests**, in a new `tests/host-columns.test.ts`, in the style of `tests/host-load.test.ts`:
   - a 3-node, 2-edge `ColumnRowsLike` loads, and the resolved `LoadResult` is `{nodes: 3, edges: 2, notes: []}`;
   - `Document.nodes` ids equal the string-table ids, in row order;
   - each boundary refusal of task 2 rejects with `TypeError` and names its field;
   - a disconnected element rejects with `InvalidStateError`;
   - the settings source after the load is not persisted.
7. **The embed example** (condition 9).
   - Add `app/src/embedColumns.ts`: one function that turns the fetched `clustered.json` into a
     `ColumnRowsLike`, following the column order in `ingest-columns.md` (cite the lines).
     `app/src/embed.ts:114-117` then calls `loadColumns` with it. Keep no second `loadGraph` path.
   - `embed.ts` and the new file stay ≤300 lines, with functions ≤40 lines.
   - Read `fixture_counts` (`deploy/nav/embed.py:129-131`) and `embed-host-load`
     (`deploy/nav/embedrows.py:81-82`). If the columns path counts edges differently from the JSON
     path, make `fixture_counts` count the columns. Either way, write in "decisions taken" which
     one you found, with the counts.
8. **The new embed rows** (conditions 5, 9).
   - Add a step and a row `embed-columns-refused`. It calls `loadColumns` with a document the motor
     refuses: a repeated node id (`crates/graph-sdk-js/src/errors.ts:79-83`). Assert that the
     rejection `name` is `ColumnsRefusedError` and that a `graph-error` carried the same code.
   - Add the step to `FULL` (`embed.py:59-60`), before `channels`. Keep the order comment
     (`:54-58`) true.
   - Its break is a `RunSpec` with its own fault and `keep=("embed-columns-refused",)`, mirroring
     `break-name` (`embed.py:112`, `embedpage.FAULTS["name"]`, `embedpage.py:203`).
   - Raise the `-ge 19` floor of `negctl-studio-embed` and `negctl-studio-pack-embed`
     (`scripts/orch/rows/host-api.rows:6,19`) to the new targeted-row count. Count it from `BREAKS`,
     and write the count in the row's comment.
9. **The contract.** `docs/contract/host-api.md`:
   - add the verb to the interface (`:25-38`);
   - add a `loadColumns` clause under condition 7, stating the cap from task 2, the refusal name,
     the cross-verb cancel and that nothing is persisted. One sentence each.

## Allowed paths

`packages/graph-studio/src/**`, `packages/graph-studio/tests/**`, `app/src/**`, `deploy/nav/**`,
`scripts/orch/rows/host-api.rows`, `docs/contract/host-api.md`, `prompts/jobs/host-columns.md`,
`target/**`.

Nothing under `crates/`, `src/`, `harness/` or `fixtures/`, and no change to any `package.json`
or lockfile.

## Run, in order, and paste each exit code and its last 3 lines

1. `scripts/studio.sh wasm`
2. `scripts/studio.sh check`
3. `scripts/studio.sh build`
4. `scripts/studio-embed.sh`
5. The `negctl-studio-embed` row's command from `host-api.rows` (expect exit 0: it asserts the
   break's 1).

The orchestrator's rows file `target/wf/host-columns.rows` re-runs these and the pack rows. A row
you did not run is "not run", never "green".

## Return block

- the files changed, with their line counts;
- the 5 exit codes above;
- the new targeted-row count;
- "decisions taken" / "decisions needed".

## Done when

- every task above is done;
- the 5 runs exit as stated;
- `git status --porcelain` shows only the allowed paths.
