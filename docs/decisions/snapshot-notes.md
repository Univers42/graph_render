# D-N — the snapshot `notes` section, contract 0.2 → 0.3

Status: **decided** (Phase 3, user decision D-N). Scope: `graph-contract` (`notes.rs`,
`binary.rs`, `binary/decode.rs`, `canonical_json`, `schema`, `version.rs`), the
generated `docs/contract/snapshot-schema.json`, and `docs/contract/binary-layout.md`,
which is authoritative for the bytes. On the graph-core side: `layout::Geometry::notes`.

## Context

Some Phase 3 stages degrade on purpose. The hierarchy repair drops edges
(`docs/decisions/hierarchy-repair.md`), and circle packing can fall back to an
approximation. The phase prompt asks that such a choice be made "visible in the output
so a consumer knows it happened". A log is not part of the output, and format 0.2 had
no place to put such a record.

## The user's decision, as given

> A new, closed, versioned section every stage can append to, so degradations are
> visible IN the snapshot (binary + JSON), never only in a log. Shape (columnar, like
> everything else): notes: k u32, then code: u32×k, then index: u32×k — appended as the
> LAST section of the binary face, after edge geometry. JSON face: "notes": {"code":
> [...], "index": [...]} (canonical, sorted keys like the rest). Order: ascending (code,
> index) — total, deterministic.
>
> Allocated codes: 1 hierarchy.cycle_edge_dropped (index = edge index), 2
> hierarchy.extra_parent_dropped (index = edge index), 3 packing.approximate (index =
> u32::MAX, snapshot-wide), 4 (reserved) dag.dummy_budget_exceeded — Phase 5, 5
> (reserved) dag.edge_reversed — Phase 5, 6 (reserved) post.route_fallback — Phase 8.
> Unknown code on read → refused (closed set), like the geometry tags. Version becomes
> 0.3; a 0.2 snapshot (no notes section) must still decode as k = 0.

## Decision, as implemented

- **Where it lives.** A new module, `graph_contract::notes`, holds the notes and their
  tests. `SnapshotParts::notes` holds the two columns. A producer states each note as a
  `Note { code: NoteCode, index }`. The reserved codes 4-6 are not `NoteCode` variants,
  so no writer can emit one.
- **One check, both faces.** `Snapshot::new` enforces the rules once, so the binary and
  JSON readers refuse exactly the same things. It checks that:
  - the two columns have the same length;
  - every code is 1-3 (4-6 are refused as `Reserved`, anything else as `Unknown`);
  - codes 1 and 2 have `index < m`, and code 3 has `index == u32::MAX`;
  - the notes are strictly ascending by `(code, index)`, so a repeated note is refused.

  The constructor never sorts. The one producer path, graph-core's `layout::snapshot`,
  sorts a stage's notes (a stable sort on the whole note) before building.
- **Version dispatch, never EOF sniffing.** The decoder reads the section only when the
  version is `major == 0 && minor >= 3` (`carries_notes`). Below that it treats `k` as 0
  and then applies the usual trailing-bytes check. As a result:
  - a 0.2 snapshot reads as having no notes and writes back its own bytes;
  - a 0.3 snapshot that is missing its `k` word is refused as `Truncated`;
  - a 0.2 reader refuses a 0.3 snapshot as `TrailingBytes`: refused, never misread.
- **Writing follows the snapshot's version.** `to_bytes` and `to_json` emit the section
  only from 0.3 on. A snapshot labelled below 0.3, including an unversioned (0.0) JSON
  document, cannot hold a note (`NotesUnsupported`). On the JSON face, `notes` is
  required from 0.3 on and optional below (absent means no notes). The schema gives it a
  default and enumerates the codes `1, 2, 3`.
- **The JSON face carries one position.** `note.index` is an **edge position**, an index
  into `edges.id`. The snapshot-wide index is written as the literal `4294967295`.
- **Hashed.** The section is part of the binary face, so two snapshots that differ only
  in their notes have different bytes and a different SHA-256.
- **Reserved codes.** Turning on code 4, 5 or 6 in a later phase is a contract **minor
  bump**, because every 0.3 reader refuses those codes.

## Alternatives considered

- **Write the notes to a log or stderr.** A consumer of the snapshot would never see
  them. The decision rules this out.
- **A per-edge flag column** (for example `edge.dropped: u8 × m`). It costs m bytes on
  every snapshot and cannot express a snapshot-wide note such as code 3. Each new kind of
  note would be another column and another layout change. The notes section costs 4
  bytes when it is empty.
- **Free-text notes.** They cannot be validated, compared or kept closed. Codes can.
- **An open code set**, so that a later phase could add codes without a version bump.
  An old reader would then pass along a note it cannot interpret, silently. The closed
  set mirrors the geometry tags, where a reserved tag is refused.
- **Detect the section from the bytes left over.** Trailing garbage would make this
  ambiguous. Dispatching on the version is exact.
- **Sort in the constructor.** Two different inputs would build the same snapshot, and a
  producer bug would be hidden. The constructor refuses instead.
- **Keep the old format number.** A 0.2 reader would then misread the section. Bumping
  the minor lets that reader refuse a 0.3 snapshot outright (`TrailingBytes`). A minor
  bump is still allowed to change the payload while the format is `0.x`, per user
  decision Q2 in `docs/contract/binary-layout.md` § Version policy.

## Why

The section is columnar and word-aligned like every other section, and it is the last
one, so the bytes of every earlier section are unchanged. It is closed and canonical, so
one set of notes has one encoding and one hash. It is versioned, so older snapshots
still read and older readers refuse newer snapshots instead of misreading them.

## Pinned by

- `crates/graph-contract/src/binary/tests/pinned.rs` covers:
  - the 84-byte example;
  - the 92-byte one-note example;
  - a 0.2 snapshot reading back as `k = 0` with its own 80 bytes;
  - `Truncated { column: "note.count" }`.

  These are the byte tables in `binary-layout.md`.
- `crates/graph-contract/src/notes/tests.rs` and `notes/tests/json.rs` have one
  decode-patch test and one JSON test for each refusal.
- `graph-cli roundtrip` draws a 0.2-labelled snapshot, a 0.3 one with `k = 0`, and one
  with each implemented code across its seeds. It fails unless it draws every case, and
  each case must be byte-exact binary → JSON → binary.
