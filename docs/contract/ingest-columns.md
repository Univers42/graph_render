# The columnar ingest document (`gm_build_columns`)

The binary document `gm_build_columns(ptr, len)` reads. Additive: `gm_build`'s provisional
JSON and `gm_build_contract`'s ingest contract are untouched, and each reader refuses the
other formats (`docs/decisions/ingest-columns.md`).

Little-endian throughout. Every section starts exactly where the previous one ends. The
declared section sizes must add up to the buffer length **exactly** — a buffer one byte short
or one byte long is refused, never read to the end and never truncated.

## Sections

| Section | Type, count | Meaning |
|---|---|---|
| header | `u32` × 8 | magic `0x31434D47` (`"GMC1"`) or `0x31584D47` (`"GMX1"`, the extend batch — "The batch format `GMX1`" below), version `1`, `node_count`, `edge_count`, `string_count`, `blob_len`, `0`, `0` (reserved, must be `0`) |
| offsets | `u32` × (`string_count + 1`) | `offsets[0] = 0`, non-decreasing, last `= blob_len` |
| blob | `blob_len` bytes | UTF-8, checked once with `str::from_utf8` |
| pad | 0–7 bytes | zeros, so the next section starts at a multiple of 8 from the buffer start |
| node weight | `f64` × `node_count` | `weight` |
| node version | `f64` × `node_count` | `version` |
| edge strength | `f64` × `edge_count` | `strength` |
| node id | `u32` × `node_count` | string index |
| node kind | `u32` × `node_count` | string index |
| node database | `u32` × `node_count` | string index, `u32::MAX` = absent |
| node source | `u32` × `node_count` | string index |
| node label | `u32` × `node_count` | string index |
| node group | `u32` × `node_count` | string index, `u32::MAX` = absent |
| node icon | `u32` × `node_count` | string index, `u32::MAX` = absent |
| node has_note | `u32` × `node_count` | `0` or `1` |
| edge id | `u32` × `edge_count` | string index |
| edge source | `u32` × `edge_count` | node **row** in `GMC1`; string index naming a node **id** in `GMX1` (required) |
| edge target | `u32` × `edge_count` | as `edge source` |
| edge kind | `u32` × `edge_count` | string index |
| edge label | `u32` × `edge_count` | string index |
| edge record_id | `u32` × `edge_count` | string index, `u32::MAX` = absent |
| edge directed | `u32` × `edge_count` | `0` or `1` |
| edge child_first | `u32` × `edge_count` | `0` or `1` |

Columns are stored one after another in the order above — structure of arrays, not a struct
of arrays per row. Reading row `r` of a column is one `u32` at `base + 4 * r`.

### Strings

`text(i)` is `blob[offsets[i] .. offsets[i + 1]]`. Two adjacent entries may hold the same
bytes (the encoder may or may not dedupe); the arena interns by content, so a duplicate costs
nothing downstream. `offsets[0]` is `0`, entries never decrease and the last one is `blob_len`,
so the slices exactly tile the blob. The blob is checked once, with `str::from_utf8`, and each
entry is then taken with `str::get`, so an entry that splits a code point is refused rather
than panicking on the `&str` boundary.

`u32::MAX` in an *optional* column means the field is absent, exactly as the JSON reader's
`null` does. It is **not** a string index: `u32::MAX` in a required column (`id`, `kind`,
`source`, `label`, `record_id` is optional) is refused, and so is `u32::MAX` as an endpoint
row — or, in a `GMX1` batch, as an endpoint string index, which is required for the same
reason.

### Kinds

`kind` is a string-table entry, not a numeric tag: `NodeKind::from_name` and
`EdgeKind::from_name` resolve it. A name that resolves to nothing is refused. This is what
keeps the format from needing a mirror of the Rust enums' discriminants — adding a kind is a
change to one enum and nothing else.

### The dense-row rule

`edge source` and `edge target` are **node row numbers**, not ids. Row `r` of a node column
is the node whose dense index is `r`.

That only works if every row is kept. `index_model` de-duplicates node ids first-wins and
drops the rest; `index_columns` instead **refuses** a node or edge id that is already taken
(criterion 3 of `docs/decisions/ingest-columns.md`), so the row the caller wrote is the index
the motor holds, and an endpoint row names exactly that node. Two consequences a producer must
respect:

- A repeated id in two rows is an error, not a merge. There is no "first wins" here.
- Because rows are dense and in the order written, the encoder must emit every node in the
  document's own order.

### The batch format `GMX1`, and its endpoint rule

The **same sections, the same order, the same rules** — under a second magic word,
`0x31584D47` (`"GMX1"`, read little-endian as a `u32`). A `GMX1` document is one batch to
append to a live graph (`docs/contract/delta.md`); `gm_graph_extend_columns` reads it,
`graph_wasm::columns_batch` writes it.

One rule differs, and only one: in a `GMC1` document `edge source` and `edge target` are node
**rows**, and in a `GMX1` batch they are **required string indices naming node ids**. The
reason is the same in both directions: a batch's edge may point at a node the graph already
holds, and a dense row is an internal index that never crosses the wire — a batch written on
one machine would name a row that means nothing on another.

- An endpoint cell is a required string index: `u32::MAX` is refused (`u32::MAX` in a
  required column), and so is an index at or past `string_count` — both as the string-index
  refusals, **not** as the document's "endpoint row ≥ `node_count`".
- An index below `string_count` is a legal cell whatever its number. A node row number is not a
  special case, and is not a dense row: the motor resolves the name against the cumulative
  graph first and against the batch second, and refuses an endpoint that names neither.
- Every other refusal in the table below is the same for both magics, and the format is a magic
  word rather than a version or a flag: each reader refuses the other's bytes (`ColumnsInvalid`
  at the ABI), because a name that can never be reused cannot be read as a variant of the other
  document. `crates/graph-contract/src/ingest_columns/tests/batch.rs` pins both directions.

### Refusals

Every one of these is `Code::ColumnsInvalid` at the ABI, and each has its own test in
`crates/graph-contract/src/ingest_columns/tests.rs`:

| Refusal | Rule |
|---|---|
| short / long buffer | the declared sections do not sum to the buffer length |
| bad magic | `header[0]` is neither `0x31434D47` nor, for the batch reader, `0x31584D47` |
| bad version | `header[1] != 1` |
| nonzero reserved | `header[6]` or `header[7]` is not `0` |
| decreasing offset | `offsets[i + 1] < offsets[i]` |
| last offset ≠ blob length | `offsets[string_count] != blob_len` |
| invalid UTF-8 | `str::from_utf8` on the blob fails |
| split code point | `str::get` on an offset pair returns `None` |
| string index out of range | an index is not `< string_count`, and is not `u32::MAX` in a required column |
| `u32::MAX` in a required column | as above |
| endpoint row ≥ `node_count` | an edge names a row that is not a node (`GMC1` only — a `GMX1` endpoint is a string index, and the two rows above are the refusals there) |
| boolean `2` | a `has_note` / `directed` / `child_first` cell is neither `0` nor `1` |
| NaN, infinite | any of `weight`, `version`, `strength` |
| nonzero padding | a pad byte is not `0` |
| count of `u32::MAX` | any count at or above the `u32::MAX` string index space |

`−0.0` and a subnormal are **accepted**: they are ordinary finite `f64`s, and the JSON reader
accepts them too. Section sizes are computed in checked `u64` and every offset is checked
against the buffer before it is used, so no allocation is ever sized from a header field.

A `len` above `MAX_INGEST_BYTES` (`crates/graph-wasm/src/ingest.rs`, F-16 of
`docs/decisions/wasm-ingest-limits.md`) is refused with `Code::IngestTooLarge` **before** the
document is decoded.

## Encoder

`crates/graph-sdk-js/src/columns.ts` — `encodeColumns(doc)`, which writes into one
`Uint8Array` sized up front with `TextEncoder.encodeInto`. Strings that fail
`String.prototype.isWellFormed()` are refused there, by field name, before any byte is
written: a lone surrogate would otherwise become U+FFFD in the blob and the row would read
back as a different id.

`graph_wasm::columns_batch(nodes, edges)` writes the `GMX1` batch, in Rust, from the same
record types `encodeColumns`' caller has: one string table interned by content in first-seen
order, the buffer allocated once at the exact total the reader demands, and an endpoint that
names a node id **interned on demand** — a batch may point at a node the graph already holds,
which a whole document's encoder cannot express. It is the writer the tests, `graph-cli tick
--path columns` and the `force-gate` stream stage's columns arm all go through, so a bench row
and a gate digest are taken over one encoder's bytes.