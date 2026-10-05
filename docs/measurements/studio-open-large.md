# studio-open-large — what opening a 62 MB document costs, and what was cut

Measured 2026-10-05 on the studio worktree, in the `gm-chromium` container, on the production
build (`scripts/studio.sh build`). The document is `target/git-plugin/git/git.studio.json`
(85 928 nodes, 107 694 edges, 62 051 330 bytes), written by `examples/plugins/git/run.sh
/tmp/gitviz/git.git`; the second is `target/git-plugin/contributor-stats/contributor-stats.studio.json`
(488 nodes, 490 edges, 347 097 bytes). Backend `webgl2` unless the row says `canvas2d`.

## The commands

```
examples/plugins/git/run.sh /tmp/gitviz/git.git                     # 29 s, writes the 62 MB document
examples/plugins/git/run.sh /tmp/gitviz/contributor-stats.git       # 2 s
scripts/studio.sh build
scripts/studio-probe.sh open-document target/git-plugin/git/git.studio.json webgl2 \
    target/git-plugin/contributor-stats/contributor-stats.studio.json
scripts/studio-probe.sh open-document target/git-plugin/git/git.studio.json canvas2d \
    target/git-plugin/contributor-stats/contributor-stats.studio.json
scripts/studio.sh test                                               # 1414 tests
scripts/orch/gate.sh target/rows-studio-open-large scripts/orch/rows/studio-open-large.rows
```

Ten rounds per arm: three on each backend with the branch's build, then two interleaved pairs
(`dev` = `origin/develop`'s `ingest.ts` copied in and rebuilt, `mine` = this branch's), one probe at
a time, load average 2.8–3.1 for every interleaved round. The first three rounds per backend ran at
a load average of 5.3 (`dev`) and 0.8–1.6 (`mine`), which is why the interleaved pair is the one to
read: 20 cores, 31 GB of RAM, 3 GB free, no GPU (`GM_GPU` unset → the software rasteriser both arms
drew on, `renderer ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)`).

## Before and after

Medians of five rounds per arm per backend. `open` is the `source.document` dispatch's own
milliseconds; `switch` is the same for the second document, opened straight after; `first frame` is
the milliseconds to the first animation frame after the open returned (see the caveat — it is a
screencast-forced frame, so read it as "one frame after the open", not as a paint latency);
`heap` is the page's `performance.memory` after the large open; `rss` is every Chromium process in
the container after the large open, sum and largest single process.

| arm | backend | open ms | first frame ms | switch ms | heap MB | rss MB (sum, largest) | crashes |
|---|---|---|---|---|---|---|---|
| develop | webgl2 | 1622 | 145 | 55 | 125 | 2139, 1270 | 0 |
| this branch | webgl2 | **1516** | 136 | 53 | 125 | 2129, 1260 | 0 |
| develop | canvas2d | 1585 | 0 | 58 | 118 | 2040, 1237 | 0 |
| this branch | canvas2d | **1450** | 0 | 51 | 118 | 2028, 1230 | 0 |

Raw rounds (open ms / switch ms):

| arm | webgl2 | canvas2d |
|---|---|---|
| develop | 1592/57 1622/55 1629/51 1972/56 1593/52 | 1585/53 1594/59 1687/74 1531/58 1532/55 |
| this branch | 1461/46 1509/48 1516/58 1547/53 1548/87 | 1372/48 1450/51 1445/47 1466/61 1453/52 |

The cut is **−106 ms (−6.5%) on webgl2 and −135 ms (−8.5%) on canvas2d**, and it is inside the
noise of a single round on its own: it is the median of five, the interleaved pair agrees
(1593 → 1548 and 1532 → 1453), and the profile rows below say where it comes from. The switch did
not move: the second document normalises in about 3 ms either way.

