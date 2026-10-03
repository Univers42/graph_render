# fix-contract — review-core-base findings F-02, F-08 … F-11, F-27 … F-29, F-66 … F-78, F-99

Every id the job body names has a row. A row is `false` only where a test that would
have failed on the defect cannot fail on this tree; the test is named and stays.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-02 | MAJOR | **false** | `f32_display_is_the_shortest_decimal_and_both_read_paths_give_the_value_back` (pre-existing, ~65.5k `f32` bit patterns, asserts no `e`/`E`) | `canonical_json.rs:214` unchanged |
| F-08 | MAJOR | fixed | `an_integer_past_the_exact_range_is_refused_rather_than_rounded` | `ingest/read/cell.rs:46,67` |
| F-09 | MAJOR | fixed | `a_field_without_the_link_member_is_refused_naming_it` (replaces the test that pinned the defect) | `ingest/read.rs:98` |
| F-10 | MAJOR | fixed | `a_repeated_values_key_collapses_to_the_first_in_document_order_and_still_reads` | `ingest/write.rs:148` |
| F-11 | MAJOR | fixed | `a_link_or_parent_cell_naming_an_unknown_record_is_refused`, `a_deleted_record_is_still_a_legal_link_target` | `ingest/validate/cells.rs:28`, wired at `validate.rs:34` |
| F-27 | MAJOR | **false** | `a_negative_zero_survives_the_json_face_with_its_sign_bit` | `canonical_json/tests/text.rs:14` |
| F-28 | MAJOR | **false** | `any_json_spelling_of_the_same_snapshot_reads_the_same` (pre-existing) | `canonical_json/read.rs:177` unchanged |
| F-29 | MAJOR | fixed (the **schema** was the defect) | `the_schema_types_a_z_column_as_an_array_and_never_requires_it` | `canonical_json/schema.rs:78` → `docs/contract/snapshot-schema.json:197,245,302` |
| F-66 | MINOR | **deferred** | — (see below) | `binary/decode.rs:71` (`Ponytail:` marker added) |
| F-67 | MINOR | fixed | `paths::the_point_read_is_sized_only_after_the_offsets_are_checked`, `paths::a_well_formed_head_over_a_short_payload_is_still_truncated` | `binary/decode.rs:151`, helper at `:166` |
| F-68 | MINOR | **false** | `an_endpoint_that_is_not_a_node_is_refused_before_any_writer_can_see_it` | `canonical_json.rs:99` unchanged |
| F-69 | MINOR | **false** | `node_ids_are_written_in_utf8_byte_order_where_utf16_would_disagree` | `canonical_json.rs:147` unchanged |
| F-70 | MINOR | **false** | `version_refusal_is_not_a_missing_version_which_reads_as_0_0` (pre-existing) | `canonical_json.rs:134` unchanged |
| F-71 | MINOR | fixed | `the_same_record_id_in_two_collections_is_two_records` (negative control: same-collection duplicate still refused, message byte-identical) | `ingest/validate.rs:163` |
| F-72 | MINOR | fixed | `an_empty_coordinate_is_refused_naming_which_one`, `a_record_id_made_of_separators_still_reads` | `ingest/validate.rs:81` |
| F-73 | MINOR | **false** (empty document) + **doc-only** (self-link) | `an_empty_document_reads_with_no_collections_and_no_records` (pre-existing) | `ingest.rs:44-54` (module doc) |
| F-74 | MINOR | **doc-only** | — | `ingest.rs:45-49` (module doc) |
| F-75 | MINOR | fixed | `to_json_refuses_a_document_whose_version_is_not_the_one_this_reader_accepts` (`#[should_panic]`) | `ingest/write.rs:23` |
| F-76 | MINOR | fixed (structural) | `cargo test -p graph-contract --all-features` green before and after; longest new function 16 lines | `ingest/write/value.rs:13` |
| F-77 | MINOR | fixed | `geometry::tests::an_unknown_wire_column_name_is_none_and_not_another_columns_name` | `geometry.rs:249`, `geometry/columns.rs:35` |
| F-78 | MINOR | **doc-only** | — (`version_refusal_spares_this_major_at_any_minor` already pins the behaviour) | `version.rs:75` |
| F-99 | MINOR | fixed (structural) | 4 byte-exact message pins added; `snapshot::tests::*` green before and after | `snapshot/error/display.rs:30` |

