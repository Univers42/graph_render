# Growing a live graph from columns, not JSON (P4e)

Status: accepted, 2026-10-04, with the devil verdict **PROCEED-WITH-CONDITIONS**. The numbered
conditions in "Verdict" are P4e-motor's and P4e-sdk's acceptance criteria; no code lands until
every one of them is green. Nothing in this document is settled by the verdict alone: the
corrections below to items 2, 3 and 5 change what the slices have to build.

## Context

- P4's exit (`docs/contract/delta.md`, "Done when") is `extend` + `grow` ≤ 30 ms per 10 000-node
  batch at 1M nodes, native and wasm, median of 3 alternated rounds.
- P4d (`docs/measurements/perf-p4d-extend.md`) cut the JSON reader and the double endpoint lookup
  and still missed in all four arms: native 33.15 / 33.61 ms, wasm 54.29 / 59.78 ms (BH / PM).
- The wasm arm pays 1.76–1.87× native. What wasm does that native does not: the SDK's
  `JSON.stringify` of the batch (`crates/graph-sdk-js/src/extend.ts`) and the copy into linear
  memory. Both arms then run the same JSON walk (`ingest::read_records`), which is still the largest
  native cost P4d named.
- The columnar build already exists for whole documents: `gm_build_columns` over the GMC1 document
  (`docs/contract/ingest-columns.md`, `docs/decisions/ingest-columns.md`), decoded by
  `graph_contract::ingest_columns::decode` and indexed through `graph_core::index_columns` with an
  `EntryTable` that resolves each string once. The SDK encodes it with `encodeColumns`
  (`crates/graph-sdk-js/src/columns.ts`), one `Uint8Array` sized up front.

## Decision

1. **A batch document "GMX1"**: magic `0x31584D47`, version `1`, every section of GMC1 in the same
   order with the same rules, except `edge source` and `edge target`, which are **string indices
   naming node ids** (required; `u32::MAX` refused), not node rows. A batch's edges may name nodes
   the graph already holds, and rows would leak the dense index, which never crosses the wire. The
   distinct magic makes each reader refuse the other's bytes, so a GMC1 document handed to the
   extend export (or the reverse) is `ColumnsInvalid`, never misread.
2. **The decoder is shared, not copied**: `graph_contract::ingest_columns` gains the GMX1 variant;
   GMC1's bytes, refusals and tests are unchanged. **Corrected (devil, 2026-10-04):** the endpoint
   rule is *not* one parameter. Three types carry an endpoint as a `u32` row —
   `graph_contract::ingest_columns::EdgeCells` (`ingest_columns/row.rs:47-64`),
   `graph_contract::ingest_columns::EdgeRow` (`ingest_columns.rs:95-114`) and
   `graph_core::EdgeCells` (`graph-core/src/index/columns/cells.rs:55-74`) — and the refusal is a
   whole function with its own variant, `check::endpoint` returning `ColumnsError::EndpointRow`
   (`ingest_columns/check.rs:132-138`). GMX1's endpoint is a *string index*, so it is checked by
   `check::required` and refused as `ColumnsError::StringIndex` instead. The rule is therefore an
   enum threaded through `layout`, `check` and `row`, not a flag. The magic is the cheap half of
   this: one `pub(super) const MAGIC` (`ingest_columns/layout.rs:13`) compared once at
   `layout.rs:177` becomes a match over two.
