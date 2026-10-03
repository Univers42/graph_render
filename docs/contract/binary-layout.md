# Binary layout — the graph-motor snapshot's byte contract

Status: **authoritative** (Phase 2; format **0.3** adds the notes section in Phase 3, user
decision D-N, `docs/decisions/snapshot-notes.md`). Scope: `graph-contract::binary`,
`crates/graph-contract/src/{binary.rs,binary/decode.rs,version.rs,geometry.rs,snapshot.rs,snapshot/error.rs,notes.rs}`.
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
| 14 | 1 | `dim` | `u8` | `0` 2D (no z column), `1` 3D (z column present); `2..=255` refused as `ReservedDim` |
| 15 | 1 | padding | `u8` | must be `0` |
| 16 | 4 | stage count | `u32` LE | reserved; only `1` is accepted |
| 20 | 4 | node count `n` | `u32` LE | length of every node column |
| 24 | 4 | edge count `m` | `u32` LE | length of every edge column |

Source of the table: `snapshot.rs:7-19` (`SnapshotHeader::encode`/`decode`,
`crates/graph-contract/src/snapshot.rs:132-167`). `HEADER_LEN = 28`.

**`dim` (format 0.4).** Byte 14 was a reserved z channel that had to be `0`; it is now the
snapshot's dimension, once for the whole payload (`snapshot/dim.rs`). `0` is 2D and carries
no z column, `1` is 3D and does. A reader **computes every column position from this byte**,
never from fixed offsets, so the `r`/`w`/`h` shift when `dim` is `1` (see the node columns
below). A reader that does not implement 3D refuses `dim = 1` at this byte, before either
geometry tag — the check order is magic, major, `dim`, padding, node tag, edge tag, stages
(`snapshot.rs:decode`). A `dim` of `2..=255` is refused as `ReservedDim` by every reader:
further dimensions are allocated to no one.

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
   When `dim = 1` a **`z` column** sits immediately after `y`, so the order is `x, y, z`
   (Point), `x, y, z, r` (Circle) or `x, y, z, w, h` (Box) — coordinates contiguous, sizes
   pushed one word along. For `dim = 0` the order is exactly the one above and no `z` column
   is written (`geometry.rs`, `NodeGeometry::columns_dim`).
6. **edge geometry**, shaped by the edge tag: nothing (Line), or offsets+points
   (Polyline), or a degree then offsets+points (Curve).
7. **notes** (format 0.3 and later only) — `k: u32`, then `code: u32 × k`, then
   `index: u32 × k`. The **last** section: nothing follows it.

Order and column names come from `Snapshot::to_bytes` (`binary.rs:170-195`) and
`decode` (`binary/decode.rs:12-41`), which write and read the same seven sections in the
same order — the seventh only when the snapshot's version carries notes (see below).

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

| node kind | columns, `dim = 0` | columns, `dim = 1` | per-element rule |
|---|---|---|---|
| Point (`0`) | `x`, `y` | `x`, `y`, `z` | finite |
| Circle (`1`) | `x`, `y`, `r` | `x`, `y`, `z`, `r` | finite; `r ≥ 0` |
| Box (`2`) | `x`, `y`, `w`, `h` | `x`, `y`, `z`, `w`, `h` | finite; `w ≥ 0`, `h ≥ 0` |

Each column is `n` little-endian `f32`s (`geometry.rs`, `NodeGeometry::columns_dim`). A `z`
is a coordinate, not a size, so it may be negative; only `r`, `w` and `h` are sizes. A `z`
column whose length is not `n` is refused as `Length { column: "node.z" }`. Edge paths stay
2D whatever `dim` is: a 3D edge path would be a second breaking change and is not in this
format.

Open gap: the axis orientation. This contract fixes the column order but not which way `y`
points. SciGraphs is y-up and flips `y` only on the way into pixels
(`text_overlay.py:231`); the studio's `worldToScreen` (`packages/graph-render/src/camera.ts`)
does not flip, so a larger `y` draws lower (`docs/reviews/review-studio.md` ST-6). No bundled
2D layout puts a signed quantity on `y` today, so the difference is a mirror image, not a wrong
shape. It is decided with the parity oracle, not in a fix job, because flipping moves every
pinned 2D screenshot.

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

