# Job perf-open-columns (agent build: a binary columnar build export, `gm_build_columns`, beside `gm_build`)

Why: a 1M-node, 3M-edge `gm_build` takes 6.73 s median (`docs/measurements/perf-open-index.md`).
3.8 s is `read_records` (JSON parse 2.23 s, per-field `String`s about 1.4 s) and 2.23 s is
`index_model`, of which 1.08 s is the 6M endpoint `StringArena::find` calls. A binary document with
one string table and endpoints given as node row numbers removes the parse, the record strings and
the endpoint lookups. The decision and its 13 acceptance criteria are
`docs/decisions/ingest-columns.md`: read it first, every criterion is a done-when of this job.

## The format (write it to `docs/contract/ingest-columns.md` first, then code to it)

Little-endian throughout. Every section starts where the previous one ends; the total must equal
the buffer length exactly (criterion 4).

| Section | Type, count | Meaning |
|---|---|---|
| header | 8 × u32 | magic `0x31434D47` ("GMC1"), version `1`, node_count, edge_count, string_count, blob_len, 0, 0 (reserved, must be 0) |
| offsets | u32 × (string_count + 1) | `offsets[0] = 0`, non-decreasing, last = blob_len |
| blob | blob_len bytes | UTF-8, checked once with `str::from_utf8` |
| pad | 0–7 bytes | zeros, so the next section starts at a multiple of 8 from the buffer start |
| node f64 | f64 × node_count, twice | weight, version |
| edge f64 | f64 × edge_count | strength |
| node u32 | u32 × node_count, 8 columns | id, kind, database_id, source, label, group, icon (string indices; `u32::MAX` = absent, only for database_id, group, icon), has_note (0 or 1) |
| edge u32 | u32 × edge_count, 8 columns | id, source_row, target_row (node row numbers), kind, label (string indices), record_id (string index or `u32::MAX`), directed, child_first (0 or 1) |

Kinds are string-table entries resolved with `NodeKind::from_name` (`graph-core/src/columns.rs:39`)
and `EdgeKind::from_name` (`graph-core/src/edgekind.rs:42`), so no numeric tag needs a mirror.

## Do, in order (one commit per step is fine; `git commit` is the gate script's, not yours)

1. **One admit path** (`crates/graph-core/src/index.rs`). `admit_node` takes `&NodeView`
   (`records.rs:60`, `NodeRecord::view` at `records.rs:108`). `admit_edge` splits into endpoint
   resolution and one shared push that takes the resolved `(source, target)` dense indices plus an
   `&EdgeView`. `index_model` keeps its behaviour byte for byte: run
   `scripts/orch/gr cargo test -p graph-core` and `hashgate --seeds 8` before going on. The test
   `the_arena_holds_kept_strings_once_in_first_seen_order` must stay green unchanged.
2. **Decoder** in `crates/graph-contract/src/ingest_columns.rs` (+ a child module if it passes 300
   lines): `pub fn decode(bytes: &[u8]) -> Result<ColumnsDoc<'_>, ColumnsError>`. `ColumnsDoc`
   borrows the blob and hands out `&str` by string index and the column values by row. No
   `unsafe`, no new dependency, no allocation sized from the header before the exact-size check
   (checked `u64` arithmetic). Every refusal in criterion 4 has its own test in
   `crates/graph-contract/src/ingest_columns/tests.rs`: short buffer, long buffer, bad magic, bad
   version, nonzero reserved, decreasing offset, last offset ≠ blob_len, invalid UTF-8, a slice
   that splits a code point (`str::get` returns `None`), string index out of range, `u32::MAX`
   in a required field, endpoint row ≥ node_count, a boolean 2, a NaN and an infinity in each f64
   column, nonzero padding, a count of `u32::MAX`. −0.0 and a subnormal are accepted.
3. **`index_columns`** in `crates/graph-core/src/index/columns.rs`. graph-core cannot see
   graph-contract, so it takes what graph-core owns: an `ExactSizeIterator<Item = NodeView<'a>>` and
   an `ExactSizeIterator` of a new `RowEdge<'a>` (an `EdgeView` whose endpoints are `u32` rows),
   and returns `Result<Topology, ColumnsRefusal>`. It uses step 1's admit path and **refuses** a
   taken node id or edge id (the `insert_full` answer is `fresh == false`) instead of dropping
   it, so a node's row equals its dense index and a row endpoint is its index. Tests: the same id
   under two string indices, a duplicate in the last row, an edge whose endpoint is the row of the
   second duplicate (refused, not attached to the first).
