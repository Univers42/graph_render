verdict: PROCEED-WITH-CONDITIONS

# Review — an additive `loadColumns` host verb

Judge: independent, 2026-10-04. Branch `review-host-columns` at `aae2491b`, base `origin/develop`.
I changed nothing under version control but this file, and ran no cargo, docker or timed gate.
Every number is quoted from a measurement file with its line; every code claim is `file:line`.

## Rung 0 first: `loadGraph` cannot already take a columnar document

It cannot, on four independent counts, so the verdict is not "not needed".

- `documentText` (`packages/graph-studio/src/host/api.ts:47-57`) takes `unknown`, refuses
  anything that is not an object (`:48`), then `JSON.stringify`s it (`:51`). A `Uint8Array` *is*
  an object, so it serialises to `{"0":12,"1":34,…}` and reaches the normaliser as a
  numeric-keyed object.
- `normaliseIngest` (`packages/graph-studio/src/source/ingest.ts:288`) reads records. Its only
  `kind` reads are the **node and edge kind** fields (`ingest.ts:175-177`, `:222-224`, against
  `NODE_KINDS` `ingest.ts:23` and `EDGE_KINDS` `ingest.ts:24`). No payload discriminator, no
  magic-byte sniff anywhere in the file.
- `Source` (`packages/graph-studio/src/state/settings.ts:21-24`) has exactly three kinds —
  `synthetic`, `fixture`, `document` — and the `document` variant carries `text` only (`:24`).
- `Payload`'s `kind` (`packages/graph-studio/src/motor/documents.ts:24-26`) does discriminate
  `"json"` from `"columns"`, but it is chosen **inside the worker** by `documentFor`
  (`documents.ts:64-75`) from `source.kind`, and only `synthetic` reaches the columns branch
  (`documents.ts:70`, `:49`). A host cannot name it, and there is no `columns` variant of `Source`
  to select.

So the columnar ingest path is shipped, measured and used — and unreachable from a host page.
`docs/reports/service-dod.md` §1 rates step 2 and step 5 *partially met* for exactly this reason
(`loadGraph(doc)` at `docs/contract/host-api.md:27`).

## Task 1 — what a 1M-node host load costs today, by file:line

The two paths, then the measured half of each.

| step | `loadGraph` | `loadColumns` (proposed) |
|---|---|---|
| verb | `loadGraph(host, studio, started, doc)` `api.ts:59` | new, `api.ts:124-131` |
| check | `documentText` `api.ts:62`, refuse non-object `api.ts:48` | length check, in the worker |
| serialise | `JSON.stringify` `api.ts:51` | `assembleColumns` `columns-assemble.ts:85` |
| normalise | `normaliseIngest` `documents.ts:57` (`ingest.ts:288`) | none — the columns path is already the normal form |
| payload | `{kind:"json", text}` `documents.ts:59` | `{kind:"columns", bytes}` `documents.ts:26` |
| wire | `load` request `protocol.ts:120`, `source` + `fixturesUrl` only | new request variant (§2b) |
| build | `open.build(text)` `session.ts:259` → `gm_build` `motor.ts:129-130`, `staging.ts:38` | `open.buildColumns(bytes)` `session.ts:259` → `gm_build_columns` `motor.ts:143-144`, `staging.ts:71` |

### The measured halves, 1M nodes