The probe as shipped differs from the one that produced the table above only in comments and
docstrings (it went over the house's 300-line limit and was trimmed); two further webgl2 rounds on
the shipped probe, after the trim, gave 1533 ms and 1581 ms — inside this arm's spread.

## Where the open goes — the three largest costs

From `scripts/studio-probe.sh open-document` on develop's build, worker profile,
0.5 ms sampling (`target/probe/base-webgl2-1.txt`, worker `392BA2`, 1.98 s sampled):

| cost | measured | where |
|---|---|---|
| `normaliseIngest` — the document's own JSON read | **incl 342.8 ms**, self 109.0 `cr` + 73.2 `Xn` + 65.9 `sr` + 26.7 `rr` | `packages/graph-studio/src/source/ingest.ts:288`, and its `wireNode:214`, `readNode:190`, `readEdge:237` |
| `gm_build` — the motor reading the wire text again | **incl 289.4 ms**, `read_records` incl 201.7 | `crates/graph-wasm` (`ingest::scan::read_records`), reached from `motor/session.ts:276` |
| the wire text itself: `JSON.stringify` then `TextEncoder.encode` into the wasm heap | **incl 156.8 ms** `encode`, and `wireDocument`'s stringify (149 ms of it, measured under node) | `JSON.stringify` in `ingest.ts:299` before the change; the encode is `crates/graph-sdk-js`'s `$e`, called from `motor/session.ts:276` |

Close behind, and neither reachable from `packages/graph-studio/src`: the layout
(`gm_run` incl 150.6 ms, `sugiyama::run` incl 130.2 ms, in `crates/graph-core`) and the page's own
draw (`transferToImageBitmap` self 352.3 ms, `(program)` self 387.9 ms, in `packages/graph-render`).
The page's JavaScript is not the problem: `send` self 37.7 ms is the structured clone of the 62 MB
text to the worker, and every studio-side row on the main thread is under 30 ms.

## Where the switch goes — the three largest costs

The second open is **not slow in this harness**: 51–58 ms on develop, against 7.1–10.65 s and a
crash in the hand measurement this job starts from. The worker profile of the switch is 99.3%
`(idle)` (2.01 s sampled for a 50 ms open), and its three costs are:

| cost | measured | where |
|---|---|---|
| native work on the page: the answer's structured clone deserialised and the new frame's buffers uploaded | `(program)` self **31.6 ms** of a 2.03 s profile | the message from `motor/serve.ts:42` and `graph-render`'s `setFrame` |
| drawing the 488-node graph | `transferToImageBitmap` self **15.9 ms**, `bulk` incl 18.8 ms | `packages/graph-render` |
| the new worker: spawn, wasm module start, `gm_build`, layout | `load` incl 5.5 ms, `open` incl 4.2 ms, `layout` incl 3.9 ms | `motor/client.ts:237` `loadFresh` → `session.ts:284` `open` → `session.ts:288` `load` |

The switch's own worker is a *new* one (`loadFresh` retires the one that held the large graph,
`motor/client.ts:241-243`), which is why its profile is almost empty: it has 50 ms of work to do.

## What was cut

One thing, and it is the only large cost inside `packages/graph-studio/src` on this path:
`normaliseIngest`'s per-record garbage. `normaliseIngest` incl **342.8 ms → 276.6 ms** on the same
document (same probe, same rows: `cr` incl 342.8 ms in `base-webgl2-1.txt` against `gr` incl
276.6 ms in `after-webgl2-1.txt` — the minified names moved because the module was split). Under
node, over the same document, the same function went 594 ms → 370 ms.

What it allocated per record before, and what it allocates now:

- `wireNode` was `Object.fromEntries(CONTRACT_MEMBERS.map(...))` — an array of ten two-element
  arrays and then the object, per node: **172 856 throwaway arrays** on this document, and 94 ms
  measured under node. It is now an object literal with the same ten keys in the same order
  (`ingest-record.ts:148`), so the wire text is byte-identical.
- `noteDropped` was `Object.keys(record)` plus `keep.includes(key)` — a fresh key array per node
  and per edge and a linear scan per key: 193 622 key arrays. It is now `for (const key in record)`
  against a hoisted `Set` (`ingest-record.ts:93`).
- `readNode`/`readEdge` each ran `MEMBERS.filter(...).length` to count the defaults — a fresh array
  per record. It is now `missingOf` (`ingest-record.ts:82`), which counts and allocates nothing.
- `readEdge` built `[...EDGE_MEMBERS, "type"]` per edge — 107 694 arrays of ten strings. Hoisted to
  one `Set` (`ingest-record.ts:50`).
- `readEdges` built `nodes.map(node => node.id)` — an array of 85 928 strings — and then, per edge,
  the pair-of-arrays `[["source", …], ["target", …]]` and a destructuring loop. Now one `Set` filled
  in place and two named checks (`ingest.ts:83`, `ingest.ts:89`).
- `edgeKindOf` lowercased every edge's `type`, and `EDGE_KINDS.find` scanned a fresh closure per
  edge. The three hierarchy spellings are now tested on the spelling as it stands, so an edge that
  names `kind` — every edge in this document — never lowercases anything (`ingest-record.ts:167`).

Behaviour is unchanged and pinned by `packages/graph-studio/tests/ingest-hot.test.ts` (new, 5
tests): the wire text byte for byte with the ten members in the contract's order, every note of a
ragged document in the order it happens, every refusal naming its index, and 20 000 nodes
normalising to the same text as one node at a time. No studio test was changed, weakened or skipped;
`scripts/studio.sh test` is 1414 pass / 0 fail before and after (527 + 776 + 111, of which the 776
includes the 5 new ones).

