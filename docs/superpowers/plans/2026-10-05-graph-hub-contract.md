# graph-hub slice 1 (hub-contract) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** graph-contract gains a pure `hub` module (wire types, strict readers, canonical writers,
write-time cell checks, the in-memory materialization model, codegen) and `ingest/write.rs` gains the
stream pieces, with `to_json`'s bytes unchanged on every fixture.

**Architecture:** Everything is pure Rust inside `crates/graph-contract` (std, no I/O, no clock). The
hub readers reuse the ingest readers (`ingest/read.rs`, `validate.rs`), widened to `pub(crate)`. The
model is two `BTreeMap`s, so byte order falls out of the key order. Codegen moves out of `lib.rs` into
`src/codegen.rs` and grows two outputs.

**Tech Stack:** Rust 2021, `canonical_json` (in crate), schemars 1.x and serde (feature `codegen`
only), cargo test. Toolchain only through `scripts/orch/gr`.

**Spec:** `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` (revision 5.1; §3 ids,
§4 wire, §5.3 order, §6 doc_bytes, §8 rows, §10 slice 1). Verdict: `docs/decisions/graph-hub.md`.

## Global Constraints

- Workspace and plugin ids `[a-z0-9][a-z0-9-]{0,62}`; the workspace id is the ingest `source`.
- Collection ids `[A-Za-z0-9_-]{1,64}`; qualified `<plugin>.<collection>`; node id `<ws>:<plugin>.<collection>:<record>`.
- Record ids: non-empty, no `:`.
- MAX_BODY 4 MiB, MAX_BATCH 10 000, MAX_RECORD_BYTES 1 MiB. Manifest caps: 64 collections, 256 fields, 256 KiB; 64 plugins per workspace. Over a cap → 413.
- seq and rev stop at 2^53−1; a cursor is `<epoch>.<seq>`, each half < 2^53; a bare seq → 400.
- `max_change = MAX_BODY + 96 × MAX_BATCH` → 413.
- `\u0000` anywhere in a body (key or string) → 422.
- Manifests only grow: a changed or removed collection or field → 409; same version, other content → 409; same version, same content → no-op.
- A batch is atomic; a record at most once across upserts and deletes, else 422; an undeclared collection or field → 422; a tag containing `:` → 422; a qualified `B.coll` in a batch → 422.
- Document order: head, collections by qualified id, records by `(qcoll, id)` in byte order, tail. A record with nothing to prune is copied verbatim.
- `doc_bytes` = frame + Σ unpruned pieces: an exact upper bound of `to_json().len()`.
- graph-contract stays buildable for `wasm32-unknown-unknown` through graph-core (`default-features = false`): no `std::env` outside `#[cfg(feature = "negctl")]`, no `HashMap`, no clock, no `usize` on the wire.
- House limits: ≤ 40 lines per function, ≤ 4 parameters, ≤ 300 lines per file; `missing_docs` on every pub item; clippy `-D warnings`; every heuristic carries `Caveat:`; no new dependency (`Cargo.lock` unchanged).
- Do not touch `server/`, `crates/graph-core`, `crates/graph-wasm`, `crates/graph-sdk-js`, `docs/contract/delta.md`, `docs/contract/service-api.md`, `crates/graph-contract/src/ingest_columns.rs`.

## Decisions recorded here (spec silent or illustrative)

1. Change and notice JSON keys are written in byte order like every canonical text; the key order in the spec's `/changes` example is illustrative.
2. Error code strings are not in this slice: `HubError::status()` gives the HTTP class; hub-api adds codes.
3. A path-id grammar error is the server's 400; the contract reports `Grammar` (422) and the server maps.
4. The manifest `name` may change between versions; collections and fields may only be added.
5. A link target in a manifest is `coll` (same plugin) or `plugin.coll`; it is stored qualified.
6. Null is accepted for every role's cell; an originally empty list is kept, a list emptied by pruning is removed.

## Review Focus