| stage | number | line |
|---|---|---|
| `JSON.stringify`, 1M/3M, median of 4 | **1820 ms** (1640, 1518, 2001, 2370) | `perf-open-columns.md:58` |
| `gm_build`, same document, median of 4 | **8278 ms** (7454, 8815, 7741, 9383) | `perf-open-columns.md:58` |
| `loadGraph` total of the two timed regions | **10 099 ms**, 56 836 wasm pages | `perf-open-columns.md:58` |
| the decision's baseline `gm_build` | 6.73 s | `perf-open-columns.md:7` |
| synthetic 1M: generate / encode / build | 1574,1389,1698 / 1276,1297,1290 / 13586,14946,15856 → **17 632 ms** | `perf-open-synth-columns.md:63` |
| synthetic 1M in the browser, before | 15.02, 12.27, 13.56 → **13.56 s** | `perf-open-synth-columns.md:98` |
| `normaliseIngest` itself | **not measured** | — |
| `gm_build_columns`, 1M/3M, median of 4 | **1946 ms** (1748, 2730, 2070, 1821) | `perf-open-columns.md:59` |
| the encoder that arm used, median of 4 | **6379 ms** (5533, 7719, 7190, 5568) | `perf-open-columns.md:59` |
| `loadColumns` total | **8 324 ms**, 13 573 wasm pages | `perf-open-columns.md:59` |
| document size, columns against JSON | 238,241,440 B against 678,016,813 B — 2.8× smaller, 169 MB of pages saved | `perf-open-columns.md:61` |
| the build-only saving, which is the whole argument | 8278 → 1946 = **6.3 s** | `perf-open-columns.md:71` |
| why that saving did not land | 6.4 s encode against 1.8 s stringify — 4.6 s handed straight back | `perf-open-columns.md:76` |
| and that encoder's cost is 23M `Map` probes over ~7M distinct strings | spread 5533–7719 ms | `perf-open-columns.md:78-80` |
| synthetic 1M, no-dedupe generator: encode / build | 531,474,572 / 3114,3377,3369 → **5729 ms** | `perf-open-synth-columns.md:64` |
| synthetic 1M document / wasm after build, columns vs json | 177 MiB / 730 MiB against 490 MiB / 2651 MiB | `perf-open-synth-columns.md:63-64` |
| synthetic 1M median saving | **11 903 ms** | `perf-open-synth-columns.md:70` |
| synthetic 1M in the browser, after | 5.68, 4.92, 5.47 → **5.47 s** (8.09 s saved) | `perf-open-synth-columns.md:99,101` |
| `assembleColumns` on its own · `Document.nodes` from `rows` · the host→worker transfer | **all three not measured** | — |

Two gaps carry the confidence score. **`normaliseIngest`** is the stage the columns path
*removes*, and it was never measured: `perf-open-synth-columns.md:8-11` splits a 3.3 s JSON round
trip three ways (records + stringify 0.78 s, staging 0.6 s, `gm_build` 1.98 s) but that is a CPU
profile of the **synthetic** path, which never calls it. So the `loadGraph` cost above is a
**lower bound** — it omits the one normaliser run.

**`assembleColumns`** is the other, and it is the load-bearing one.
`perf-open-synth-columns.md:64`'s `encode` column is the **generator's own no-dedupe fill**
(`synthetic-columns.ts:102`, `:141`), not `assembleColumns`.
`perf-open-columns.md:84-85` says the no-dedupe lever "was not measured here and is not
claimed", and the 6379 ms encoder it *did* measure is `encodeColumns`, which `assembleColumns`
is not — `columns-assemble.ts:83` is explicit that the assembler does not even attempt the
dedupe the expensive encoder does. Honest reading: the **build** half is measured twice and lands
6.3 s better; the **assemble** half has no number for `assembleColumns`, and 4.6 s of the 6.3 s
was already lost to a predecessor of it.

## Task 2 — what `loadColumns` must define

### (a) `hostApi` stays `2`

**Rule: it does not move.** `host/contract.ts:15` carries `HOST_API = 2` with the comment at
`:14` — "Bumped on a breaking change only; an addition is detected with `"name" in el`" — and
condition 12 (`host-api.md:170-173`) says the same. The deciding line is
**`docs/contract/host-api.md:27`**, `readonly hostApi: 2`, which that same sentence qualifies.
`applyDeltas` is the precedent: it landed after `HOST_API` was 2 and was declared outside the v1
promise by addition alone (`host-api.md:144-148`), with hosts feature-testing it as
`"applyDeltas" in el` (`:146`). `loadColumns` is detected exactly so, and the Surface block at
`host-api.md:25-38` grows by one line.

### (b) Assembly in the worker; the typed arrays are **copied**

**Rule: the main thread may not assemble, and nothing is transferred.**

- *Where.* `assembleColumns` is reachable only from `motor/worker.ts:2` (imported, bound at
  `:77`) and `motor/local.ts:6,23`. Not an accident: `app/eslint.config.js:12-15` is a
  `no-restricted-imports` rule naming "src/motor/worker.ts, src/motor/local.ts and
  src/motor/helper.ts" as the only files allowed to reach `crates/graph-sdk-js`, and
  `documents.ts:36-38` states the same boundary from the other side. `host/api.ts` is not on that
  list, so `loadColumns` takes `ColumnRows` and the worker assembles with the `Assembler` it
  already holds (`documents.ts:39`, `session.ts:74`).