### Notes (format 0.3)

What a stage repaired or approximated, recorded in the snapshot itself so a consumer can
tell an exact result from a degraded one (`notes.rs:1-24`). Columnar like the rest:
`k: u32`, then the `k` codes, then the `k` indices, all `u32` LE, so no padding.

| code | name | `index` |
|---:|---|---|
| 1 | `hierarchy.cycle_edge_dropped` | the dropped edge's position |
| 2 | `hierarchy.extra_parent_dropped` | the dropped edge's position |
| 3 | `packing.approximate` | `4294967295` (`u32::MAX`): snapshot-wide |
| 4 | reserved: `dag.dummy_budget_exceeded` (Phase 5) | refused as `Reserved` |
| 5 | reserved: `dag.edge_reversed` (Phase 5) | refused as `Reserved` |
| 6 | reserved: `post.route_fallback` (Phase 8) | refused as `Reserved` |

- An `index` is an **edge position** — an index into the edge id table and every edge
  column, `0..m` — or, for a snapshot-wide note, `u32::MAX`. Codes 1 and 2 need
  `index < m`; code 3 needs `index == u32::MAX` (`notes.rs:148-170`).
- **Closed set**, like the geometry tags: 1-3 are read; 4-6 are allocated and refused as
  `NoteCodeError::Reserved`; anything else is `NoteCodeError::Unknown`
  (`notes.rs:86-94`). Turning a reserved code on is a **minor bump**: a 0.3 reader
  refuses it.