`ingest.ts` grew past the house's 300-line limit while being rewritten, so it is now three modules:
`ingest.ts` (131 lines, the document around its records), `ingest-record.ts` (201, one record and
the contract's member names) and `ingest-shape.ts` (53, the types both halves read — a third
module because the two halves import each other's types otherwise). Nothing outside
`packages/graph-studio/src/source/` changed; no import site moved, because `ingest.ts` re-exports
the shape.

## The crash — not reproduced

**It did not reproduce.** 20 probe runs, each opening the 62 MB document and then the 488-node one:
0 crashes, 0 page exceptions, 0 `Log` errors, in every arm. `memory.performance.usedJSHeapSize` does
not grow across the switch (125 MB → 123 MB on webgl2, 118 MB → 118 MB on canvas2d) and the
browser's resident memory does not either: 2139 MB → 2155 MB, of which the largest single process
went 1270 MB → 1280 MB. So on this host, with this container (`PERF_MEMORY` 10 g), the second
document is 50 ms of work and the tab survives it. The 10.1 s and the four "Target crashed" in six
attempts are not reproduced by `deploy/perf/open-document.py`, and this report does not claim they
are explained.

What the code says still holds the first document until the switch lands, which is what the 10 g
container has to survive: `state.settings.source.text` keeps all 62 MB of text
(`state/settings.ts:125-133` `sourceOf`), the worker's `normaliseIngest` peak holds the parsed tree,
the 61 MB wire text and the same text again as UTF-8 in the wasm heap at once
(`source/limits.ts:7-9` records a 97 MiB document peaking at 791 MiB to normalise), and `GraphMeta`
— five string columns plus `tags` for every node — rides back on the `laid-out` answer as a structured clone, not a transfer
(`motor/protocol.ts:67`; `motor/serve.ts:47-50` transfers `run.bytes` and nothing else). At the
switch `loadFresh` retires the worker
(`motor/client.ts:241`) and the settings are rewritten with the new source, so the references go;
what does not go back to the OS is the retired worker's high-water mark, which is why the RSS sum
is flat rather than falling.

## What was not cut, and why

- **The wire text (≈250 ms: 149 ms `JSON.stringify` under node, 156.8 ms `TextEncoder.encode` in
  the SDK, then `gm_build` reading it back in 289.4 ms).** A document leaves `normaliseIngest` as
  JSON because `gm_build` reads JSON; `documents.ts:55-63` says in as many words that the columns
  path would be "a second way to spell it". Routing documents through `gm_build_columns` would
  remove all three, and it is inside the paths this job allows — but it changes what the engine is
  given for a document, which is a decision and not a refactor. See "decisions needed".
- **`sha256Hex`'s `bytes.slice()` (`motor/session.ts:128`).** `crypto.subtle.digest` accepts the
  view, so the copy is redundant, but the profile puts the whole digest at 22 ms self including the
  SHA-256 itself; the copy is a low-single-digit-millisecond memcpy. Not worth a change to a hash
  path.
- **`GraphMeta`'s structured clone** (five string columns and `tags` for 85 928 nodes, ~500 000
  values). Cutting it means changing what `RunReport` carries, which is a message shape in
  `src/motor/protocol.ts` — the stop-list in this job.
- **The 62 MB text in `settings.source`, and the structured clone of it to the worker.** Both are
  `settings.source` and the worker lifecycle, the other two things on the stop-list.
- **The page's draw** (`transferToImageBitmap` 352.3 ms self) and the layout (130.2 ms) are in
  `packages/graph-render` and `crates/`, neither of which this job may touch.

Caveat: every number here is one probe on one host with a software rasteriser and no GPU, so the
absolute milliseconds are this machine's and not a user's; the medians are of five rounds and the
rounds vary by 300 ms on their own (the develop webgl2 column spans 1592–1972 ms for the same
binary). `open` is measured from `performance.now()` around the dispatch and therefore carries the
document's cross-origin fetch, its structured clone to the worker, the wasm module start of a worker
that begins with it and the profiler's own few percent, none of which this probe separates. The
first-frame column is not a paint latency: headless produces no BeginFrames for a page nobody
watches, so the probe asks the compositor for one frame with a 2×2 screencast and reports the wait
for it — which is why it reads 136 ms on webgl2 whatever the document is, 0 on canvas2d when a
frame is already pending, and −1 when none arrives in 9.6 s (three of the twenty rounds). The heap
column is the page's JavaScript heap only: `performance.memory` does not exist in a worker, so the
wasm heap is visible here only through the resident memory of the browser, which this probe does not
attribute to processes. And the profile rows are a 0.5 ms sampling profiler over minified
JavaScript and mangled wasm symbols: a row under a few samples is noise, and a JavaScript row names
a function in `app/dist/assets/<chunk>.js`, not in the source.

## Decisions needed

1. **Should a document leave the studio as columns rather than as JSON text?** Recommendation: yes,
   as its own job with its own gate row, not inside this one. It is worth ≈250 ms of the 1.5 s open
   and it is the only remaining large cost in `packages/graph-studio/src`; the risk is that
   `gm_build_columns` and `gm_build` are two readers of one contract, and every refusal, note and
   idempotence case in `ingest.test.ts` would have to hold on both. Needs a devil verdict before
   code.
2. **Is `RunReport.meta`'s shape allowed to change?** Recommendation: yes, in the same direction —
   transfer the typed columns (`group`, `weight`, `versions`, `degree` are already typed arrays)
   and keep only `ids`/`labels`/`tags`/`dbs`/`paths` as text, or send `tags` as one flat string
   table plus offsets. That is a `src/motor/protocol.ts` message shape, so it stops here by rule.