- *The message.* A new `Request` variant in `protocol.ts:118-133`, carrying `ColumnRowsLike` and
  nothing else — the `load` request at `protocol.ts:120` carries `Source` + `fixturesUrl` and no
  bytes. It must also be added to the `REQUESTS` list at `protocol.ts:210`, because
  `isRequest`/`isResult` (`protocol.ts:228-233`) are tag checks against those arrays, so a kind
  missing from `REQUESTS` is refused before it reaches a handler. No new `Result` kind is needed:
  `loaded` (`protocol.ts:165`) already answers.
- *Transfer.* **Copied, not transferred.** `Port.send` (`protocol.ts:191-196`) has no transfer
  parameter and `workerPort.ts:5` is `worker.postMessage(message)` with no transfer list, so
  every request payload is structured-cloned today; transfer exists only on the reply path
  (`protocol.ts:147-153`, `worker.ts:11,87`). A transferred request would be the first, and it
  would detach the host's own arrays, which the rule below needs afterwards. Copy them, and say
  so in a `Caveat:` on the new variant.

### (c) `Document.nodes` from the string table and `nodeCells`, with no second normaliser

`Document.nodes` (`documents.ts:31`) is load-bearing: `replace` puts it on `Built`
(`session.ts:260`), `Built.nodes` is typed at `built.ts:13`, and `describe` calls
`metaOf(built.nodes, order, snapshot)` at `built.ts:44`. A document that leaves it empty does not
draw — `metaOf` throws `MetaMismatch` (`source/meta.ts:41-47`), which reads as a studio bug.

The generator is the model, and it is why no second normaliser appears: `table`
(`synthetic-columns.ts:83-98`) writes the node's `id` and `label` **from the very `IngestNode`
objects `nodes` holds** — "a second copy per field would be the cost this path exists to avoid"
(`:80-82`) — and `floatColumns` (`:141-145`) copies the weights off those same objects.
`syntheticColumns` returns `{ rows, nodes: drawn.nodes, edgeCount }` (`:165`), adding no object.

**Rule:** `loadColumns` mirrors that shape and inverts nothing. `nodeCells` already holds, for
row *r*, the string-table index of `id` (`synthetic-columns.ts:107`, `HEAD + 2*r`) and of
`label` (`:113`), the kind (`:108`), the database (`:109`), the source (`:110`) and the group
(`:114`). So the worker's `columnar()` builder reads those six indices per row out of
`rows.strings` and materialises **one `IngestNode` per node** (`source/ingest.ts:26-40`) — the
same count the generator already pays. It must not call `normaliseIngest` (`ingest.ts:288`), and
it must not parse JSON to do it. `documents.ts:9-11` is the precedent in words: "the columns
path would be a second way to spell it".

**Caveat, required:** 1M `IngestNode` objects is 1M × 12 fields, and **nothing measures it**.
The only related number is the synthetic generator's own 1389 → 1621 ms at 1M
(`perf-open-synth-columns.md:74`), which includes building those objects *and* filling six
typed arrays, and which the report calls "+232, inside the spread".

### (d) Validation at the trust boundary

| layer | what it refuses | where |
|---|---|---|
| `assembleColumns` | column lengths that disagree (`checkLengths`, `checkColumn`) — a bare `GraphMotorError` with `code === undefined`; and a lone surrogate in one table entry, as a `ColumnsEncoderError` carrying `.field` | `columns-assemble.ts:92-100`, `:125-137`, `:241-247` |
| `assembleColumns` | **deliberately not** a non-finite `weight`/`version`/`strength`, nor `u32::MAX` in a required column | `columns-assemble.ts:83` (doc comment) |
| `gm_build_columns` | 15 distinct refusals, **all** `Code::ColumnsInvalid`, each with a named test | `docs/contract/ingest-columns.md:79-98` |
| byte cap | `len > MAX_INGEST_BYTES` → `IngestTooLarge`, before decode | `crates/graph-wasm/src/ingest.rs:77` (1,073,741,824), enforced `crates/graph-wasm/src/ingest/columns.rs:41` |
| JS class | `ColumnsRefusedError`, `name = "ColumnsRefusedError"` — a **sibling**, not a subclass, of `BuildRefusedError`; the ABI name behind it is `ColumnsInvalid`, carried by `codeName` | `crates/graph-sdk-js/src/errors.ts:85-86`, doc `:78-83`, `:32,56` |

