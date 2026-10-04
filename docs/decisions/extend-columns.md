# Growing a live graph from columns, not JSON (P4e)

Status: proposed, 2026-10-04. Needs the devil verdict (public ABI) before any code.

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
2. **The decoder is shared, not copied**: `graph_contract::ingest_columns` gains the GMX1 variant
   behind one parameter (the endpoint rule); GMC1's bytes, refusals and tests are unchanged.
3. **graph-core `Topology::extend_columns`** appends a decoded batch through the `EntryTable` the
   columns build uses: same validation as `Topology::extend` (C12 on the cumulative graph), validate
   first then mutate, same `ExtendError`s. After it, the topology is byte-identical to
   `Topology::extend` over the same records.
4. **One additive export** `gm_graph_extend_columns(graph, ptr, len) -> u32`, `1` appended, `0`
   refused. Refusals: `InvalidHandle`, `BuildSourceInvalid` (dead buffer), `IngestTooLarge` (above
   `MAX_INGEST_BYTES`, before decoding), `ColumnsInvalid` for every format fault **and** every graph
   fault (repeated id, dangling endpoint), as `gm_build_columns` does. No new code; `ABI_VERSION`
   stays 1; C7: the same invalidation events as `gm_graph_extend`. `gm_graph_extend` is unchanged.
5. **SDK, additive**: `encodeBatch(batch: GraphBatch)` beside `encodeColumns` (shared intern/write
   code, not a copy) and `Motor.extendColumns(handle, batch)`, staged like `buildColumns`, bumping
   the views as `extend` does. `Motor.extend` stays.
6. **Studio**: `packages/graph-studio/src/motor/session.ts:159` calls `extendColumns`.
7. **Bench**: `graph-cli tick --stream … --path json|columns` and `wasm-stream-bench.mjs --path
   json|columns` time both paths over the same stream file.

## Assumptions (unverified until the slice measures them)

- A1. The JSON walk plus `JSON.stringify` is most of the wasm premium. P4d attributed it; it did not
  isolate it. If encoding columns in JS costs what `stringify` did, wasm gains only the walk.
- A2. `index_columns`' `EntryTable` path is cheaper per record than the P4d JSON path at 10k rows.
- A3. The studio's batches are already `GraphBatch`-shaped (`session.ts:159`), so the switch is one
  call.

## Failure modes

- F1. Divergence: the columns path appends something other than the JSON path. Judge: a test that
  `extend_columns` over random strict streams leaves a topology whose stage encoding equals
  `extend` over the same records, and the stream arm of the hash gate run over the columns path.
- F2. A partial append on a refused batch. Judge: one refusal test per row of the refusal table,
  each asserting the topology is unchanged.
- F3. GMC1 regressions from sharing the decoder. Judge: GMC1's existing tests unchanged and green.
- F4. The miss persists. Then it is recorded as a miss with the next cost named, the JSON export
  stays, and the row stays `implemented`.

## Unknowns

- U1. Whether `ColumnsInvalid` for graph faults is right, or whether they should stay
  `IngestInvalid` as `gm_graph_extend` gives them. Proposed: `ColumnsInvalid`, so the code names the
  reader that refused.
- U2. Whether the 30 ms target is reachable natively at all on this host: P4d's native floor is
  33 ms with the JSON walk; the columns path removes the walk, not the index work.

## Slices

- **P4e-motor**: the contract decoder variant and its tests, `Topology::extend_columns` and its
  tests, the export and its refusal tests, `docs/contract/delta.md` and `ingest-columns.md`, the
  native `--path columns` arm.
- **P4e-sdk** (after P4e-motor lands): `encodeBatch`, `Motor.extendColumns`, `sdk:test`, the wasm
  `--path columns` arm, the studio switch, the 1M measurement and its report.