1. A 5-byte `1e300` in a batch is written as a 301-digit integer, so a body under MAX_BODY can exceed `max_change` (Task 5 pins it; the finding goes to hub-report).
2. Record keys `("a", …)` vs `("a-b", …)`: tuple order, not string concatenation order (Task 6).
3. Collection order `"a-b.c" < "a.x"` (byte order of the qualified string, `-` 0x2D < `.` 0x2E) (Task 6).
4. NUL inside a map key, not only inside a string value (Tasks 2 and 4).
5. `-0` and 2^53−1 round-trip byte-identical through batch → change (Tasks 4 and 5).
6. Re-create after delete restarts rev at 1 (Task 6).
7. A link to an unregistered plugin's collection is dropped from the declaration and its cells pruned (Task 6).
8. A tag `:` inside a List cell vs a bare Text tags cell (Task 4).

---

### Task 1: Stream pieces in `ingest/write.rs`

**Files:**
- Create: `crates/graph-contract/src/ingest/write/pieces.rs`, `crates/graph-contract/src/ingest/tests/pieces.rs`
- Modify: `crates/graph-contract/src/ingest/write.rs` (`to_json` L23; delete private `collections` L59 and `records` L118), `crates/graph-contract/src/ingest.rs:86`, `crates/graph-contract/src/ingest/tests.rs` (add `mod pieces;`)

**Interfaces:**
- Produces (re-exported from `ingest`): `pub const DOC_HEAD: &str`, `pub const DOC_MIDDLE: &str`, `pub const DOC_SEPARATOR: &str`, `pub fn doc_tail(source: &str) -> String`, `pub fn collection_piece(c: &Collection) -> String`, `pub fn record_piece(r: &Record) -> String`, `pub fn frame_bytes(source: &str, collections: u64, records: u64) -> u64`.

- [ ] **Step 1: Byte-pin test on the old code.** In `tests/pieces.rs`, an FNV-1a 64 over `to_json(&read(ingest_member(F)))` plus its length, for EXPECTED_GRAPH, ROWS, NOTION (from `tests/fixtures.rs`), the writer test's MINIMAL, and a document with no collections and no records. Write the constants as `0` first.

```rust
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}
const PINS: [(&str, u64, usize); 5] = [("expected-graph", 0, 0), ("rows", 0, 0), ("notion", 0, 0), ("minimal", 0, 0), ("empty", 0, 0)];
#[test]
fn to_json_bytes_are_pinned() { /* for each fixture assert_eq!((fnv1a(t.as_bytes()), t.len()), (pin.1, pin.2), "{}", pin.0) */ }
```

- [ ] **Step 2: Bless.** Run `scripts/orch/gr cargo test -p graph-contract to_json_bytes_are_pinned` on the UNCHANGED writer; paste each reported `(hash, len)` into `PINS`; rerun: PASS. Commit (`updated`).
- [ ] **Step 3: Failing tests for the pieces.** For each fixture: `DOC_HEAD + join(collection_piece) + DOC_MIDDLE + join(record_piece) + doc_tail(source) == to_json(doc)`, and `frame_bytes(source, nc, nr) + Σ piece lengths == to_json(doc).len() as u64`. Run: FAIL (names not defined).
- [ ] **Step 4: Implement `write/pieces.rs`.**

```rust
//! The canonical document as pieces, so a store can stream it without building the `Ingest`.
use super::super::{Collection, Record, VERSION};
use super::{collection, quoted, record};
/// The bytes before the first collection.
pub const DOC_HEAD: &str = "{\"collections\":[";
/// The bytes between the last collection and the first record.
pub const DOC_MIDDLE: &str = "],\"records\":[";
/// The bytes between two collections or two records.
pub const DOC_SEPARATOR: &str = ",";
/// The bytes after the last record, trailing newline included.
pub fn doc_tail(source: &str) -> String { format!("],\"source\":{},\"version\":{VERSION}}}\n", quoted(source)) }
/// One collection's canonical text.
pub fn collection_piece(c: &Collection) -> String { collection(c) }
/// One record's canonical text.
pub fn record_piece(r: &Record) -> String { record(r) }
/// The document's bytes that are not a piece: head, middle, tail and separators.
pub fn frame_bytes(source: &str, collections: u64, records: u64) -> u64 {
    let separators = collections.saturating_sub(1) + records.saturating_sub(1);
    (DOC_HEAD.len() + DOC_MIDDLE.len() + doc_tail(source).len()) as u64 + separators
}
```