Three consequences, each a condition below.

1. **The name a host reads is `ColumnsRefusedError`, not `ColumnsInvalid`.** `wireError`
   (`host/contract.ts:94-96`) returns `shown.code ?? shown.title`, the element builds its events
   off the error's `name` (`host-api.md:200-205`), and condition 7 requires the rejection's `name`
   to equal `graph-error.detail.error` (`host-api.md:141`). A verb whose refusals are a
   *different* class needs that equivalence stated, not assumed.
2. **`ColumnsRefusedError` has no hint.** `HINTS` (`state/errors.ts:21-40`) names
   `BuildRefusedError` at `:22` and nothing for columns, so a refused columnar document reaches
   the reader as `DEFAULT_HINT` — "Unexpected studio error — see the browser console for the
   stack" (`state/errors.ts:19`). A documented refusal must not read as a studio bug.
3. **Neither cap transfers.** `loadGraph`'s effective cap is `MAX_DOCUMENT_CHARS`, 2^28 UTF-16
   units (`source/limits.ts:16`), *not* the motor's 1 GiB — `host-api.md:206-208` overrides
   condition 7's cap clause and says so. A columns load writes no such string, so the only bound
   it has is the motor's 1 GiB, refused as `IngestTooLarge`. The verb must say which bound, and
   the difference must be a `Caveat:`, not a silence.

### (e) Every `loadGraph` rule in condition 7 that must also hold for `loadColumns`

| clause | `host-api.md` | how `loadColumns` keeps it |
|---|---|---|
| resolves on `setFrame`, not the rAF paint | `:137` | shares `apply` (`api.ts:73`) and the same commit; nothing new to build |
| a superseded call rejects `CancelledError`, emits no `graph-load` | `:138-139` | the generation token is `Rig.generation` (`pipeline.ts:232-233`), checked by `guard` at `:131`; **cancel must work across both verbs**, so a `loadColumns` that overtakes a `loadGraph` — and the reverse — rejects the loser. No such cross-verb pair exists today, because there is only one load verb |
| a call while disconnected rejects at once | `:139` | `element.ts:86-88` returns `notConnected()` (`element.ts:37-39`, a `DOMException` named `InvalidStateError`); the new element method must take the same branch, not `?.` |
| `notes` is the bounded `firstOf` list | `:140` | `api.ts:76` re-freezes `firstOf(graph.notes)` (`pipeline.ts:100-102`); the columns path's `notes` is `[]` (`documents.ts:50`), so `firstOf([]) === []` — same shape, and `embedrows.py:80` asserts `isinstance(notes, list)` |
| condition 3: nothing is persisted | `:112-115` | enforced downstream, not in `api.ts`: `source.host === true` (`actions/nodes.ts:116`) is what `persist.ts:38` reads to return `null`. A columns source that omits the marker becomes persistable, and `embed-storage` goes red |

Condition 7's first clause — "It takes an object, serialises it, then runs `normaliseIngest`"
(`host-api.md:136`) — is `loadGraph`'s own shape and `loadColumns` does not satisfy it by
construction. It needs its own clause, written by whoever lands this.

### (f) The embed example

`app/src/embed.ts:19` is `const FIXTURE = "fixtures/force/clustered.json"`, `:114` is
`const doc = await fixture()` and `:117` is the single `(await element.loadGraph(doc)).nodes`.
Three lines, all JSON.

**Rule: it changes to `loadColumns`, and it does not get a second path.** A query parameter
would make the published example ambiguous about which verb it teaches and would double the rows
`studio-embed` must keep green. What the swap forces is spelled out in condition 9: the counts
`fixture_counts` reads (`deploy/nav/embed.py:129-131`), the `notes` list `embedrows.py:80`
asserts, and the `-ge 19` floor hard-coded in `negctl-studio-embed`
(`scripts/orch/rows/host-api.rows:6`, its comment at `:5` naming 19 as "the targeted rows").