3. **graph-core `Topology::extend_columns`** appends a decoded batch through the `EntryTable` the
   columns build uses: same validation as `Topology::extend` (C12 on the cumulative graph), validate
   first then mutate, same `ExtendError`s. After it, the topology is byte-identical to
   `Topology::extend` over the same records. **Corrected (devil, 2026-10-04):** `index_columns`
   itself is not reusable and must not be called. It builds a **new** `Topology` —
   `Topology::with_row_capacity`, `topology.strings = StringArena::with_capacity(..)`,
   `topology.finish()` (`graph-core/src/index/columns.rs:102-113`) — it admits as it walks, so a
   refusal halfway is harmless only because the half-built graph is dropped on the floor. On a
   live handle that is F2, and it is the opposite of the discipline `Topology::extend` states at
   `index/extend.rs:6-7`. So `extend_columns` reuses the *pieces*: `index_node`/`index_edge`'s
   intern order (already identical to `admit_node`/`admit_edge`'s — `id, database, source, label,
   group, icon` then `id, label, record_id`, `index/columns.rs:141-150` against
   `index/admit.rs:98-138`, which is why byte-identity is achievable at all) and `Entries`' memo,
   which are module-private today (`fn index_node`, `pub(super) struct Entries` in
   `index/columns/cells.rs:82`) and must be widened for a sibling module to reach. It must *add*
   what `index_columns` leaves to `finish()`: the three `csr.push_row()` calls, `degree.push(0)`
   and `group_node` per node (`index/extend.rs:210-214`), and `file_edge` per edge
   (`index/extend.rs:234-245`). And it must resolve a GMX1 endpoint — a string-table entry naming
   a node id — to a dense row the way `row_of` already does for an id
   (`index/extend.rs:255-261`): a probe in the cumulative graph, arithmetic for a batch node. All
   of it before the first intern, because interning writes.
4. **One additive export** `gm_graph_extend_columns(graph, ptr, len) -> u32`, `1` appended, `0`
   refused. Refusals: `InvalidHandle`, `BuildSourceInvalid` (dead buffer), `IngestTooLarge` (above
   `MAX_INGEST_BYTES`, before decoding), `ColumnsInvalid` for every format fault **and** every graph
   fault (repeated id, dangling endpoint), as `gm_build_columns` does. No new error code;
   `ABI_VERSION` stays **2** (corrected, devil 2026-10-04: it is 2, not 1 —
   `crates/graph-wasm/src/lib.rs:139`, `docs/contract/wasm-abi.md:31`,
   `crates/graph-sdk-js/src/wasm.ts:90`); C7: the same invalidation events as `gm_graph_extend`,
   i.e. the handle's snapshot and geometry are cleared (`exports/delta.rs:78-79`).
   `gm_graph_extend` is unchanged, code and behaviour.
5. **SDK, additive**: `encodeBatch(batch: GraphBatch)` beside `encodeColumns` (shared intern/write
   code, not a copy) and `Motor.extendColumns(handle, batch)`, staged like `buildColumns`, bumping
   the views as `extend` does. `Motor.extend` stays. **Corrected (devil, 2026-10-04):** "beside"
   understates it. `columns.ts`'s `Table.edgeCells` writes an endpoint as a node *row* and
   **refuses** one that names no node of the document — it throws a `ColumnsEncoderError` whose
   text names the field and reads "names no node"
   (`crates/graph-sdk-js/src/columns.ts:143-149`, the throw at `:149`). A GMX1 batch's endpoint may
   name a node the graph already holds, which the encoder cannot see, so under GMX1 that refusal
   must become an on-demand intern into the same table. That is a change to `Table`'s semantics, not a flag on
   `assembleColumns`; sharing is still right, "not a copy" is still the rule, and the assembler
   itself (`columns-assemble.ts:85-100`) is reusable unchanged because the sections are identical.
6. **Studio**: `packages/graph-studio/src/motor/session.ts:159` calls `extendColumns`.
7. **Bench**: `graph-cli tick --stream … --path json|columns` and `wasm-stream-bench.mjs --path
   json|columns` time both paths over the same stream file.

## Verdict

**PROCEED-WITH-CONDITIONS**, 2026-10-04 (devil, public ABI + public format + a hot path).
The shape is right: purely additive, one new symbol, no existing export's signature, code or
document moves, and the divergence the slice exists to prevent has a cheap decisive judge (the
stage encoding of two topologies). But this is not `PROCEED`: two of the three structural claims
about the code being reused are wrong as written (items 2, 3, 5 above), and A1/A2 are still
unmeasured. Those are conditions, not reasons to stop.

**Risk scores.**

| axis | score | reason |
|---|---:|---|
| blast radius | **4** | A shipped wasm export, a second binary format two contracts must keep in step, a new append path on the topology every layout reads, an SDK method, a studio switch. A wrong append is not a crash — it is plausible wrong geometry in all four arms, and it survives every test except the equivalence one. Not 5: nothing existing changes behaviour, and the equivalence test makes the failure loud if it is written before the code. |
| reversibility | **4** | The code and this document revert by deleting files. Two things do not: once a host calls `gm_graph_extend_columns` the symbol can never be withdrawn without an ABI major, and the `EXPORT_NAMES` bump makes every module built before it fail **at load** — one-way, and the whole motor degrades, not one method. (`delta-abi.md` scored this 3 for a change that moved `gm_run`'s existing signature; this one moves nothing, hence 4.) |
| cost on failure | **4** | Data loss, of a kind that does not announce itself: a half-appended handle cannot be un-extended, and a divergent append yields a plausible layout. Not 5 because every failure named in F1–F3 has a test that must be green first, and because `Topology::extend`'s validate-then-mutate discipline (`index/extend.rs:6-7`) is the pattern being copied. |
| confidence | **2** | Every *measured* claim verifies: the four P4d medians, `JSON.stringify` at `sdk-js/src/extend.ts:46`, `read_records` at `wasm/src/service.rs:57`, `EntryTable`, `index_columns`, `ColumnsInvalid = 23`. But `index_columns` is not an append path, the endpoint cell's type has to change, and `Table.edgeCells` refuses the endpoint GMX1 needs — so the plan understates the work by roughly the whole motor slice. A1/A2 remain unmeasured. |

**The worst axis is blast radius** (4, tied with cost on failure at 4, and worse in kind: the
failure is silent). Confidence at 2 is the reason this is not a clean `PROCEED`.

### U1 — `ColumnsInvalid` for both fault classes: **upheld**

`ColumnsInvalid`, for the format fault *and* for the graph fault. The reason is that
`ColumnsInvalid`'s published meaning is already "the columnar path refused this buffer", stated in
two clauses — the decoder refused the document, or the graph refused what the document describes
(`crates/graph-wasm/src/errors.rs:108-118`, and `wasm-abi.md`'s "Three build paths" table). GMX1
is the same two clauses with a different verb, so the code's *meaning* does not change and
`wasm-abi.md:31`'s "a refusal code's meaning changes" never fires; no ABI bump, no new code, and
condition 11 below is what keeps the doc honest. `IngestInvalid` is the worse answer for a reason
the house has already written down twice (`errors.rs:63-66` and `:114-118`): it means "text that is
not JSON or is JSON of the wrong shape", so putting it on a binary document "would send a host
looking for a bad member in a binary document". Widening `IngestInvalid` to cover a graph fault
instead is worse still, because that *is* a meaning change and forces `ABI_VERSION` to 3 — the one
thing this plan is right to want to avoid. The cost of the ruling is real and must be paid
explicitly: `gm_graph_extend` keeps publishing `IngestInvalid` for a repeated id and a dangling
endpoint (`exports/delta.rs:73-81`, `service.rs:56-61`, `delta.md:141`), so the two extend exports
give **different codes for the same logical refusal**, and `sdk-js/src/extend.ts:30-32` throws
`BuildRefusedError` where `extendColumns` must throw `ColumnsRefusedError`
(`sdk-js/src/errors.ts:78-86`). A host that swaps one call for the other silently loses its error
handling unless the class doc and the contract row both say so. Conditions 11 and 15 are that
cost, priced.

### The magic: **GMX1, upheld**, and `0x31584D47` is correct

`0x31584D47` is the four bytes `"GMX1"` read little-endian, exactly as `0x31434D47` is `"GMC1"`
(`ingest-columns.md:15`, `ingest_columns/layout.rs:12-13`); recomputed both, they agree. A distinct
magic beats a GMC1 version or flag on one specific ground: **the magic is the only header word that
can never be reused.** The version field cannot carry the job, because both formats legitimately
want version `1` and GMC1's own section table is built to grow (`layout.rs:189-190` names
`node_count` and friends from header words) — a future GMC1 v2 would then be *accepted* by a GMX1
reader that shares its magic, or refused only as "bad version", which is a refusal reason that
cannot name which format was meant. With a distinct magic the two formats have separate namespaces
and can each evolve; with a flag they share one and only one bit of it. The refusal reason is also
nameable per format, which is the same reason `ContractInvalid` and `ColumnsInvalid` exist
(`errors.rs:55-67`). The cost is one constant and one comparison (`layout.rs:13`, `:177`) plus two
tests, and it is smaller than the flag's cost once either format changes. Condition 2 makes the
refusal *proven* rather than asserted: the doc's claim that "each reader refuses the other's bytes"
is currently a claim, and the precedent for pinning it is
`crate::contract::tests::the_two_ingest_formats_are_not_interchangeable` (`wasm-abi.md:413-415`).

### The conditions — P4e-motor and P4e-sdk acceptance criteria

Written before the code, so they can fail it.

**P4e-motor**

1. **The decoder shares, it does not fork.** `graph_contract::ingest_columns` takes the format as
   an enum threaded through `layout`, `check` and `row`. Proof: `git grep -n 'fn endpoint' crates/
   graph-contract/src/ingest_columns` prints one function with two arms, and the GMC1 arm is the
   code that is there today. A second `check_*.rs` or a `gmx.rs` beside `check.rs` fails this.
2. **Both directions of "the other format's bytes are refused" are pinned**, modelled on
   `crate::contract::tests::the_two_ingest_formats_are_not_interchangeable`: a GMC1 document at
   `gm_graph_extend_columns` is `ColumnsInvalid`, and a GMX1 document at `gm_build_columns` is
   `ColumnsInvalid`. Both asserted on the *code*, not just on the refusal.
3. **`GMC1`'s own tests are unchanged and green** — `crates/graph-contract/src/ingest_columns/tests.rs`
   and the `gm_build_columns` export tests, `git diff` over those paths empty except for additions.
   This is F3's judge and the only thing standing between "shared" and "quietly changed".
4. **The equivalence test**, named for the one it joins: `extend_columns_matches_extend`, in
   `crates/graph-core/src/index/extend/tests.rs` beside the existing `extend_matches_index_model`
   (`:91`). It reuses that file's seeded harness rather than inventing one — `stream(seed)` over
   seeds `0..96` (`:48`), `Mt19937`, no new randomness (D1/D10) — and its judge is that file's:
   `bytes()` = `crate::stage::topology_bytes` (`stage.rs:133`, at `:67`), i.e. "the topology
   stage's bytes", plus `stats()` and every
   `node_index`/`edge_index`, exactly as `assert_matches_rebuild` (`:72-88`) does. `extend_columns`
   over each batch must leave the same bytes as `extend` over the same records. This is the
   load-bearing test of the slice: written, and observed RED, before `extend_columns` exists.
5. **One refusal test per refusal row, each asserting the topology is byte-identical afterwards.**
   Extend the existing `refusals()` table (`:110`) and `extend_refusal_leaves_topology_unchanged`
   (`:146`) instead of writing a second pair, and add the export's own rows: `InvalidHandle`,
   `BuildSourceInvalid`, `IngestTooLarge`, bad magic, bad version, nonzero reserved, every
   `check::check` refusal, string index out of range, `u32::MAX` in a required column, boolean `2`,
   non-finite, nonzero pad, short/long buffer, duplicate node id, duplicate edge id, dangling
   endpoint. `ExtendError::Capacity` is already covered on the JSON side by
   `a_batch_that_could_overflow_a_count_is_refused_before_anything_is_counted_in` (`:167`) and needs
   its columns twin — it is the case F2 is really about and the one a reader forgets, because the
   overflow is the only refusal whose check is a sum over strings rather than one row.
6. **Validate first, then mutate**, proven by test 5 rather than by comment: every node id and
   every endpoint row is resolved before the first `intern`, because interning writes to the arena
   and a refusal after it cannot be rolled back. `git grep -n 'index_columns' crates/graph-core/src/index/extend`
   prints nothing — that function builds a fresh `Topology` (`index/columns.rs:102-113`) and must
   never be called on a live handle.
7. **The pieces are shared, not copied.** `index_node`/`index_edge`/`Entries` are reached by
   visibility (`pub(in crate::index)`), not reimplemented; `git grep -c 'NodeKind::from_name'
   crates/graph-core/src/index` stays at its current count. The intern order is asserted equal to
   `admit_node`/`admit_edge`'s, since byte-identity rests on it
   (`index/columns.rs:141-150` against `index/admit.rs:98-138`).
8. **House limits.** `crates/graph-core/src/index/extend.rs` is 264 lines today, so `extend_columns`
   goes in a child module (`index/extend/columns.rs`) rather than being squeezed in; every new
   function ≤ 40 lines, ≤ 4 parameters, every new file ≤ 300 lines.
9. **No new dependency** in any `Cargo.toml` (root and workspace members), and **no `unsafe`** beyond
   the existing export pattern: one `is_live` check immediately before the one slice formed from
   it, with the same `// SAFETY:` comment shape as `exports/delta.rs:31-47`.
10. **`force-gate`'s stream stage runs over the columns path** and its negative control
    `GM_MUTATE_DROP_DELTA=1` is red — F1's second judge. Two commands, both in the rows file:
    `scripts/orch/gr cargo run -q -p graph-cli -- force-gate --seeds 4` (0) and
    `scripts/orch/gr -e GM_MUTATE_DROP_DELTA=1 cargo run -q -p graph-cli -- force-gate --seeds 4; test $? -eq 1`.
    The stage needs a columns arm to exist first (`forcecheck/stream.rs:125` calls `service::extend`).
11. **The ABI documents say it**: `docs/contract/wasm-abi.md` gains the export row with its
    refusal order, and its Errors row for `ColumnsInvalid` gains the extend clause; `docs/contract/
    delta.md` gains the columns batch and the asymmetry (same faults, `ColumnsInvalid` here and
    `IngestInvalid` under `gm_graph_extend`) stated in words; `docs/contract/ingest-columns.md` (or
    its successor) documents GMX1's endpoint rule beside the dense-row rule.
12. **`codegen --check` and `capabilities --check` are green**, and `errors/mirrors.rs` still passes
    (`mirrors.rs:14` `include_str!`s `wasm-abi.md`, so a stale table is a red test, not a stale doc).

**P4e-sdk**

13. **The SDK names the new export and bumps nothing else.** `RawExports` and `EXPORT_NAMES` gain
    `gm_graph_extend_columns` (`sdk-js/src/wasm.ts:65-80`), `ABI_VERSION` stays `2`
    (`sdk-js/src/wasm.ts:90`), and the existing test that refuses an older module by name still
    refuses it by the *new* name (`docs/decisions/ingest-columns.md:60`, criterion 7, re-run).
14. **The view bump is real, not inherited by accident.** `buildStaged` bumps on success
    (`staging.ts:133`, `:169`), so `extendColumns` must go through that same path, and
    `kinds.delete(handle)` (`extend.ts:48`) must be repeated as `extendGraph` does — a method that
    appends and forgets to invalidate the handle's kind cache is the quiet failure here. An
    `sdk:test` reads a fresh view after an `extendColumns`, and a stale-view negative control is red.
15. **`ColumnsRefusedError`, not `BuildRefusedError`.** `extendColumns`'s refuse arm maps
    `ColumnsInvalid` to `ColumnsRefusedError`, and that class's doc (`errors.ts:78-86`) is widened
    to say it covers both `gm_build_columns` and `gm_graph_extend_columns`. A test asserts the class,
    so U1's cost cannot be paid by accident.
16. **The encoder refuses a lone surrogate by field name, as `encodeColumns` does**
    (`columns.ts:114-116`), and a test round-trips `encodeBatch` → `gm_graph_extend_columns`.
17. **The studio switch is guarded like the one it replaces.** `session.ts:157-159` already tests
    `motor.extend === undefined`; the columns path gets the same guard, so an older SDK in the
    studio throws `SessionRefusal` (the queue's `failed` result) rather than a `TypeError`.
18. **The 1M measurement is recorded either way.** `graph-cli tick --stream … --path json|columns`
    and `harness/wasm-stream-bench.mjs --path json|columns`, same stream file, median of 3
    alternated rounds, p95 and max and host load beside it, into
    `docs/measurements/perf-p4e-extend.md`. A miss is a miss: it is recorded as a miss with the next
    cost named, the JSON export stays, and the row stays `implemented` (F4). A1 and A2 are answered
    by this run, not by assertion.

## Assumptions (unverified until the slice measures them)

- A1. The JSON walk plus `JSON.stringify` is most of the wasm premium. P4d attributed it; it did not
  isolate it. If encoding columns in JS costs what `stringify` did, wasm gains only the walk.
- A2. `index_columns`' `EntryTable` path is cheaper per record than the P4d JSON path at 10k rows.
- A3. The studio's batches are already `GraphBatch`-shaped (`session.ts:159`), so the switch is one
  call.

## Failure modes

- F1. Divergence: the columns path appends something other than the JSON path. Judge: a test that
  `extend_columns` over random strict streams leaves a topology whose stage encoding equals
  `extend` over the same records, and the **stream stage of `force-gate`** run over the columns
  path (corrected, devil 2026-10-04: the hash gate has no stream arm — the stream stage is
  `crates/graph-cli/src/forcecheck/stream.rs`, whose per-batch append is `service::extend` at
  `stream.rs:125`, and whose negative control is `GM_MUTATE_DROP_DELTA`
  (`hashgate/knob/kind.rs:290`, `hashgate/knob/arms.rs:126`); `docs/contract/delta.md:202` names
  it "the `force-gate` stream stage").
- F2. A partial append on a refused batch. Judge: one refusal test per row of the refusal table,
  each asserting the topology is unchanged.
- F3. GMC1 regressions from sharing the decoder. Judge: GMC1's existing tests unchanged and green.
- F4. The miss persists. Then it is recorded as a miss with the next cost named, the JSON export
  stays, and the row stays `implemented`.

## Unknowns

- U1. Whether `ColumnsInvalid` for graph faults is right, or whether they should stay
  `IngestInvalid` as `gm_graph_extend` gives them. Proposed: `ColumnsInvalid`, so the code names the
  reader that refused. **Ruled: upheld** — see "Verdict" above.
- U2. Whether the 30 ms target is reachable natively at all on this host: P4d's native floor is
  33 ms with the JSON walk; the columns path removes the walk, not the index work.

## Slices

- **P4e-motor**: the contract decoder variant and its tests, `Topology::extend_columns` and its
  tests, the export and its refusal tests, `docs/contract/delta.md` and `ingest-columns.md`, the
  native `--path columns` arm.
- **P4e-sdk** (after P4e-motor lands): `encodeBatch`, `Motor.extendColumns`, `sdk:test`, the wasm
  `--path columns` arm, the studio switch, the 1M measurement and its report.

## Result (P4e-sdk)

Conditions 13–17 green: the export is named (`ABI_VERSION` still 2), `extendColumns` goes through
`buildStaged` and bumps, its refusals are `ColumnsRefusedError`, `encodeBatch` shares `Table` and
the assembler, and the studio prefers it with both guards. Condition 18's eight-arm run is in
[`perf-p4e-extend.md`](../measurements/perf-p4e-extend.md): **native 23.92 / 19.34 ms — the budget
is met on both engines' native side; wasm 55.12 / 56.74 ms — still missed**, and the JSON arms are
unchanged in behaviour and stay. **A2 confirmed; A1 refuted in its strong form** — the wasm arm
gave up less than the native arm did, so the walk-plus-serialize is not most of the wasm premium;
timing `encodeBatch` alone is the next measurement and this run does not have it.

## Early read (P4e-motor)

One round, native Barnes-Hut, 1M nodes, 10 × 10 000-node batches, `--from target/bench/p4e-1m.jsonl`, `GR_MEM=12g`; not P4e-sdk's 3-round median, no wasm arm, and A1/A2 answered in the columns path's favour on this round only. The two `extend` columns measure different spans: `json` reads *and* appends, `columns` decodes and appends with its read and encode untimed.
```
| `--path`         | extend median      | grow median | sum median                         | load start → end |
| `json` (two runs) | 28.80 / 29.10 ms | 3.54 / 3.64 ms | 32.48 / 33.75 ms (over the budget) | 9.51→8.20, 13.15→12.98 |
| `columns`        | 16.35 ms           | 5.15 ms     | **21.79 ms**, under the 30 ms budget | 9.49→11.80 |
```

## Result (P4f)

[`perf-p4f-wasm.md`](../measurements/perf-p4f-wasm.md) times `encodeBatch` on its own: it is
**84 % / 79 % of the wasm32 `extend` timer** (31.18 / 29.52 ms of 37.33 / 37.14), so the premium
was never the wasm motor — the copy into linear memory is 0.25 % and the motor is the whole of
the 6–8 ms left over. Removing the encoder's wasted work (a field path per field per row, a
discarded `join` of the whole table, a second measurement of every entry) leaves the GMX1 bytes
byte-identical, pinned by a literal and a SHA-256 with a verified negative control.

## Result (P4g)

[`perf-p4g-wasm.md`](../measurements/perf-p4g-wasm.md): growing the mesh in place cuts wasm PM `grow`
10.02 → 3.93 ms, and a `charCodeAt` width walk plus one `encodeInto` over the joined table cut
`encode` ~5 ms, GMX1 bytes unchanged (the "never joined" test became "one `encodeInto` over the
join", deliberately). Against `75c885b6` at load 4–7: wasm BH 33.42 → 28.86, PM 38.05 → 28.36 ms —
met on a quiet host, 1.1–2.4 ms headroom, missed under load. The arena hash stays (probe 70 % of `find`).