4. **Export** `gm_build_columns(ptr, len) -> handle` in `crates/graph-wasm/src/exports/build.rs`
   beside `gm_build`, same handle lifecycle. A `len` over `MAX_INGEST_BYTES`
   (`graph-wasm/src/ingest.rs:60`) is refused with `IngestTooLarge` before decoding. A decoder or
   `index_columns` refusal is the new code `ColumnsInvalid`, appended after the highest code: on
   this base that is 20 (`IngestTooLarge = 19`). Write `20` now; `ux-params-abi` also claims
   20–21, and whichever lands second renumbers at merge (note it in the report). The code goes
   into `errors.rs`, the Errors table of `docs/contract/wasm-abi.md` and `CODE_NAMES` in
   `crates/graph-sdk-js/src/errors.ts`; `crates/graph-wasm/src/errors/mirrors.rs` checks all three.
5. **Differential** `crates/graph-wasm/src/ingest/tests/columns.rs` (native): for every document
   in the corpus, `format!("{:?}")` of the `Topology` from `read_records` + `ingest::index` equals
   the one from the columns path. Corpus: `fixtures/scale/n220.json` plus generated documents that
   between them hold a non-null `record_id`, a null and a non-null `database_id`, `group` and
   `icon`, every node kind and edge kind, a nonzero `version`, a negative weight, a −0.0 weight,
   `child_first` true, `directed` false, and a multi-byte id. A test asserts each of those field
   classes appears in the corpus. The encoder used by the native test is a small Rust function in
   the test module that turns `NodeRecord`/`EdgeRecord` slices into the bytes (dedupe strings
   through a `BTreeMap<&str, u32>` or an `IndexMap`; never `HashMap`, D4). Negative control
   (criterion 9): a `#[test]` that flips one edge's `child_first`, or rotates one endpoint row,
   in the encoded bytes and asserts the two `Debug` strings **differ**. Mutated documents fed to
   `gm_build_columns` return `ColumnsInvalid`, never a panic.
6. **SDK** (`crates/graph-sdk-js/src`): `encodeColumns(doc)` in a new file `columns.ts`
   (the `index.ts` file is already at the 300-line limit, so add only the method wiring there),
   using `TextEncoder.encodeInto` into one `Uint8Array` sized up front, and refusing a string for
   which `String.prototype.isWellFormed()` is false (throw a `GraphMotorError` named for the
   field). `buildColumns(bytes)` stages through `buildStaged` (`staging.ts`, widen its `call`
   union). Add `gm_build_columns` to `RawExports` and `EXPORT_NAMES` (`wasm.ts:24,59`) and to the
   export list in `crates/graph-sdk-js/test/abi-version.test.mjs:13`. Test
   `crates/graph-sdk-js/test/columns.test.mjs` over the real release artifact
   (`target/wasm32-unknown-unknown/release/graph_wasm.wasm`): the same document through `build`
   (JSON) and `buildColumns` gives equal `gm_node_count` and equal snapshot bytes
   from one deterministic layout run (pick a layout with no RNG from `Motor.layouts()`); a
   document with a −0 weight round-trips; a lone surrogate id is refused by the encoder; a module
   without `gm_build_columns` is refused by name.
7. **Docs**: `docs/contract/ingest-columns.md` (the table above, the refusals, the dense-row
   rule), and in `wasm-abi.md` the export row, a third column in "Two build paths" (rename the
   heading to name three), and a Coverage row. No version bump (`wasm-abi.md:31`).
8. **Measure** (criterion 11). Copy `/mnt/storage/bench/perf-keep/open-bench.mjs` to
   `target/bench/` and add a columns arm: read
   `/mnt/storage/bench/perf-keep/random-n1000000-d3-p0.json`, parse it in Node, time
   `encodeColumns`, then time `gm_build_columns`; the JSON arm times `gm_build` on the text (as
   today) and reports `JSON.stringify` of the parsed document separately (the studio pays it).
   Build the release artifact once, then 4 runs per arm, alternated, one process per run. Write
   `docs/measurements/perf-open-columns.md`: the commands, every run, the medians, wasm pages,
   and a verdict line: if the columns arm (encode + build) does not beat the JSON arm
   (stringify + build) by at least 3 s at the median, say so — the studio then stays on JSON.

## Out of bounds

`packages/`, `app/`, `deploy/`, `src/` (the studio wiring is a later job, after the measurement).
`crates/graph-core` gains no dependency and no `unsafe`; graph-core and graph-contract read no
environment variable. Do not change `gm_build`'s behaviour, the JSON reader, or any existing error
code number. House limits: 40 lines a function, 4 parameters, 300 lines a file, nesting 3. No type
assertions in TypeScript. Comments say why; every heuristic carries a `Caveat:` line.

## Done when

- `scripts/orch/gate.sh target/gate-columns scripts/orch/rows/perf-open-columns.rows` is all PASS.
- The differential and its negative control both run in `cargo test` (the control asserts the
  difference, so it passes when the differential would catch the mutation).
- `docs/measurements/perf-open-columns.md` holds the 4 + 4 runs and the verdict line.
- The return block lists each of the 13 criteria of `docs/decisions/ingest-columns.md` with the
  file:line or test name that meets it, or "not met" and why.