## Axis scores (5 = worst)

| Axis | Score | Why |
|---|---|---|
| Blast radius | 3 | one new verb on the shipped element (`contract.ts:51-53`, `element.ts:86-88`), one new protocol kind (`protocol.ts:118-133,210`), one new document builder (`documents.ts`), the embed example, and the harness row counts. Nothing in `crates/` moves and no kernel changes |
| Reversibility | 2 | additive, `hostApi` stays 2, hosts feature-detect with `in` (`host-api.md:170-173`), and the existing path stays. A revert deletes the verb, the request kind and three embed lines |
| Cost on failure | 3 | two named user-visible failures: a columns refusal reads "Unexpected studio error" (`state/errors.ts:19`), and a wrong `Document.nodes` opens the studio **blank** with `MetaMismatch` (`meta.ts:41-47`) after the host's promise resolved. No data loss, no breach; a reload recovers |
| Confidence | **4** | the build half is measured twice and is solid. But `assembleColumns` has **no** number of its own (`perf-open-columns.md:84-85`), `Document.nodes` from `rows` has none, the host→worker transfer has none, and `normaliseIngest` — the stage the columns path *removes* — was never measured either, so the saving is quoted against an unmeasured baseline |

**Worst axis: confidence, 4.** It is 4 and not 5 because the mechanism is already shipped and
gated — `gm_build_columns` sits behind `perf-open-columns.rows` and `perf-open-synth-columns.rows`
with their own negative controls, and the studio already ships a columns document
(`documents.ts:49`). What is unproven is the host-facing half, and each gap below becomes a row.

## The 9 conditions

Each is checkable by a row with a negative control that must fail, and becomes an acceptance
criterion.

1. **Rung 0 stays closed: `loadGraph` gains no columnar branch.** `documentText` (`api.ts:47-57`)
   keeps refusing a non-object at `:48`, and no sniff is added to `ingest.ts`. **Row
   `studio-check`** (`scripts/orch/rows/host-api.rows:1`, existing) over `host-api-unit`. **Break:**
   let `documentText` accept a `Uint8Array` and hand it to the worker unnormalised — the unit row
   must go red on a `Uint8Array` reaching a build.
2. **`hostApi` stays `2` and detection is `"loadColumns" in el`.** `contract.ts:15` unmoved,
   `host-api.md:25-38` gains the verb, `embed.ts:112`'s version guard unchanged. **Row
   `studio-embed`** (`host-api.rows:3`) with **`negctl-studio-embed`** (`:6`, existing). **Break:**
   set `HOST_API = 3` — `embed.ts:112` must refuse the element by name.
3. **Assembly in the worker; the typed arrays are copied, not transferred.** A new `Request`
   variant in `protocol.ts:118-133` plus a `REQUESTS` entry at `:210`, carrying `ColumnRowsLike`
   and no bytes; `Port.send` (`protocol.ts:191-196`) and `workerPort.ts:5` keep no transfer list,
   and the variant carries a `Caveat:` saying the arrays are cloned. **Row `studio-check`**
   (`host-api.rows:1`) over the protocol tag test. **Break:** delete the kind from `REQUESTS` —
   `isRequest` (`protocol.ts:228-233`) must refuse the message and the row must go red, not
   silently pass.
4. **`Document.nodes` is built once, in the worker, from `rows` + `nodeCells`, with a `Caveat:` on
   its unmeasured cost.** Mirror `table` (`synthetic-columns.ts:83-98`) and `nodeColumns`
   (`:102-116`); never call `normaliseIngest` (`ingest.ts:288`) on this path. **Row
   `studio-columns`** — new, in `scripts/orch/rows/host-api.rows`, beside `studio-embed`.
   **Break:** return `Document.nodes: []` — the row must go red on `MetaMismatch` (`meta.ts:41-47`)
   raised out of `metaOf` (`built.ts:44`).
