# Binary layout — the graph-motor snapshot's byte contract

Status: **authoritative** (Phase 2). Scope: `graph-contract::binary`,
`crates/graph-contract/src/{binary.rs,binary/decode.rs,version.rs,geometry.rs,snapshot.rs}`.
`binary.rs`'s own module doc points back here: this file is normative, the code follows it.

Columnar, little-endian, word-aligned throughout: every section's length is a multiple of
4 bytes, so every column starts on a 4-byte boundary and a reader can view it in place with
no copy. Every integer on the wire is `u32` or a single tag byte, never `usize` (D6,
`snapshot.rs:20`). A [`Snapshot`] can only be constructed valid, so `to_bytes()` cannot
produce a byte the rules below forbid; `from_bytes()` refuses exactly what construction
refuses, plus truncation and trailing bytes.

## The 28-byte header

| offset | size | field | type | notes |
|-------:|-----:|-------|------|-------|
| 0 | 4 | magic | bytes | `b"GMSN"` (`4D 53 4E` read as `47 4D 53 4E`), else `BadMagic` |
| 4 | 4 | format major | `u32` LE | refused if greater than this reader's known major |
| 8 | 4 | format minor | `u32` LE | any minor of a known major is read |
| 12 | 1 | node geometry tag | tag byte | `NodeGeometryKind`: `0` Point, `1` Circle, `2` Box |
| 13 | 1 | edge geometry tag | tag byte | `EdgeGeometryKind`: `0` Line, `1` Polyline, `2` Curve, `3`/`4` reserved |
| 14 | 1 | z channel | `u8` | `0` = absent; any nonzero is refused (reserved, not implemented) |
| 15 | 1 | padding | `u8` | must be `0` |
| 16 | 4 | stage count | `u32` LE | reserved; only `1` is accepted |
| 20 | 4 | node count `n` | `u32` LE | length of every node column |
| 24 | 4 | edge count `m` | `u32` LE | length of every edge column |

Source of the table: `snapshot.rs:7-19` (`SnapshotHeader::encode`/`decode`,
`crates/graph-contract/src/snapshot.rs:130-167`). `HEADER_LEN = 28`.

**Reserved geometry tags.** `geometry.rs:45-48` allocates edge tag `3` to Sankey ribbons
and `4` to chord arcs: recognised (so a future reader knows they exist) but refused by
this one (`TagError::Reserved`); a tag with no allocation at all (`5..`) is
`TagError::Unknown`. No node tag is reserved yet — any node tag past `2` is `Unknown`,
not `Reserved`, since nothing has claimed it (`geometry.rs:74-82`).

## Payload, in wire order

After the header, for a snapshot with `n` nodes and `m` edges:

1. **node id table** — CSR string table, `n` strings.
2. **edge id table** — CSR string table, `m` strings.
3. **edge.source** — `u32 × m`, each a position in the node id table.
4. **edge.target** — `u32 × m`, same.
5. **node columns**, in this fixed order: `x`, `y`, then `r` (Circle) or `w`, `h` (Box).
   Point has no third column. Each column is `f32 × n`.
6. **edge geometry**, shaped by the edge tag: nothing (Line), or offsets+points
   (Polyline), or a degree then offsets+points (Curve).

Order and column names come from `Snapshot::to_bytes` (`binary.rs:165-185`) and
`Reader::decode` (`binary/decode.rs:11-34`), which read the same six sections in the same
order.

### String tables (node id / edge id) — CSR-shaped

- `count + 1` offsets, `u32` LE, `offsets[0] == 0`, never decreasing, `offsets[count]` is
  the byte length of the text that follows.
- The strings' UTF-8 bytes back to back, string `i` at `bytes[offsets[i]..offsets[i+1]]`.
  Empty strings are kept (`offsets[i] == offsets[i+1]`).
- Zero bytes padding the byte run up to the next 4-byte boundary:
  `padding(len) = (4 - len % 4) % 4` (`binary.rs:195-197`), so `0..3` bytes, all `0`.

Every id in a table is unique (checked at construction, not on the wire itself).

### Adjacency

`edge.source[e]` and `edge.target[e]` are positions in the node id table (`0..n`), not
byte offsets and not ids. An edge id table entry pairs positionally with `source`/`target`
at the same index `e`.

### Node geometry columns

| node kind | columns, in order | per-element rule |
|---|---|---|
| Point (`0`) | `x`, `y` | finite |
| Circle (`1`) | `x`, `y`, `r` | finite; `r ≥ 0` |
| Box (`2`) | `x`, `y`, `w`, `h` | finite; `w ≥ 0`, `h ≥ 0` |

Each column is `n` little-endian `f32`s (`geometry.rs:174-181`, `NodeGeometry::columns`).

### Edge geometry, by edge tag

| edge kind | wire bytes |
|---|---|
| Line (`0`) | none — endpoints come from node geometry, zero bytes per edge |
| Polyline (`1`) | `offsets: u32 × (m + 1)`, then `pts: f32 × 2·offsets[m]` |
| Curve (`2`) | `degree: u32 × 1`, then `offsets: u32 × (m + 1)`, then `pts: f32 × 2·offsets[m]` |