Handoff (not a review id): `Collection::first_with_role` **deleted** —
`ingest/collection.rs`. A repo-wide grep found zero callers (definition plus one comment in
`validate.rs`, both now gone).

## Judgement calls, with the rule each one follows

**F-02 `false`.** `docs/contract/binary-layout.md` §Numbers states the rule as Rust's `f32`
`Display`, "never an exponent" — the contract *wants* what the code does, so the reviewer's
proposed fix would contradict the document. The JS engine is the oracle for the 17
`core/model` numeric functions, not for canonical JSON text, and no reader in the tree
parses the JSON face expecting an exponent. Evidence gathered: `harness/oracle-wire.mjs:4-8`
carries every number as 16 hex digits of its IEEE-754 bits, so decimal spelling never enters
the oracle; `harness/adapter-convergence.mjs:80-97` writes its own canonicalizer *because*
`JSON.stringify` cannot (insertion order), sorts with `compareBytes` over UTF-8, and is used
only for the ingest face; `harness/read-snapshot-raw.mjs` validates the committed schema with
`JSON.parse` only and exits 0.

**F-27 `false`, as the brief expected.** `-0.0` written `-0` reads back with bits
`0x80000000`, and `to_bytes` of the snapshot read back is byte-identical to the original, so
the JSON face's requirement — round-trip to identical binary bytes — holds; `0` would not
hold it. One sentence added to the Numbers bullet of `binary-layout.md`.

**F-28 `false`.** The leniency is the design: `canonical_json.rs:138` documents `from_json` as
reading "a JSON document — canonical or not, keys in any order", `parse.rs:5` keeps a number's
text so each field's reader decides the rounding, and `any_json_spelling_of_the_same_snapshot_reads_the_same`
pins it. One sentence added to the Numbers bullet.

**F-29: the schema was the defect, not the reader.** `binary-layout.md:326-331` says
`geometry.nodes` carries a `"z"` **array** iff `dim` is `1`; a `null` is not an array, so the
reader's refusal is correct. The generator is fixed (`z_schema` + `schema_with` on all three
variants, the same mechanism `note_codes` already uses), `graph-cli codegen` rewrote
`docs/contract/snapshot-schema.json`, and `read/geometry.rs` is untouched. `z` stays out of
`required` — its presence follows the top-level `dim`, which JSON Schema cannot express.

**F-08: refused, not widened.** `JsonValue::Number` is `f64` and `JsonValue` is a `pub enum`
without `#[non_exhaustive]` that `graph-core` matches exhaustively, so an integer variant
would be a breaking public change. The first attempt refused *every* bare integer past 2^53
and the two negative-control tests failed — `ingest::tests::writer` pins that `1e21` writes
as the bare integer `1000000000000000000000` and reads back, so that rule would have broken
the crate's own write-then-read invariant. The shipped rule tests the **canonical spelling**
instead: refuse a bare integer whose `f64` `Display` is not the text itself. `9007199254740993`
is refused (it reads as `9007199254740992`); the writer's own `1000000000000000000000` still
reads; `1000000000000000000001` is refused (it is not the text for the number it reads as).

**F-09: the reader was tightened.** Three statements of the rule say every member is named and
required (`ingest.rs:39`, `ingest.rs:94-95`, `read.rs:1-2`), `ingest/schema.rs:160-184`
deliberately puts `link` in `Field.required`, and `docs/contract/ingest-schema.json` requires
it. The reader was the outlier. The pre-existing test
`a_field_without_the_link_member_reads_with_link_none` asserted the defect and could not
survive the fix; it is replaced by `a_field_without_the_link_member_is_refused_naming_it`.