5. **Every refusal names itself, and `ColumnsRefusedError` gets a hint.** `HINTS`
   (`state/errors.ts:21-40`) gains an entry beside `:22`, and the contract states that the name a
   host reads is `ColumnsRefusedError` (`errors.ts:86`), `ColumnsInvalid` (`errors.ts:32`) being
   the ABI code behind it. **Row `embed-columns-refused`** — new, in `deploy/nav/embedrows.py`,
   driven by `studio-embed` (`host-api.rows:3`) with its own `RunSpec(keep=...)`
   (`embed.py:94-126`). **Break:** drop the `HINTS` entry — the row must go red on `DEFAULT_HINT`
   (`state/errors.ts:19`) reaching the page.
6. **Cancel works across both verbs.** A `loadColumns` that overtakes a `loadGraph`, and the
   reverse, leaves exactly one `graph-load` and one `CancelledError` (`host-api.md:138-139`,
   `:143`), with the token at `pipeline.ts:232-233` checked at `:131`. **Row `studio-embed`**
   (`host-api.rows:3`) — the existing `embed-overlap` and `embed-overlap-reentrant` rows, extended
   to the pair. **Break:** the `supersede` fault at `embed.py:110-111`, the control that already
   exists for the single-verb case; it must redden the cross-verb pair too.
7. **Nothing is persisted.** The columns source carries the same `host: true` marker
   (`actions/nodes.ts:116`) that `persist.ts:38` reads, and a disconnected element rejects at once
   with `InvalidStateError` (`element.ts:37-39,86-88`) rather than through `?.`. **Row
   `studio-embed`** (`host-api.rows:3`): `embed-storage`, with **`negctl-studio-embed`** (`:6`).
   **Break:** the `remember` fault at `embed.py:108`, existing and unchanged.
8. **The cap is stated, and the difference is a `Caveat:`.** `loadColumns` has the motor's
   `MAX_INGEST_BYTES` = 1,073,741,824 (`ingest.rs:77`, refused at `columns.rs:41` as
   `IngestTooLarge`) and **not** the studio's `MAX_DOCUMENT_CHARS` 2^28 (`source/limits.ts:16`),
   because it writes no JSON string; `host-api.md:206-208` records that override for `loadGraph`
   and the new clause repeats it rather than leaving the reader to assume. **Row `studio-check`**
   (`host-api.rows:1`). **Break:** drop the `Caveat:` and let the row assert its presence, the way
   every other bounded read in this tree is asserted.
9. **The embed example loads columns and the row counts move with it.** `app/src/embed.ts:19,114,117`
   become a columns source and a `loadColumns` call; `fixture_counts` (`embed.py:129-131`) counts
   the columns instead of the JSON, or `embed-host-load` (`embedrows.py:81-82`) compares against
   the wrong edge count — and `notes` is `[]` on this path (`documents.ts:50`), which
   `embedrows.py:80`'s `isinstance(notes, list)` accepts. `negctl-studio-embed`'s `-ge 19` floor
   (`host-api.rows:6`) is raised to the new targeted-row count in the same commit, and every new
   row gets its own `RunSpec` with a `keep=` (`embed.py:92-93,94-126`); `studio-pack-embed` (`:16`)
   and `negctl-studio-pack-embed` (`:19`) keep their own floors in step.
   **Row `studio-embed`** (`host-api.rows:3`) and **`negctl-studio-embed`** (`:6`).
   **Break:** add the columns row to the `GATE` runs without a matching `RunSpec(keep=...)` —
   `negctl-studio-embed` must go red on a targeted row that never FAILed, the defect
   `embed.py:92-93` exists to prevent.

## Diff estimate

12 files, ~128 lines. Nothing in `crates/`, no kernel, no `Cargo.toml`. Every file stays under
the 300-line limit and every new function under 40 lines with at most 4 parameters; `loadGraph`
is already at the limit with 4 (`api.ts:59`), so `loadColumns` must not become a fifth parameter
on it — it is its own function.