`offsets[0] == 0`, never decreasing; edge `e` owns points
`offsets[e]..offsets[e+1]`; point `p` is `(pts[2p], pts[2p+1])` (`geometry.rs:153-162`).
A curve's `degree` is one value for the whole snapshot (2 quadratic, 3 cubic, …) and must
be at least 1. `u32`/`f32` arrays never need padding: every element is already 4 bytes.

## The pinned 80-byte example (ground truth)

`binary/tests.rs:75-111`, `the_layout_is_pinned_byte_for_byte_for_a_tiny_snapshot`: 2 nodes
(`"a"`, `"bc"`), 1 edge (`"e"`, `a → bc`), Point nodes, Line edges. Annotated byte for byte:

| offset | bytes (hex) | len | field | value |
|---:|---|---:|---|---|
| 0 | `47 4D 53 4E` | 4 | magic | `"GMSN"` |
| 4 | `00 00 00 00` | 4 | version.major | 0 |
| 8 | `02 00 00 00` | 4 | version.minor | 2 |
| 12 | `00` | 1 | node_kind tag | 0 = Point |
| 13 | `00` | 1 | edge_kind tag | 0 = Line |
| 14 | `00` | 1 | z channel | 0 |
| 15 | `00` | 1 | header padding | 0 |
| 16 | `01 00 00 00` | 4 | stage_count | 1 |
| 20 | `02 00 00 00` | 4 | node_count (n) | 2 |
| 24 | `01 00 00 00` | 4 | edge_count (m) | 1 |
| 28 | `00 00 00 00` | 4 | node.id offsets[0] | 0 |
| 32 | `01 00 00 00` | 4 | node.id offsets[1] | 1 |
| 36 | `03 00 00 00` | 4 | node.id offsets[2] | 3 |
| 40 | `61 62 63` | 3 | node.id bytes | `"abc"` (`"a"`, `"bc"`) |
| 43 | `00` | 1 | node.id padding | `padding(3) = 1` |
| 44 | `00 00 00 00` | 4 | edge.id offsets[0] | 0 |
| 48 | `01 00 00 00` | 4 | edge.id offsets[1] | 1 |
| 52 | `65` | 1 | edge.id bytes | `"e"` |
| 53 | `00 00 00` | 3 | edge.id padding | `padding(1) = 3` |
| 56 | `00 00 00 00` | 4 | edge.source[0] | 0 (node `"a"`) |
| 60 | `01 00 00 00` | 4 | edge.target[0] | 1 (node `"bc"`) |
| 64 | `00 00 80 3F` | 4 | node.x[0] | 1.0 |
| 68 | `00 00 20 C0` | 4 | node.x[1] | −2.5 |
| 72 | `00 00 00 00` | 4 | node.y[0] | 0.0 |
| 76 | `00 00 00 3F` | 4 | node.y[1] | 0.5 |

80 bytes total; edges are Line, so nothing follows offset 80.

The same test reuses this snapshot with `edges = Curve { degree: 2, paths }` (offsets
`[0, 1]`, one point `(1.0, 0.5)`): only byte 13 changes (`00` → `02`), everything through
offset 79 is identical, and a Curve tail follows:

| offset (from 80) | bytes (hex) | len | field | value |
|---:|---|---:|---|---|
| 0 | `02 00 00 00` | 4 | edge.degree | 2 |
| 4 | `00 00 00 00` | 4 | edge.offsets[0] | 0 |
| 8 | `01 00 00 00` | 4 | edge.offsets[1] | 1 |
| 12 | `00 00 80 3F` | 4 | edge.pts[0] (x) | 1.0 |
| 16 | `00 00 00 3F` | 4 | edge.pts[1] (y) | 0.5 |

## Decode-time refusals

Every refusal names the offending column and, where the fault has one, a position; the
position is a logical index into that column's elements, not a byte offset — except for
the fixed header, whose faults report the exact byte offset above.

### Header (`ReadError`, `snapshot.rs:92-112`)

| variant | reports | offset |
|---|---|---|
| `Truncated { needed, found }` | byte counts | fewer than 28 bytes present |
| `BadMagic` | — | 0..4 |
| `UnsupportedMajor(NewerMajor)` | found vs. known version | 4..8 |
| `Geometry(TagError::Unknown\|Reserved)` | the tag byte | 12 (node) or 13 (edge) |
| `ReservedZChannel(v)` | the byte | 14 |
| `NonZeroPadding(v)` | the byte | 15 |
| `ReservedStageCount(n)` | the `u32` | 16..20 |

### Body (`SnapshotError`, `snapshot.rs:188-264`)