**F-11: deleted records stay legal targets.** The record-id universe includes deleted records,
because a reference to a deleted record is data a real source produces and the derivation — not
the reader — is what skips a deleted target (review F-106, another job). Refusing it would make
the reader stricter than the derivation it feeds. `Ponytail:` at `cells.rs::referenced_ids`
names what is *not* checked: a cell that is a number, a bool, a nested object, or a list nested
more than one deep.

**F-66 deferred.** `String::with_capacity(len)` is already bounded 1:1 by the remaining payload
(`Reader::take` refuses anything longer than what is left), so an absolute cap would be a
*policy* limit and no such constant exists anywhere in the crate — nor in `graph-wasm`, whose
F-16 asks for the same thing. A cap is one decision, not two, and it is not this job's to make.
Recorded as a `Ponytail:` marker at `binary/decode.rs:71` instead of a code change.

**F-67: `Offsets { index: 1 }` is unreachable.** The two checks `Reader::table` already applies
(start at 0, never decreasing) cannot produce index 1: `offsets[0] != 0` is reported at index 0
first, and a decreasing window at position 0 needs `offsets[1] < offsets[0] == 0`, impossible for
`u32`. The one check that *would* yield index 1 — "the last offset runs past the data", a clause
in `SnapshotError::Offsets`' doc — turns a genuinely short payload into an offsets error and
breaks the pinned `binary::tests::every_truncation_is_refused_as_truncated`. Not implemented;
that fault stays `Truncated`, asserted explicitly.

**F-73/F-74 doc-only.** Refusing a self-link would reject documents a source legitimately
produced, and refusing a duplicate *role* would defeat `Role::Scalar`, which exists precisely so
a source can declare every field it has. Both are now stated in the "What the reader refuses"
section of the `ingest.rs` module doc.

**F-99: exhaustiveness restored.** The first split routed `fmt` through a second match with a
`_ => Ok(())` arm, which would have printed an empty refusal for any variant added later — and
the comment claimed the opposite. Replaced by one exhaustive `message` match over all 18
variants with no wildcard: a new variant now fails to compile.

## The committed-document gate the brief demands

| document | result |
|---|---|
| `fixtures/ingest/expected-graph.json` (`ingest` member) | reads — 2 collections, 6 records, source `lib` |
| `fixtures/ingest/rows.json` | refused `the document: unknown member \`_comment\`` — a foreign source shape (`tables`/`columns`), not an ingest document; unchanged by this job |
| `fixtures/ingest/notion.json` | refused `the document: unknown member \`_comment\`` — same, `databases`/`properties` |

`ingest::tests::fixtures::every_committed_ingest_fixture_still_reads` is kept as a test, with all
three fixtures `include_str!`-ed so a rename is a build failure:

Derived output compared to the committed convergence graph, `graph-cli ingest --from
fixtures/ingest/expected-graph.json`:

```
derived == committed graph: True
```

No committed document is refused by any refusal added here.

## Commands (real exit codes)

```
scripts/orch/gr cargo test -p graph-contract --all-features                  -> 0  (140 passed)
scripts/orch/gr cargo fmt --all --check                                      -> 0
scripts/orch/gr cargo clippy --workspace --all-features --all-targets -D warnings -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                          -> 1  (1 pre-existing failure, see below)
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown      -> 0
scripts/orch/gr cargo run -p graph-cli -- codegen --check                      -> 0  (4x "up to date")
scripts/orch/gr cargo run --release -p graph-cli -- roundtrip --seeds 100      -> 1  (pre-existing, see below)
scripts/orch/gr cargo run --release -p graph-cli -- hashgate --seeds 8        -> 0  (PASS)
scripts/orch/gr cargo run --release -p graph-cli -- capabilities --check      -> 1  (36 rows, 0 value mismatches)
scripts/scigraphs-conformance.sh                                              -> 0  (PASS)
```

### RED / GREEN for the fixes

RED, with the five ingest guards reverted in place and the tests left standing:

```
test ingest::tests::reader::a_field_without_the_link_member_is_refused_naming_it ... FAILED
test ingest::tests::reader::an_integer_past_the_exact_range_is_refused_rather_than_rounded ... FAILED
test ingest::tests::roles::a_link_or_parent_cell_naming_an_unknown_record_is_refused ... FAILED
test ingest::tests::roles::an_empty_coordinate_is_refused_naming_which_one ... FAILED
test ingest::tests::roles::the_same_record_id_in_two_collections_is_two_records ... FAILED
test result: FAILED. 35 passed; 5 failed; 0 ignored; 0 measured; 96 filtered out
```

GREEN after restoring:

```
scripts/orch/gr cargo test -p graph-contract --all-features ingest::
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out
```

F-29 RED: `left: Array["array","null"]  right: "array"`; GREEN after `graph-cli codegen`.
F-27 / F-68 / F-69 RED and GREEN are in `canonical_json::tests::text`.
F-08's *first* rule failed its own negative control: `a_number_writes_in_the_shortest_form_that_reads_back_as_the_same_f64`
and `every_json_value_kind_writes_and_reads_back` both FAILED on `1e21`; the canonical-spelling
rule is what made them green again.

### Two failures that are not mine, with the evidence

1. **`roundtrip --seeds 100` exit 1** — `layout.dag.sugiyama on its structural invariants on 93/100 seeds`,
   six seeds with `reversed=true` and no `dag.edge_reversed` note. The check lives in
   `crates/graph-cli/src/snapshot_cmd/dag.rs:44` and the layout in
   `crates/graph-core/src/layout/sugiyama/`, last changed by commit `0bed9cb` ("updated",
   2026-10-02 20:54, 12 files under `layout/sugiyama/` + `docs/decisions/sugiyama-heuristics.md`) —
   ten hours before this job's first edit, and in paths this job may not touch. The same run
   reports `binary <-> JSON byte-exact on 4000/4000 snapshots` and grid / circular / packing
   `100/100`, which are the faces this job changed. **Decision needed: re-pin or fix by the
   sugiyama owner; recommendation — fix in `layout/sugiyama`, do not re-pin from here.**
2. **`cargo test --workspace` exit 1** — `layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them`
   panics at `oracle_probe.rs:58`: `target/probe/rank1000.txt is missing; see this module's doc for
   the command that writes it`. A missing generated probe file in graph-core, unrelated to
   graph-contract and independent of any edit.

### `capabilities --check` exit 1 is structural

All 36 rows are `hashgate ran 8 seeds, need 1000`, `no oracle-diff record: run the gate`,
`no roundtrip record: run the gate`, or `hashgate record is from another tree: re-run the gate`.
**Zero rows are a value mismatch.** The job's own done-when asks for `hashgate --seeds 8`, which
by construction cannot satisfy a check that demands the 1000-seed record, and the brief forbids
this job from running the timed gate (`hashgate --seeds 1000`) that would. Recorded, not worked
around.

## House limits

`wc -l`: `ingest/read.rs` 251, `ingest/read/cell.rs` 72, `ingest/validate.rs` 235,
`ingest/validate/cells.rs` 105, `ingest/tests/reader.rs` 275, `ingest/tests/fixtures.rs` 52,
`ingest/tests/roles.rs` 201, `canonical_json/schema.rs` 200, `canonical_json/tests.rs` 286,
`canonical_json/tests/text.rs` 89, `canonical_json/tests/generated.rs` 30,
`snapshot/error.rs` 120, `snapshot/error/display.rs` 224,
`ingest/write.rs` 236, `ingest/write/value.rs` 67, `binary/decode.rs` 178, `binary/tests/paths.rs` 103,
`geometry.rs` 295, `geometry/columns.rs` 55, `version.rs` 129 — all under 300.
Longest function written or changed: `SnapshotError` `message` 36 lines
(`snapshot/error/display.rs:30`), `Reader::table` 29, `read::document` 31. Three files crossed the
300-line limit while fixing their own findings and were split: `ingest/read.rs` (307 → `read/cell.rs`),
`ingest/tests/reader.rs` (319 → `tests/fixtures.rs`, the committed-document gate), and
`canonical_json/tests.rs` (310 → `tests/text.rs` + `tests/generated.rs`).