Rewrite `to_json` (keep its version assert) as the concatenation above; delete `collections` and `records`; `mod pieces; pub use pieces::*;` in write.rs; extend the `pub use write::{…}` line in `ingest.rs`.
- [ ] **Step 5:** `scripts/orch/gr cargo test -p graph-contract ingest::` → every pin and every existing writer test PASS (bytes unchanged). Commit.

### Task 2: hub skeleton: limits, ids, cursor, errors, breaks, strict parse

**Files:**
- Create: `crates/graph-contract/src/hub.rs`, `hub/{ids,error,breaks,strict}.rs`, `hub/tests.rs`, `hub/tests/wire.rs`
- Modify: `crates/graph-contract/src/lib.rs` (`pub mod hub;`), `crates/graph-contract/Cargo.toml` (`negctl = []` under `[features]`), `ingest/read.rs` (`IngestError::from_json` → `pub(crate)`), `ingest.rs` (`pub(crate) mod read; pub(crate) mod validate; pub(crate) mod write;`)

**Interfaces:**
- Produces: `hub::{VERSION: u32 = 1, MAX_COLLECTIONS = 64, MAX_FIELDS = 256, MAX_MANIFEST_BYTES = 262_144, MAX_PLUGINS = 64}` (u64 except VERSION); `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Limits { pub max_body: u64, pub max_batch: u64, pub max_record_bytes: u64 }` with `pub const DEFAULT: Limits = Limits { max_body: 4 << 20, max_batch: 10_000, max_record_bytes: 1 << 20 }`.
- `hub::ids`: `pub const MAX_SEQ: u64 = (1 << 53) - 1`; `check_workspace_id`, `check_plugin_id`, `check_collection_id`, `check_record_id`, each `(&str) -> Result<(), HubError>`; `pub fn qualify(plugin: &str, collection: &str) -> String`; `pub struct Cursor { pub epoch: u64, pub seq: u64 }` with `Cursor::parse(&str) -> Result<Cursor, HubError>` and `Display` (`"{epoch}.{seq}"`).
- `hub::error::HubError` (re-exported as `hub::HubError`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubError {
    Shape(crate::ingest::IngestError),
    Invalid { path: String, what: String },
    Grammar { coordinate: &'static str, value: String },
    Nul { path: String },
    TooLarge { what: &'static str, limit: u64 },
    Conflict { what: String },
    Cursor { text: String },
}
```
  `pub fn status(&self) -> u16`: Shape/Invalid/Grammar/Nul 422, TooLarge 413, Conflict 409, Cursor 400; `Display` names path and reason.
- `hub::breaks::on(name: &str) -> bool`: copy of `server/graph-server/src/breaks.rs`, reading `GM_HUB_BREAK`; the non-negctl `on` is `const fn` returning false.
- `hub::strict`: `pub(crate) fn parse_strict(text: &str, max: u64, what: &'static str) -> Result<canonical_json::Value, HubError>`; `pub(crate) fn child(path: &str, key: &str) -> String` (`key` when `path` is empty, else `path.key`).

- [ ] **Step 1: Failing tests in `hub/tests/wire.rs`:** each id checker accepts its grammar's edges (`"a"`, 63-char slug, 64-char collection) and refuses `""`, `"-a"`, `"A"` (slug), a 64-char slug, `"B.coll"` (collection), `"a:b"` (record); `Cursor::parse("3.118")` = `{3,118}`; refuses `"118"`, `"03.1"`, `"1.9007199254740992"`, `"1.2.3"`, `"1."`, `"+1.2"`; `Cursor::to_string()` round-trips; `parse_strict` refuses `"{\"a\":\"x\\u0000\"}"` and `"{\"k\\u0000\":1}"` with `Nul` (path `a` and `` respectively), refuses a text one byte over `max` with `TooLarge`, and maps a duplicate key to `Shape`; every `status()` value.
- [ ] **Step 2:** `scripts/orch/gr cargo test -p graph-contract hub::` → FAIL (module missing).
- [ ] **Step 3: Implement.** `parse_strict`: `if text.len() as u64 > max { TooLarge }`; `canonical_json::parse(text).map_err(|e| HubError::Shape(IngestError::from_json(e)))`; unless `breaks::on("lax-reader")`, walk keys and strings recursively (depth is already ≤ 32 by the parser) and return `Nul { path }` at the first `'\0'`. Slug check is a byte loop, no regex.
- [ ] **Step 4:** tests PASS; `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` exits 0. Commit.

### Task 3: Manifest reader, writer and growth

**Files:**
- Create: `hub/manifest.rs` (+ `hub/manifest/growth.rs` if over 300 lines), `hub/tests/manifest.rs`
- Modify: `ingest/read.rs` and `ingest/validate.rs` (widen to `pub(crate)`: `object`, `array`, `member`, `require_only`, `text_of`, `boolean`, `integer`, `shape`, `collection`, `field`, `link`, `check_title`, `check_coordinate`; `read/cell.rs` `cell` → `pub(crate)`), `ingest/write.rs` (`quoted` → `pub(crate)`)

**Interfaces:**
- Produces: `pub struct Manifest { pub version: u32, pub name: String, pub collections: Vec<Collection> }` (version = wire `manifestVersion`); `pub fn read_manifest(text: &str, plugin: &str) -> Result<Manifest, HubError>`; `pub fn manifest_json(m: &Manifest) -> String`; `pub enum Growth { Same, Grown }`; `pub fn growth(old: &Manifest, new: &Manifest) -> Result<Growth, HubError>`.

- [ ] **Step 1: Failing tests:** a two-collection manifest reads, with fields and collections sorted by id and link targets qualified (`"coll"` → `"tracker.coll"`, `"other.c"` kept); refuses: an unknown member, `version: 2`, a duplicate collection id, a duplicate field id, a missing title field, a link target `"a.b.c"`, 65 collections (413), 257 fields in one collection (413), a 262 145-byte text (413), NUL in a name (422); `manifest_json(read_manifest(t))` re-reads to an equal `Manifest`; growth: equal → `Same`; v2 adds a collection → `Grown`; v2 adds a field → `Grown`; v2 drops a field → 409; v2 changes a field's role → 409; same version, other content → 409; lower version → 409; v2 renames the manifest → `Grown`.
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement.** `parse_strict(text, MAX_MANIFEST_BYTES, "manifest")`; `require_only(root, "", &["collections","manifestVersion","name","version"])`; each collection through ingest `collection(value, path)`; then `check_collection_id`, duplicates, caps, `check_title`, link qualification; sort. `manifest_json` keys `collections`, `manifestVersion`, `name`, `version`, collections from `collection_piece`, no trailing newline.
- [ ] **Step 4:** PASS, `scripts/orch/gr cargo test -p graph-contract ingest::` still PASS. Commit.

### Task 4: Batch reader and write-time cell checks

**Files:**
- Create: `hub/batch.rs`, `hub/batch/cells.rs`, `hub/tests/batch.rs`

**Interfaces:**
- Consumes: `Manifest`, `Limits`, `parse_strict`, ingest `cell`, `record_piece`, `qualify`.
- Produces: `pub struct Upsert { pub collection: String, pub id: String, pub updated_at: u32, pub values: Vec<(String, JsonValue)> }` (values sorted by key); `pub struct Delete { pub collection: String, pub id: String }`; `pub struct Batch { pub upserts: Vec<Upsert>, pub deletes: Vec<Delete> }`; `pub fn read_batch(text: &str, limits: &Limits) -> Result<Batch, HubError>`; `impl Upsert { pub fn record(&self, plugin: &str) -> Record }` (qualified collection, `deleted: false`); `impl Batch { pub fn check(&self, plugin: &str, manifest: &Manifest, limits: &Limits) -> Result<(), HubError> }`.

- [ ] **Step 1: Failing tests:** a batch with one upsert and one delete reads; refuses an unknown member at root, upsert and delete level; `collection: "B.coll"` (422); a record id with `:`; the same `(collection, id)` twice across upserts and deletes (422); `max_batch + 1` ops (413); a NUL in a value key; `check` refuses an undeclared collection, an undeclared field, a Tags cell `["a","b:c"]` and a Tags cell `"b:c"`, a Weight `"x"`, a Parent `["a","b"]`, a many-Link `"a"`, a one-Link `["a"]`, and a record whose `record_piece` exceeds `max_record_bytes` (413); `check` accepts null in every role and anything in a Scalar; `-0` and `9007199254740991` in a Scalar cell survive `record_piece` byte-identical (`"-0"`, `"9007199254740991"`). Under `GM_HUB_BREAK=lax-reader` the `B.coll` and NUL tests must go red (the negctl).
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement.** Reader as in Task 3; `check_collection_id` skipped when `breaks::on("lax-reader")`. `cells.rs` is a `match field.role` over `Option<&JsonValue>`, with the head comment `// Caveat: mirrors graph-core's role readers (roles.rs); a reader change there that this misses lets a cell in that the motor then ignores.`
- [ ] **Step 4:** PASS. Commit.

### Task 5: Change, notice and answer writers

**Files:**
- Create: `hub/change.rs`, `hub/tests/change.rs`
- Modify: `ingest/write.rs` (add `pub(crate) fn record_rows(r: &Record) -> Vec<(&'static str, String)>` returning the record's members already written, used by `record` itself and by `change`; `members` → `pub(crate)`)

**Interfaces:**
- Produces: `pub struct ChangeHead<'a> { pub seq: u64, pub plugin: &'a str, pub at: &'a str }`; `pub fn change_json(head: &ChangeHead, upserts: &[(Record, u64)], deletes: &[(String, String, u64)]) -> String`; `pub fn manifest_change_json(head: &ChangeHead, m: &Manifest) -> String`; `pub fn notice_json(head: &ChangeHead) -> String`; `pub fn answer_json(seq: u64, applied: u64) -> String`; `pub fn max_change(limits: &Limits) -> u64`; `pub fn check_change(text: &str, limits: &Limits) -> Result<(), HubError>`.

- [ ] **Step 1: Failing tests:** `change_json` for one upsert (rev 4) and one delete (rev 7) equals the literal `{"at":"…","deletes":[{"collection":"t.c","id":"r2","rev":7}],"kind":"batch","plugin":"t","seq":118,"upserts":[{"collection":"t.c","deleted":false,"id":"r1","rev":4,"updatedAt":5,"values":{…}}]}`; every writer output parses with `canonical_json::parse` and contains no space and no newline; `manifest_change_json` keys `at, kind, manifest, plugin, seq`; `answer_json(118, 0)` = `{"applied":0,"seq":118}`; `max_change(&Limits::DEFAULT)` = 4 194 304 + 960 000; **1e300:** a batch body of `max_body − 64` bytes whose Scalar cells are `1e300` passes `read_batch`, and its change text exceeds `max_change` → `check_change` gives `TooLarge { what: "change", .. }` (the test's comment names the hub-report finding).
- [ ] **Step 2:** run, FAIL. **Step 3:** implement with `members`. **Step 4:** PASS; Task 1 pins still PASS. Commit.

### Task 6: Materialization model and pruning

**Files:**
- Create: `hub/model.rs`, `hub/prune.rs`, `hub/tests/materialize.rs`, `hub/tests/rng.rs` (splitmix64)

**Interfaces:**
- Produces: `pub struct Stored { pub rev: u64, pub text: String, pub record: Record }`; `pub struct Model { workspace: String, plugins: BTreeMap<String, Manifest>, records: BTreeMap<(String, String), Stored> }`; `Model::new(ws: &str) -> Result<Model, HubError>`; `register(&mut self, plugin: &str, m: Manifest) -> Result<Growth, HubError>` (65th plugin → 413); `apply(&mut self, plugin: &str, b: &Batch, limits: &Limits) -> Result<Applied, HubError>` with `pub struct Applied { pub upserted: Vec<(Record, u64)>, pub deleted: Vec<(String, String, u64)> }`; `to_ingest(&self) -> Ingest`; `to_json(&self) -> String`; `doc_bytes(&self) -> u64`.
- `hub::prune`: `pub fn kept_collection(c: &Collection, registered: &dyn Fn(&str) -> bool) -> Collection` (drops link fields whose target is not registered); `pub fn prune_record(r: &Record, kept: &Collection, exists: &dyn Fn(&str, &str) -> bool) -> Option<Record>` (None = nothing to prune).

- [ ] **Step 1: Failing tests (examples):** rev is 1 on create, +1 on a changing upsert, unchanged on an identical upsert (no `Applied` entry); delete then re-create gives rev 1; deleting an absent record is a no-op; a batch with one bad record changes nothing (atomic); plugins `a`, `a-b`, `b`: `to_json` lists collection `a-b.c` before `a.x` and records `("a.x","1")` … in tuple order; a link to `zz.c` (unregistered) is absent from the declaration and its cells are gone; a dangling parent target is pruned; a list emptied by pruning is removed while an originally empty list stays.
- [ ] **Step 2: Property test** `random_ops_materialize_canonically`: splitmix64 seeds 0..64, 40 random register/upsert/delete steps each; after every step assert `model.to_json() == ingest::to_json(&model.to_ingest())`, `ingest::read(&model.to_json())` is Ok, `to_json(read(x)) == x`, `to_json().len() as u64 <= doc_bytes()` with equality when nothing was pruned; at the end a model fed the same final state in reverse insertion order gives the same bytes, and two `to_json` calls are equal. Under `GM_HUB_BREAK=keep-dangling` or `keep-cells` this test must fail (the negctls).
- [ ] **Step 3:** run, FAIL. **Step 4: Implement.** `to_json` streams `DOC_HEAD`, kept collection pieces, `DOC_MIDDLE`, for each record `prune_record(...)` → its `record_piece` or `stored.text` verbatim, then `doc_tail(workspace)`. **Step 5:** PASS. Commit.

### Task 7: Codegen move and hub schema

**Files:**
- Create: `crates/graph-contract/src/codegen.rs`, `codegen/typescript.rs`, `codegen/tests.rs`, `hub/schema.rs`, `docs/contract/hub-schema.json`, `crates/graph-contract/generated/hub.d.ts` (both written by `codegen`, never by hand)
- Modify: `crates/graph-contract/src/lib.rs` (L24–273 move out; `#[cfg(feature = "codegen")] pub mod codegen;`)

**Interfaces:**
- `codegen::outputs()` keeps its first six entries in order (graph-cli's test reads `outputs()[1]`); appends `docs/contract/hub-schema.json` (`format!("{:#}\n", hub::schema::schema())`) and `crates/graph-contract/generated/hub.d.ts`.
- `hub::schema` (feature `codegen`): root `HubWire { manifest, batch, change, notice, answer, error }` of mirror structs with `#[serde(deny_unknown_fields, rename_all = "camelCase")]`, reusing `ingest::schema::{Collection, AnyValue}`; u64 fields `#[schemars(range(max = 9007199254740991))]`; `ChangeWire.kind` an enum `batch | manifest` with optional `upserts`, `deletes`, `manifest`; `ErrorWire { error, message }`.

- [ ] **Step 1:** Pure move first: `git mv`-equivalent cut of `pub mod codegen { … }` into `src/codegen.rs`, its tests into `codegen/tests.rs` (`include_str!` paths gain one `../`). `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` exits 0 and `scripts/orch/gr cargo test -p graph-contract --features codegen` PASS. Commit.
- [ ] **Step 2: Failing test:** `codegen/tests.rs` asserts `outputs().len() == 8`, the hub schema refuses (by its `additionalProperties: false`) an unknown member, and `hub.d.ts` contains `export interface HubWire`. Run, FAIL.
- [ ] **Step 3:** Extend `ts_type` for `anyOf`/`oneOf`, type arrays, `null`, `const`, string `enum` and `additionalProperties`; add `hub/schema.rs`; `scripts/orch/gr cargo run -q -p graph-cli -- codegen` writes the two files.
- [ ] **Step 4:** `codegen --check` exits 0; the six existing outputs are byte-unchanged (`git diff --exit-code docs/contract/ingest-schema.json crates/graph-contract/generated/` minus `hub.d.ts`). Commit.

### Task 8: Rows file and the gate

**Files:**
- Create: `scripts/orch/rows/hub-contract.rows`

- [ ] **Step 1:** Write the rows (names exact; `negctl-*` rows expect non-zero through `; test $? -ne 0`):

```
fmt|0|scripts/orch/gr cargo fmt --all --check
clippy|0|scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
clippy-hub|0|scripts/orch/gr cargo clippy -p graph-contract --all-targets --features codegen,negctl -- -D warnings
test|0|scripts/orch/gr cargo test --workspace --no-fail-fast
codegen|0|scripts/orch/gr cargo run -q -p graph-cli -- codegen --check
wasm32-core|0|scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown
hub-wire|0|scripts/orch/gr cargo test -p graph-contract --features codegen,negctl hub::tests -- --skip hub::tests::materialize
negctl-lax-reader|0|scripts/orch/gr -e GM_HUB_BREAK=lax-reader cargo test -p graph-contract --features codegen,negctl hub::tests::batch; test $? -ne 0
hub-materialize|0|scripts/orch/gr cargo test -p graph-contract --features codegen,negctl hub::tests::materialize
negctl-keep-dangling|0|scripts/orch/gr -e GM_HUB_BREAK=keep-dangling cargo test -p graph-contract --features codegen,negctl hub::tests::materialize; test $? -ne 0
negctl-keep-cells|0|scripts/orch/gr -e GM_HUB_BREAK=keep-cells cargo test -p graph-contract --features codegen,negctl hub::tests::materialize; test $? -ne 0
motor-lock|0|git diff --exit-code origin/develop -- Cargo.lock && git diff --exit-code origin/develop -- crates/graph-core/Cargo.toml
```
plus the six rows of `scripts/orch/rows/svc-floor.rows` verbatim (graph-server depends on graph-contract).
- [ ] **Step 2:** `scripts/orch/gate.sh <logdir> scripts/orch/rows/hub-contract.rows` → every row exits as declared. Commit.

## Self-review

- Spec §10 slice 1 items → tasks: types/readers/caps/NUL/id grammar (2, 3, 4), canonical writers (3, 5), cell checks (4), materialization model with dropped fields' cells (6), codegen (7), stream pieces with unchanged bytes (1), rows `hub-wire` and pure `hub-materialize` (8), byte order (6), head/tail lengths for `doc_bytes` (1, 6).
- Names used across tasks: `parse_strict`, `child`, `qualify`, `record_piece`, `collection_piece`, `frame_bytes`, `Limits::DEFAULT`, `Growth`, `Applied` — each defined once above.