- **Canonical**: strictly ascending by `(code, index)`. That pair is the whole note, so
  the order is total and a repeated note is refused (`NoteOrder`). Producers sort before
  building; the constructor checks and never sorts (graph-core's `layout::snapshot` sorts
  a stage's notes before it builds).

## The pinned 84-byte example (ground truth)

`binary/tests/pinned.rs:9-51`, `the_layout_is_pinned_byte_for_byte_for_a_tiny_snapshot`: 2
nodes (`"a"`, `"bc"`), 1 edge (`"e"`, `a → bc`), Point nodes, Line edges, format 0.3, no
notes. Annotated byte for byte:

| offset | bytes (hex) | len | field | value |
|---:|---|---:|---|---|
| 0 | `47 4D 53 4E` | 4 | magic | `"GMSN"` |
| 4 | `00 00 00 00` | 4 | version.major | 0 |
| 8 | `03 00 00 00` | 4 | version.minor | 3 |
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
| 80 | `00 00 00 00` | 4 | note.count (k) | 0 |

84 bytes total; edges are Line, so the edge geometry is empty and the notes section — just
`k = 0` — follows the node columns at offset 80.

The same test reuses this snapshot with `edges = Curve { degree: 2, paths }` (offsets
`[0, 1]`, one point `(1.0, 0.5)`): only byte 13 changes (`00` → `02`), everything through
offset 79 is identical, and a Curve tail, then `k = 0`, follows:

| offset (from 80) | bytes (hex) | len | field | value |
|---:|---|---:|---|---|
| 0 | `02 00 00 00` | 4 | edge.degree | 2 |
| 4 | `00 00 00 00` | 4 | edge.offsets[0] | 0 |
| 8 | `01 00 00 00` | 4 | edge.offsets[1] | 1 |
| 12 | `00 00 80 3F` | 4 | edge.pts[0] (x) | 1.0 |
| 16 | `00 00 00 3F` | 4 | edge.pts[1] (y) | 0.5 |
| 20 | `00 00 00 00` | 4 | note.count (k) | 0 |

### The pinned one-note example

`binary/tests/pinned.rs:53-66`, `a_snapshot_with_one_note_is_pinned_byte_for_byte`: the
84-byte snapshot above carrying one note, `packing.approximate` for the whole snapshot.
Bytes 0-79 are identical; the notes section becomes:

| offset | bytes (hex) | len | field | value |
|---:|---|---:|---|---|
| 80 | `01 00 00 00` | 4 | note.count (k) | 1 |
| 84 | `03 00 00 00` | 4 | note.code[0] | 3 = `packing.approximate` |
| 88 | `FF FF FF FF` | 4 | note.index[0] | 4294967295 (`u32::MAX`, snapshot-wide) |

92 bytes total. It reads back and writes the same 92 bytes.

### A 0.2 snapshot

The 84-byte example's first 80 bytes with byte 8 = `02` — no notes section at all — is a
valid 0.2 snapshot: a 0.3 reader reads it as `k = 0` and writes back the same 80 bytes
(`binary/tests/pinned.rs:68-85`). The first 80 bytes left labelled 0.3 are refused as
`Truncated { column: "note.count" }`, never read as `k = 0` (`pinned.rs:87-95`).

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
| `ReservedDim(v)` | the byte | 14 |
| `NonZeroPadding(v)` | the byte | 15 |
| `ReservedStageCount(n)` | the `u32` | 16..20 |

The table above is ordered by byte offset, not by check order: a header wrong in more
than one way reports whichever fault `decode` reaches first, which is `Truncated`,
`BadMagic`, `UnsupportedMajor`, the dim byte (14), the padding byte
(byte 15), the node tag (byte 12), the edge tag (byte 13), then `ReservedStageCount` —
so an unimplemented dim or nonzero padding is reported ahead of a bad geometry tag even
though the tag bytes sit earlier in the layout (`snapshot.rs`'s `decode`, pinned by
`dim_and_padding_are_checked_before_the_geometry_tag`). A 2D-only reader therefore
refuses a 3D snapshot at byte 14, ahead of any tag.

### Body (`SnapshotError`, `snapshot/error.rs:13-113`)

| variant | example columns | position reported |
|---|---|---|
| `Header(ReadError)` | — | wraps the table above |
| `Truncated { column }` | any, `note.count`/`note.code`/`note.index` included | decode only: the payload ends inside this column |
| `TrailingBytes { count }` | — | bytes left after the last column |
| `Length { column, expected, found }` | `edge.source`, `edge.target`, `node.x/y/z/r/w/h`, `edge.offsets`, `edge.pts`, `note.index` | construction only: wrong element count (`note.index`: not as long as `note.code`) |
| `Offsets { column, index }` | `node.id`, `edge.id`, `edge.offsets` | first offset that isn't 0, decreases, or overruns the data |
| `Utf8 { column, index }` | `node.id`, `edge.id` | the string whose bytes are not UTF-8 |
| `Padding { column }` | `node.id`, `edge.id` | a string table's padding byte is nonzero |
| `DuplicateId { column, index }` | `node.id`, `edge.id` | the later of the two equal ids |
| `Endpoint { column, index }` | `edge.source`, `edge.target` | the edge whose endpoint is `≥ n` |
| `NonFinite { column, index }` | `node.x/y/z/r/w/h`, `edge.pts` | first NaN/±∞ (D9, `geometry.rs`) |
| `Negative { column, index }` | `node.r`, `node.w`, `node.h` | first negative value |
| `CurveDegree` | — | a Curve's degree is 0 |
| `Capacity { column }` | `node.id`, `edge.id` | construction only: more ids/bytes/points than a `u32` counts |
| `NoteCode { index, error }` | `note.code` | the note whose code is `Reserved` (4-6) or `Unknown` |
| `NoteOrder { index }` | `note.*` | the note not strictly after the one before it by `(code, index)` — a repeat included |
| `NoteTarget { index }` | `note.index` | the note whose index its code does not allow (`≥ m` for 1-2, not `u32::MAX` for 3) |
| `NotesUnsupported { version }` | `notes` | construction only: a note on a snapshot labelled below 0.3 (0.0 included) |
| `DimUnnameable { version }` | `node.z` | construction only: a z column on a snapshot labelled below 0.4, a version that names no dimension |

`binary/tests.rs:184-246` pins one decode-time patch per variant above (e.g. writing a
NaN at byte `h+48` yields `NonFinite { column: "node.y", index: 1 }`) and confirms every
truncation length is refused as `Truncated`, and that a header claiming more nodes than
the payload holds is refused before any allocation for those nodes. `notes/tests.rs:183-234`
does the same for each note refusal the bytes can carry (`NoteCode` reserved and unknown,
`NoteOrder`, `NoteTarget`, a short or long section, trailing bytes), and
`notes/tests/json.rs` refuses each on the JSON face too. `Length { column: "note.index" }`
and `NotesUnsupported` cannot be written in bytes — one `k` sizes both columns, and a
snapshot below 0.3 has no section to read — so they are construction and JSON refusals.

## Version policy

`version.rs:1-14,63-72`. A reader **refuses** a major above the one it knows
(`check_readable`), naming both versions, rather than guess at a newer layout. A newer
minor of a known major is read. `CURRENT_VERSION` is **0.4**; a JSON document with no
`version` reads as `UNVERSIONED` (0.0), which a 0.4 reader also accepts.

**A snapshot is labelled with the lowest version that can express it**, and
`label_for` (`snapshot/dim.rs`) is the one function that says which. A `dim = 0` snapshot is
labelled **0.3** and a `dim = 1` one **0.4**, so raising `CURRENT_VERSION` to 0.4 moves no
2D byte at all — byte 8 is the version, and no 2D producer writes any other. Every producer
calls `label_for`, never `CURRENT_VERSION`: `graph-core/src/layout/mod.rs`,
`graph-cli`'s `snapshot_cmd/dag.rs` and `snapshot_cmd/exercise.rs`. This is the notes
precedent again — writing follows the snapshot's own version. A z column under a label that
names no dimension is refused (`DimUnnameable`), so a snapshot can never claim a dimension
its own version has no word for.

The notes section is dispatched on the **version**, never by sniffing for bytes at the
end (`carries_notes`, `notes.rs:173-178`): a 0.3 reader reads it from a snapshot labelled
0.3 or later and treats `k` as 0 below, then applies the trailing-bytes check as before.
So a **0.2 snapshot still reads**, as one with no notes, and a **0.2 reader refuses a 0.3
snapshot as `TrailingBytes`** — the section is bytes after what it knows as the last
column: refused, never misread. Writing follows the snapshot's own version: `to_bytes` and
`to_json` emit the section only from 0.3 on, and a snapshot labelled below 0.3 cannot hold
a note (`NotesUnsupported`). Turning on a reserved note code (4-6) is a **minor bump**,
because a 0.3 reader refuses those codes.

Under `0.x` the crate treats a minor bump as license for a breaking payload change — a 0.2
reader does not refuse a 0.1-labelled or unlabelled snapshot even if its shape moved. That
is acceptable **only** while nothing persists snapshots; once Phase 4's zero-copy transport
has consumed this format, 1.0 is to be declared and the rule above stops applying. (User
decision Q2, `docs/reports/HANDOFF.md`.)

## The JSON face

`canonical_json.rs:1-17`. Same information, different shape, for any third-party frontend.
`to_json`/`from_json` round-trip a `Snapshot` byte-exact through the binary face
(`graph-cli roundtrip`, `crates/graph-cli/src/snapshot_cmd.rs`; its sweep draws a 0.2-labelled
snapshot, a 0.3 one with `k = 0`, one with each implemented code, and a 3D one per three
seeds — a third of the exercise snapshots carry a `z` column, drawn by hand because no 3D
layout exists yet).

- **Shape**: `{"dim","edges":{"id","source","target"}, "geometry":{"edges","nodes"},
  "nodes":{"id"}, "notes":{"code","index"}, "version":{"major","minor"}}`. Edge endpoints
  are node **ids** (strings), never the dense node positions the binary face uses. The one
  position the JSON face carries is a note's `index`: an **edge position** (an index into
  `edges.id`), or the literal `4294967295` (`u32::MAX`) for a snapshot-wide note.
  `notes` is written from 0.3 on, **required** from 0.3 on and **optional** below (absent
  reads as no notes; `canonical_json/read.rs:205-217`); the schema marks it with a
  default and enumerates the codes `1, 2, 3`. Node/edge geometry objects carry a
  `"kind"` string (`Point`/`Circle`/`Box`, `Line`/`Polyline`/`Curve`) plus that kind's
  columns; the full shape is `docs/contract/snapshot-schema.json`.
