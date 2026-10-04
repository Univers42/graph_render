# Job review-host-columns: a devil verdict on an additive `loadColumns` host verb

Worktree `~/goinfre/wt/review-host-columns`, branch `review-host-columns`, from develop. This is a
**review**. Write one file, `docs/reviews/review-host-columns.md`, and change nothing else under
version control. Dispatch at most 2 `explore` subagents.

## The question

The service DoD (2026-10-03) asks for `load(columns)` in the host contract, and for an embed example
that "loads columns". `docs/reports/service-dod.md` §1 rates steps 2 and 5 **partially met** for one
reason: the only load on the host API is `loadGraph(doc: object)` (`docs/contract/host-api.md:27`). It
serialises the object to JSON and runs `normaliseIngest` (`packages/graph-studio/src/host/api.ts:48-55`,
contract condition 7). The rename `load` → `loadGraph` is condition 1 of that contract's verdict and
stays.

What already exists:

- The SDK: `ColumnRows` and `assembleColumns(rows)` (`crates/graph-sdk-js/src/columns-assemble.ts:66-86`)
  and `Motor.buildColumns(bytes)` (`crates/graph-sdk-js/src/motor.ts:142-143`). The bytes are the format
  in `docs/contract/ingest-columns.md`.
- The studio already sends columns to the worker: `Payload` `{ kind: "columns"; bytes }`
  (`packages/graph-studio/src/motor/documents.ts:24-26`). The synthetic generator uses it
  (`documents.ts:41-49`, measured in `docs/measurements/perf-open-synth-columns.md`).
- But `Document` also carries `nodes: readonly IngestNode[]` (`documents.ts:28-34`). The main thread
  holds them for the inspector and the search.

The proposal: add `loadColumns(rows: ColumnRows): Promise<LoadResult>` to `GraphStudioHost`. It is
additive. It builds a `Document` with the `columns` payload. Rule on whether to build it, and on how.

## Exact tasks

1. **Measure the path a 1M-node host load takes today**, from file:line only, no run:
   - `loadGraph` → `JSON.stringify` → `normaliseIngest` → `json` payload → `gm_build`;
   - against `loadColumns` → `assembleColumns` → `columns` payload → `gm_build_columns`.
   Quote the numbers `perf-open-columns.md` and `perf-open-synth-columns.md` already measured for each
   half, with their lines. If a half has no number, say "not measured".
2. **List what `loadColumns` must define**, each with the file:line it touches and the rule you set:
   - (a) `hostApi`: stays `2`, because the change is additive (`host-api.md:27`: "bumped on a breaking
     change only"), or moves. Name the line that decides.
   - (b) Where the bytes are assembled, on the main thread or in the worker, and whether the typed
     arrays are transferred or copied. Name the protocol message in
     `packages/graph-studio/src/motor/protocol.ts`.
   - (c) How `Document.nodes` (`IngestNode[]`) is produced from the string table and `nodeCells`,
     without a second normaliser. Read `source/synthetic-columns.ts` for how the generator does it.
   - (d) Validation at the trust boundary. What `assembleColumns` refuses (`columns-assemble.ts:83`),
     what `gm_build_columns` refuses (`ColumnsInvalid`, `crates/graph-sdk-js/src/errors.ts:78`), and the
     byte cap (contract condition 7: `IngestTooLarge` at 1 GiB). Check that every refusal rejects the
     promise with a `name` equal to the `graph-error` code, as condition 7 requires for `loadGraph`.
   - (e) Every `loadGraph` rule in condition 7 that must also hold for `loadColumns`:
     - it resolves on `setFrame`;
     - a superseded call rejects with `CancelledError`, and the cancel works across both verbs;
     - a call while disconnected rejects at once;
     - `notes`;
     - condition 3: nothing is persisted.
   - (f) The embed example: `app/src/embed.ts:19,114,117` changes to `loadColumns`, or it gets a second
     path, chosen by a query parameter. The `studio-embed` row (`scripts/orch/rows/host-api.rows:3`) must
     keep its counts.
3. **The verdict**, in the format of `docs/reviews/review-svc-r3.md`:
   - a first line `verdict: PROCEED | PROCEED-WITH-CONDITIONS | BLOCK`;
   - the four axis scores 1-5 (blast radius, reversibility, cost on failure, confidence), with the worst
     one named;
   - numbered conditions. Each condition can be checked by a row with a negative control: name the rows
     file, the row (new or existing), and the break that turns it red;
   - the diff estimate: files, and rough lines per file.

   Treat this as a public-surface change (`.claude/rules/devil/risk.md`). If the answer is BLOCK, say
   what would unblock it.
4. **Rung 0 first.** If `loadGraph` over a columnar document already exists (a `kind` field read by
   `normaliseIngest`, or a format sniff), the verb is not needed. Say so, with the line, and the verdict
   is then "not needed" and names the existing path.

## Allowed paths

`docs/reviews/review-host-columns.md` (new), `prompts/jobs/review-host-columns.md`, `target/**`.

## Run, and paste the exit code

`scripts/orch/gate.sh target/wf/docs scripts/orch/rows/docs.rows` (expect 0).

## Return block

- task 1's numbers with their lines;
- the verdict line, the four scores, the condition count and the diff estimate;
- "decisions taken" / "decisions needed".

## Done when

- `docs/reviews/review-host-columns.md` exists with a verdict line;
- every claim carries a file:line;
- `docs.rows` passes;
- `git status --porcelain` shows only the allowed paths.