| file | Δ | what |
|---|---|---|
| `packages/graph-studio/src/host/api.ts` | +18 | the verb, and the columns refusal branch beside `api.ts:61-71` (131 lines today, 149 after) |
| `host/contract.ts` +3, `element.ts` +5, `state/errors.ts` +1 | +9 | `loadColumns` on `GraphStudioHost` (`:51-53`); the method beside `element.ts:86-88` taking `notConnected()`; the hint beside `errors.ts:22` |
| `motor/protocol.ts` | +6 | the `Request` variant (`:118-133`) and its `REQUESTS` entry (`:210`) |
| `motor/session.ts` | +12 | `loadColumns` on the session facade beside `load` (`:268-271`) |
| `motor/documents.ts` | +22 | `columnar()`, one `IngestNode` per row (75 lines today, 97 after) |
| `app/src/embed.ts` | +12 | a columns source for the fixture and the `loadColumns` call (`:19,114,117`) |
| `deploy/nav/embedrows.py` +25, `deploy/nav/embed.py` +4 | +29 | the columns row and counts from the columns source; a `RunSpec(keep=...)` and the raised floor |
| `scripts/orch/rows/host-api.rows` +2, `docs/contract/host-api.md` +18 | +20 | `studio-columns` and its negctl; the surface line (`:25-38`) and a new condition 7 clause |

## Findings

| severity | file:line | defect | failure scenario | fix |
|---|---|---|---|---|
| medium | `state/errors.ts:21-40` | `HINTS` names `BuildRefusedError` at `:22` and has no entry for `ColumnsRefusedError` (`crates/graph-sdk-js/src/errors.ts:86`) | a host hands over a columnar document the motor refuses — a repeated node id, which the JSON path drops and the dense-row rule cannot (`errors.ts:79-83`) — and the reader is told "Unexpected studio error" (`:19`) for a refusal the contract documents at `ingest-columns.md:79-98` | condition 5 |
| medium | `motor/documents.ts:31`, `built.ts:44` | `Document.nodes` is load-bearing (`metaOf` reads it) and **no code path builds it from `rows`**; only `syntheticColumns` returns them, and only for a graph it drew itself (`synthetic-columns.ts:165`) | a `loadColumns` document leaves `nodes` empty, `metaOf` throws `MetaMismatch` (`meta.ts:41-47`), the studio opens blank, and the host's promise already resolved | condition 4 |
| medium | `perf-open-columns.md:84-85` | `assembleColumns` has no measurement. The 6379 ms encoder at `:59` is `encodeColumns`, which `assembleColumns` is not — `columns-assemble.ts:83` says the assembler skips the dedupe and the value checks | the proposal is argued from a 6.3 s build saving (`:71`) whose 4.6 s was handed straight back by the encoder (`:76`), and that encoder's successor is unmeasured. A host that dedupes its table pays the 6379 ms and gains 1 774 ms (`:65`) | condition 4's `Caveat:`, plus a measurement before the verb is published |
| low | `motor/protocol.ts:191-196`, `motor/workerPort.ts:5` | every request payload is structured-cloned; there is no transferred request anywhere in the tree (`protocol.ts:147-153` is the reply path) | a design that transfers the host's typed arrays detaches them, and the host needs `rows` again for `Document.nodes` | condition 3 copies, and says so |
| low | `ingest-columns.md:104` vs `source/limits.ts:16` | the contract names the 1 GiB cap without its value, and the studio's real cap for `loadGraph` is 2^28 UTF-16 units, not 1 GiB (`host-api.md:206-208`) | a host applies 1 GiB to a `loadColumns` document and is wrong in the safe direction — but applies 2^28 to `loadGraph` and is wrong in the unsafe one | condition 8 |

## Not verified here (UNKNOWN, and treated as FAIL)

- **Every number in Task 1's tables.** I read the measurement files; I ran no bench. Both are
  dated 2026-10-03 and name their own hosts and branches (`perf-open-columns.md:3-4`,
  `perf-open-synth-columns.md:3-6`).
- **`assembleColumns`' cost, `Document.nodes` from `rows`, the host→worker transfer, and
  `normaliseIngest`'s cost.** No number exists for any of the four. Named as gaps, not estimated.
  Every "saving" quoted here is against a baseline that omits `normaliseIngest`, so the true
  saving is larger than the tables show — by an unknown amount, the direction I would want it and
  not a number I can sign.
- **The browser rows.** `perf-open-synth-columns.md:111-117` says its `open s` includes a layout
  and is "not a pure ingest measurement", and its `Caveat:` at `:107-110` puts the load average
  at 15–23 on a 20-core host. I quote it as the file does and no more strongly.
- **That the 9 conditions are satisfiable as written.** They are acceptance criteria for a
  builder, not results. No row named here has been run by me.