- **`"dim"` (format 0.4)**: written from 0.4 on, which by the label rule above is exactly
  when the snapshot can be 3D, and **reads as `0` when absent** — the same optional-member
  rule as `notes`. `geometry.nodes` carries a `"z"` array iff `dim` is `1`; for `dim = 0` the
  object is unchanged, and a `z` array under `dim = 0` is refused rather than dropped. The
  schema marks `dim` with a default of `0` and enumerates `0, 1`. JSON Schema cannot tie a
  member's presence to another member's value, so the reader enforces the rule.
- **Canonical**: compact (no insignificant whitespace), object keys sorted by UTF-8 bytes
  at every depth — so the binary's fixed column order (`x, y, r` / `x, y, w, h`) is not the
  JSON key order (`kind, r, x, y` / `h, kind, w, x, y`) — arrays in snapshot/column order,
  one trailing newline. That key order is UTF-8 **byte** order and deliberately not
  JavaScript's UTF-16 code-unit order, which disagrees for a key mixing an astral character
  with one in U+E000..U+FFFF; the one trailing newline is likewise part of the text, so a
  comparison is over all of it, trailing byte included.
- **Numbers**: `f32` values are written with Rust's `f32` `Display`, the shortest decimal
  that reads back as the same `f32`, never an exponent; `u32` values as a plain decimal, no
  sign or fraction. A decimal that overflows `f32` range parses to ±∞, which construction
  then refuses (D9) rather than silently accepting it. `-0.0` is written `-0` and reads
  back as `-0.0`, so a negative zero crosses the JSON face with its sign bit, and the
  binary face's byte comparison is what decides that the two are different snapshots. The
  writer is canonical and the reader is deliberately lenient: any spelling of a number the
  grammar allows (`1E0`, `1.0`, `1e0`) reads as the same value and is rewritten in the
  canonical form, so a hand-written or third-party document need not spell a float the way
  the writer would.
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
never reaches the hash either. The notes section is part of those bytes, so two snapshots
that differ only in their notes hash differently
(`snapshot_cmd/tests.rs:73`, `notes/tests.rs:237`).