| variant | example columns | position reported |
|---|---|---|
| `Header(ReadError)` | — | wraps the table above |
| `Truncated { column }` | any | decode only: the payload ends inside this column |
| `TrailingBytes { count }` | — | bytes left after the last column |
| `Length { column, expected, found }` | `edge.source`, `edge.target`, `node.x`, `edge.offsets`, `edge.pts` | construction only: wrong element count |
| `Offsets { column, index }` | `node.id`, `edge.id`, `edge.offsets` | first offset that isn't 0, decreases, or overruns the data |
| `Utf8 { column, index }` | `node.id`, `edge.id` | the string whose bytes are not UTF-8 |
| `Padding { column }` | `node.id`, `edge.id` | a string table's padding byte is nonzero |
| `DuplicateId { column, index }` | `node.id`, `edge.id` | the later of the two equal ids |
| `Endpoint { column, index }` | `edge.source`, `edge.target` | the edge whose endpoint is `≥ n` |
| `NonFinite { column, index }` | `node.x/y/r/w/h`, `edge.pts` | first NaN/±∞ (D9, `geometry.rs:273-282`) |
| `Negative { column, index }` | `node.r`, `node.w`, `node.h` | first negative value |
| `CurveDegree` | — | a Curve's degree is 0 |
| `Capacity { column }` | `node.id`, `edge.id` | construction only: more ids/bytes/points than a `u32` counts |

`binary/tests.rs:224-277` pins one decode-time patch per variant above (e.g. flipping
byte `h+48` to NaN yields `NonFinite { column: "node.y", index: 1 }`) and confirms every
truncation length is refused as `Truncated`, and that a header claiming more nodes than
the payload holds is refused before any allocation for those nodes.

## Version policy

`version.rs:1-11,60-69`. A reader **refuses** a major above the one it knows
(`check_readable`), naming both versions, rather than guess at a newer layout. A newer
minor of a known major is read. `CURRENT_VERSION` is **0.2**; a JSON document with no
`version` reads as `UNVERSIONED` (0.0), which a 0.2 reader also accepts.

Under `0.x` the crate treats a minor bump as license for a breaking payload change — a 0.2
reader does not refuse a 0.1-labelled or unlabelled snapshot even if its shape moved. That
is acceptable **only** while nothing persists snapshots; once Phase 4's zero-copy transport
has consumed this format, 1.0 is to be declared and the rule above stops applying. (User
decision Q2, `docs/reports/HANDOFF.md`.)

## The JSON face

`canonical_json.rs:1-13`. Same information, different shape, for any third-party frontend.
`to_json`/`from_json` round-trip a `Snapshot` byte-exact through the binary face
(`graph-cli roundtrip`, `crates/graph-cli/src/snapshot_cmd.rs:189-208`).

- **Shape**: `{"edges":{"id","source","target"}, "geometry":{"edges","nodes"},
  "nodes":{"id"}, "version":{"major","minor"}}`. Edge endpoints are node **ids** (strings),
  never the dense positions the binary face uses. Node/edge geometry objects carry a
  `"kind"` string (`Point`/`Circle`/`Box`, `Line`/`Polyline`/`Curve`) plus that kind's
  columns; the full shape is `docs/contract/snapshot-schema.json`.
- **Canonical**: compact (no insignificant whitespace), object keys sorted by UTF-8 bytes
  at every depth — so the binary's fixed column order (`x, y, r` / `x, y, w, h`) is not the
  JSON key order (`kind, r, x, y` / `h, kind, w, x, y`) — arrays in snapshot/column order,
  one trailing newline.
- **Numbers**: `f32` values are written with Rust's `f32` `Display`, the shortest decimal
  that reads back as the same `f32`, never an exponent; `u32` values as a plain decimal, no
  sign or fraction. A decimal that overflows `f32` range parses to ±∞, which construction
  then refuses (D9) rather than silently accepting it.
- **Strictness**: the reader (`canonical_json/parse.rs`) is a dependency-free RFC 8259
  parser that refuses a key repeated in one object, an unpaired UTF-16 surrogate, and
  nesting past 32; `canonical_json/read.rs` refuses any member the shape does not name.

## The hash

SHA-256, computed over the **binary face's bytes** (`Snapshot::to_bytes()`), not the JSON
text. `crates/graph-cli/src/snapshot_cmd.rs:96-104` writes the binary face then reports
`sha256_hex(&bytes)` over exactly those bytes; `crates/graph-cli/src/runner.rs:14-16`
implements `sha256_hex` with the `sha2` crate's `Sha256::digest`. Because a `Snapshot` can
only be constructed valid, this hash is always taken over a payload with no NaN/±∞ (D9)
and no `usize` on the wire (D6) — a refused snapshot never reaches `to_bytes()`, so it
never reaches the hash either.

## Verification note

Every fact above was checked against the cited source, not recalled: the header table
against `SnapshotHeader::encode`/`decode`, the hex dump against
`the_layout_is_pinned_byte_for_byte_for_a_tiny_snapshot`, the refusal table against
`SnapshotError`/`ReadError` and the decode-time patch test, the version policy against
`check_readable`, and the hash against `snapshot_cmd.rs`/`runner.rs`. No disagreement was
found between the code and `docs/reports/HANDOFF.md` item 2's checklist for this document;
where the handoff is silent (the reserved Ribbon/Arc tags, the z-channel/stage-count
reservation, `Capacity`), this document adds the detail the code carries.
