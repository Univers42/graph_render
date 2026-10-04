# Columnar ingest: `gm_build_columns`

**Status:** decided, with conditions. **Date:** 2026-10-03.

## Context

A 1M-node, 3M-edge `gm_build` takes 6.73 s median (`docs/measurements/perf-open-index.md`):
`read_records` 3.8 s (JSON parse 2.23 s, record strings about 1.4 s) and `index_model` 2.23 s
(endpoint `StringArena::find` 1.08 s). The studio's `JSON.stringify` adds 1.25 s. The target is
real-time 1M-node graphs, including graphs fed by a program (P4).

## Decision

An additive export, `gm_build_columns(ptr, len)`, over a little-endian binary document
(`docs/contract/ingest-columns.md`): u32 integers, one UTF-8 string table with u32 offsets,
`u32::MAX` for an absent optional field, and edge endpoints given as node row numbers. The decoder
lives in `graph-contract`. `graph-core::index_columns` shares the admit path with `index_model` and
refuses a duplicate id instead of dropping it. Refusals use a new code `ColumnsInvalid`, appended
after the highest code on develop when it merges (22 at the time of writing: `ux-params-abi` holds
20 and 21). The ABI stays at 1: the export is additive (rule in `wasm-abi.md`, precedent
`gm_build_contract`).

## Alternatives

- (a) Faster JSON only (an open-addressing arena, a direct row map): saves part of 1.7 s; the
  2.23 s parse, the 1.4 s of record strings and the 1.25 s stringify stay. Still worth doing,
  because it helps file imports.
- (b) Generate the synthetic graph inside the motor: helps synthetic graphs only, not programmatic
  feeds, and makes a second generator to keep equal to `synthetic.ts`.
- (c) A streaming JSON reader: removes the value tree and part of the 584 ms of malloc, but still
  scans 678 MB of text and still needs the stringify; the strict reader would need a new
  differential. An estimated saving of 1.5 s at most, not measured.

## Consequences

Two ingest paths. Drift is checked by a whole-`Topology` differential, on native and on wasm32,
over a corpus that must cover every field, with a negative control. Endpoints given as row numbers
require duplicates to be refused. The hash gate still covers only `gm_build`. If the measured
median saving is under 3 s, the studio stays on JSON.

## Acceptance criteria (the risk verdict's conditions)

1. The branch starts from `perf-open-index` and merges develop once that lands.
2. One admit path: `admit_node` takes `&NodeView`; `admit_edge` splits into endpoint resolution and
   one shared push. hashgate-8 and its negative control, oracle-diff and the arena-order test stay
   green. Amended by `perf-open-intern`: `index_columns` admits through `claim_*`/`push_*`
   (`index/admit.rs`) over interned cells; `index_model` keeps `admit_node`/`admit_edge`.
3. `index_columns` (`graph-core/src/index/columns.rs`) refuses a taken node or edge id, compared
   through the arena by content. Tests: the same id under two table indices, a duplicate in the
   last row, an edge pointing at the second duplicate.
4. The decoder (`graph-contract`) hands out borrowed `&str`, no `unsafe`, no new dependency. Before
   any allocation sized from the header, the section sizes are computed in checked `u64` and must
   equal the buffer length exactly. Offsets never decrease and the last equals the blob length;
   the blob is checked once with `from_utf8` and sliced with `str::get`. Every string index and
   endpoint is in range; booleans are 0 or 1; every f64 is finite (−0 and subnormals accepted, as
   `ingest.rs` does); counts stay below `u32::MAX`. One refusal test per check.
5. Kinds travel as string-table entries resolved by `from_name`.
6. `ColumnsInvalid` is appended to the Rust enum, the Errors table and the SDK's `CODE_NAMES`;
   `errors/mirrors.rs` keeps the three equal.
7. No version bump: `RawExports` and `EXPORT_NAMES` gain the export, a test refuses an older
   module by name, `wasm-abi.md` gains the export row, a build-paths column and a Coverage row,
   and `docs/contract/ingest-columns.md` specifies the format.
8. A differential compares `format!("{:?}")` of the whole `Topology` from both paths over n220 and
   generated documents that cover every field class (asserted), on native and on the wasm32
   artifact under Node. Mutated documents are refused with `ColumnsInvalid`, never a panic.
9. The negative control is a mutation inside the test that turns the differential red. No
   environment variable is read in `graph-core` or `graph-contract`.
10. The SDK encoder refuses a string that fails `isWellFormed()`; a test round-trips the export
    through JSON and `gm_build` with a −0 weight in the input.
11. Measured: four alternating runs per arm on one 1M document in `docs/measurements/`. Under a
    3 s median saving, the studio stays on JSON.
12. House limits: 40 lines a function, 300 a file, clippy `-D warnings`.
13. The byte ceiling of `wasm-ingest-limits.md` (F-16) applies: a buffer longer than
    `MAX_INGEST_BYTES` (`graph-wasm/src/ingest.rs`) is refused with `IngestTooLarge` before it is
    decoded, and the exact-size check of criterion 4 guards the header inside that bound.