## Verification note

Every fact above was checked against the cited source, not recalled: the header table
against `SnapshotHeader::encode`/`decode`, the hex dump against
`the_layout_is_pinned_byte_for_byte_for_a_tiny_snapshot`, the refusal table against
`SnapshotError`/`ReadError` and the decode-time patch test, the version policy against
`check_readable`, and the hash against `snapshot_cmd.rs`/`runner.rs`. No disagreement was
found between the code and `docs/reports/HANDOFF.md` item 2's checklist for this document;
where the handoff is silent (the reserved Ribbon/Arc tags, the dim byte/stage-count
reservation, `Capacity`), this document adds the detail the code carries. The format 0.3
update (notes) was checked the same way, against `notes.rs`, `binary.rs`/`decode.rs`, the
pinned tests in `binary/tests/pinned.rs` and the refusal tests in `notes/tests.rs`. The
format 0.4 update (`dim` and the z column) was checked against `snapshot/dim.rs`,
`geometry.rs` and `canonical_json.rs`/`read.rs`, and 2D byte-identity was measured rather
than argued: every 2D layout's binary face, seeds 0-7, digests identically before and after
the change (`docs/decisions/contract-3d-verdict.md` condition 2), and the pinned 0.3 example
in `binary/tests/pinned.rs` stays green unedited, `expected[8] == 3` included.
