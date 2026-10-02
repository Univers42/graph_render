# fix-core-ingest — review-core-base findings in the ingest path

Scope: `crates/graph-core/src/ingest.rs`, `crates/graph-core/src/ingest/**`, and the
`graph` half of `fixtures/ingest/expected-graph.json` (regenerated, never hand-edited).
Ids F-12, F-13, F-14, F-15, F-19, F-100 … F-109.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-12 | MAJOR | fixed | `build::links::a_directed_link_keeps_its_orientation_in_its_id`, `build::derive::every_derived_edge_id_is_the_grammars_own`, `convergence::the_committed_graph_is_exactly_what_the_derivation_produces` | `crates/graph-core/src/ingest/build/builder.rs:195` (was `:184`) |
| F-13 | MAJOR | fixed | `edges::the_same_record_id_in_two_collections_is_two_records_not_a_duplicate` | `crates/graph-core/src/ingest/build/builder.rs:135` (was `:118-124`) |
| F-14 | MAJOR | false | `build::links::a_parent_naming_a_deleted_record_is_stated_and_then_dropped_by_indexing` (green before and after) | `crates/graph-core/src/ingest/build/builder.rs:182` (was `:159-170`) |
| F-15 | MAJOR | fixed (round 2) | `roles::an_absent_value_is_none_and_an_empty_one_is_some_empty` (restored, RED then green), `roles::a_value_of_the_wrong_json_type_is_absent_not_a_lossy_conversion` (the negative control) | `crates/graph-core/src/ingest/roles.rs:74` — the fallback is the collection's declaration, not the record's cell |
| F-19 | MAJOR | fixed | `roles::the_first_field_with_a_role_is_the_lowest_id_whatever_order_they_were_declared_in`, `roles::link_fields_come_out_in_canonical_id_order_not_declaration_order` | `crates/graph-core/src/ingest/roles.rs:170` (was `:143`) |
| F-100 | MINOR | fixed | none (parameter count; clippy/build is the gate) | `crates/graph-core/src/ingest/build/builder/spec.rs:24` — `Spec::new` is 4, `Ends` carries the endpoints |
| F-101 | MINOR | doc-only | — | `crates/graph-core/src/ingest/build.rs:22` — the derivation table now says `has_note`/`icon` are not derived from any role |
| F-102 | MINOR | doc-only | `build::derive::the_whole_derivation_is_pinned_line_for_line` pins `version=0` on tag hubs | `crates/graph-core/src/ingest/build.rs:27` — stated in the table, not changed: a tag is not a record and carries no `updatedAt` |
| F-103 | MINOR | fixed (round 2) | `build::links::a_link_carried_by_both_records_is_one_edge` (new, RED then green), `build::links::a_reference_carried_twice_in_one_many_link_is_one_edge` | `crates/graph-core/src/ingest/build/builder.rs:134-152` — dedup keyed on the edge id over the whole walk (`edges_stated` at `:24`), not per record |
| F-104 | MINOR | doc-only | `roles::the_first_field_with_a_role_is_the_lowest_id_whatever_order_they_were_declared_in` | `crates/graph-core/src/ingest/roles.rs:53-56` — "first" is now stated as *lowest id*, so it is not field order |
| F-105 | MINOR | doc-only | `roles::the_title_comes_from_the_collections_title_field_not_from_searching_for_a_role` | `crates/graph-core/src/ingest/build.rs:14` — the row now says `titleField`, which is what the contract's `Role::Title` declares |
| F-106 | MINOR | doc-only | `build::links::an_edge_to_a_deleted_record_is_never_dropped_by_the_derivation_itself` | `crates/graph-core/src/ingest/build.rs:33-41` — the deleted-record claim is now split into "no node, no edge of its own" and "an edge towards one is stated, and `index_model` drops it" |
| F-107 | MINOR | fixed (round 2) | `build::role_cases::an_index_capacity_refusal_is_a_build_error_and_says_what_overflowed` (no RED possible on this tree — a `u32` index exhaustion is not reachable from a document a test can hold; the test pins the variant and its message) | `crates/graph-core/src/ingest/build.rs:195` — `index_model(..).map_err(..)` replaced the `.expect`; the variant is at `:128`, its `Display` arm at `:158` |
| F-108 | MINOR | fixed (round 2) | `strength::tests::the_table_states_no_strength_the_match_does_not` (new), `strength::tests::the_table_is_total_over_all_kinds` | `crates/graph-core/src/ingest/strength.rs:55` — `STRENGTH_TABLE` rows read `edge_strength(kind)` instead of repeating the five literals, so the numbers are written once (at `:27`) |
| F-109 | MINOR | doc-only | `roles::a_weight_is_derived_as_declared_and_is_never_clamped_to_the_zero_one_convention` | `crates/graph-core/src/ingest/roles.rs:93` — `NodeRecord.weight` documents 0..1 *by convention* (`records.rs:22`), and both committed source fixtures declare 3, 5 and 8. A clamp would silently rewrite every weight in the convergence dataset to `1.0` and a refusal would make the phase's own fixture unbuildable. Recorded as pass-through with a Ponytail line naming what it gets wrong. |

## Notes on the three judgement calls

**F-12 — the TypeScript oracle agrees.** There is no ingest derivation in `src/` at all:
the adapters (`crates/graph-sdk-js/src/adapters/{rows,notion}.ts`) only *map* a source
shape to the contract document, and `harness/adapter-convergence.mjs` canonicalizes the
document they produce. The graph half was derived in one place, this one, and the oracle
never derived an edge id from a field name. So the reviewer's premise holds and the code
was the defect: `relation:Blocks` → `relation:blocks`, both in the edge id and the label.

Committed files whose bytes changed: **`fixtures/ingest/expected-graph.json` only**, two
lines (the one relation edge's `id` and `label`), regenerated by the fixture's own
generator. Nothing under `app/` (studio), the SDK's committed fixtures or any hashgate
input carries a relation edge id — the only other occurrence of the old text in the tree
is `crates/graph-sdk-js/EXAMPLES.md:140`, a documentation transcript outside this job's
paths, reported below.

**F-15 — the contract decides, and it says "both".** `graph-contract/src/ingest.rs:87`
declares `Role::Label` as "A second string, the node's `group`" and `:90` declares
`Role::Group` as "A facet string; a grouping key with no hub nodes of its own", and
`docs/contract/ingest-schema.json:149-178` lists `group` with no per-role description at
all. Two roles, one node column, both plausible. Resolved the way that keeps every
committed byte intact: **`label` wins where both are declared, `group` is the fallback
for a collection that declares no `label` role.** A `Ponytail:` line at `roles.rs:63`
names what the fallback gets wrong.

**F-15, round 2 — which side decides there is a fallback.** Round 1 read the fallback off
the *record's cell* (`role_value(..Label).or_else(|| role_value(..Group))`), which does not
implement the sentence above: in a collection declaring both roles, a record with no label
cell took its `group` cell, so one collection derived two different node columns depending
on a cell's presence, with nothing in the output saying which rule ran. `roles::group` now
asks `role_field(collection, Role::Label).is_some()` — the **declaration** — and reads only
that role. RED on the restored test:

```
test ingest::tests::roles::an_absent_value_is_none_and_an_empty_one_is_some_empty ... FAILED
  left: Some("doing")
 right: None
```

`an_absent_value_is_none_and_an_empty_one_is_some_empty` had been rewritten onto
`group_role_only()` in round 1, where the `label` field is not even declared — so the first
block, whose comment says "the `label` role's cell (`note`)", tested nothing. It is back on
`one_of_each()` with only the `note` cell removed, and it passes unchanged. The negative
control is `a_value_of_the_wrong_json_type_is_absent_not_a_lossy_conversion`: an unreadable
label cell yields `None` while the `group` cell holds "doing", so any cell-driven fallback
fails it. Round 1's line "Resolved the way that keeps every committed byte **and every
existing test intact**" was wrong about "every existing test": that test was edited in the
same round to make the change look clean. No committed fixture moved — `expected-graph.json`
declares both `slug` (label) and `state` (group) and every record carries both cells.

**F-107, round 2 — not deferred after all.** `build_topology` already returned
`Result<(Derived, Topology), BuildError>` (`build.rs:186`), so `BuildError::Capacity
{ what: &'static str }` is purely additive: no signature moved, no variant was removed, and
`graph-wasm` needed no change — `crates/graph-wasm/src/contract.rs:53` holds the error as
`ContractError::Derivation(BuildError)` and matches on `ContractError`'s own variants, and
`exports/build.rs:123` only asks whether it is `Ok`. `index_model`'s `CapacityError` is now
mapped at `build.rs:195` instead of `.expect`ed at the old `:176`; the stale comment that
claimed the case unreachable is gone. No RED is possible on this tree — `next_index` is the
only producer and a `u32` exhaustion cannot be held in memory — so the test pins the two
observable halves: the variant exists and its message names what overflowed.

**F-103, round 2 — the dedup was one record wide.** `emitted` was declared inside
`relations`, so a reference repeated in one list was deduped and a symmetric link carried by
**both** records was not: the two `Derived` edges had one id (`make_edge_id` orders an
undirected edge's endpoints) and `index_model` kept the first. RED on the new
`build::links::a_link_carried_by_both_records_is_one_edge`:

```
  left: ["rows:task:r1--rows:task:r2:relation:blocks", "rows:task:r1--rows:task:r2:relation:blocks"]
 right: ["rows:task:r1--rows:task:r2:relation:blocks"]
```

`edges_stated: IndexSet<String>` now lives on the `Builder` (`builder.rs:24`) and keys the
dedup over the whole walk, first-appearance order preserved and nothing re-sorted (D3). The
spec-building half moved to `relation_specs` (`builder.rs:154`) because the filter needs
`&mut self` and the build needs `&self`. Within one record the behaviour is unchanged — the
same set covers both ways of saying it once.

**F-108, round 2 — one source of the numbers.** `STRENGTH_TABLE` spelled out the same five
literals the `match` spelled out, and only `the_table_is_total_over_all_kinds` kept them
equal. The rows now read `edge_strength(kind)`, which is possible because it is a `const fn`
— one place a strength is written down, and the public `graph_core::ingest::STRENGTH_TABLE`
export is unchanged. The module doc (old `:8`) and the `edge_strength` doc (old `:28`) no
longer claim the table is the source or that a totality test holds it in step.

**F-14 and F-106 — the doc was wrong, not the code.** The module doc claimed "a deleted
record derives nothing at all — no node, and no edge naming it", which is false: an edge
a *live* record draws towards a deleted record was (and still is) emitted, and
`index_model`'s pinned rule is `edges_skip_taken_ids_and_dangling_endpoints_without_claiming_the_id`
(`docs/reports/phase-01.md:364`). The derivation states what the document claimed and
indexing drops the edge — that split is the design, `Derived.edges` versus `Topology`, and
refusing in `build` instead would move a documented, tested boundary. Corrected the doc to
the two rules it actually has, and pinned both directions with tests that were green
before this change (which is what "false"/"doc-only" means here): the derivation neither
invents an endpoint nor drops the claim.

**F-108 — fixed by construction, evidence is the compiler.** `edge_strength` was a `const
fn` scanning a table with a `panic!` for a missing row, so a new `EdgeKind` variant was a
runtime panic in release builds. It is now a `const fn` exhaustive `match`, which makes a
missing row a compile error as the module doc already claimed. The totality test stays and
still guards the table. (Round 2 adds the other half — see the F-108 note above: the table
is now a view of the match, not a second copy of its numbers.)

## Fixture regeneration

`GM_WRITE_INGEST_GRAPH=1 cargo test -p graph-core the_committed_graph_is_exactly_what_the_derivation_produces`
now rewrites the fixture's `graph` member from the derivation and prints what it wrote —
the env var the committed failure messages already named, which until now was read by
nothing. Only the `graph` member's text is replaced (braces scanned, not the whole
document re-emitted), so the JavaScript-written `ingest` half cannot drift. Unit tests in
`tests/fixture_write.rs` cover the splice, idempotence, a `}` inside a string, and the
file's own pretty shape.

## Commands (real exit codes)

```
scripts/orch/gr cargo fmt --all --check                                        -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings           -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                           -> 0   (20/20 test binaries ok)
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown       -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8                 -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 \
  cargo run -q -p graph-cli -- hashgate --seeds 8                               -> 1   (control: "FAIL: 8 of 8 seeds diverge")
scripts/scigraphs-conformance.sh                                                -> 0   (PASS)
scripts/orch/node-slim.sh node harness/sdk-smoke.mjs --adapter-convergence     -> 0   (4 ok, "# pass")
```

The done-when rows, pasted:

```
scripts/orch/gr cargo test -p graph-core ingest
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 1022 filtered out; finished in 0.00s

scripts/orch/gr cargo test -p graph-cli ingest
test ingest_cmd::tests::deriving_the_committed_document_reproduces_the_committed_graph ... ok
test ingest_cmd::tests::a_missing_member_is_refused_by_name ... ok
test ingest_cmd::tests::a_document_that_is_not_the_contract_is_refused_rather_than_half_read ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 313 filtered out; finished in 0.01s
```

```
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 8/8 seeds
PASS

scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
```

## Outside this job's paths

- `crates/graph-sdk-js/EXAMPLES.md:140` — a documented transcript containing the old
  `relation:Blocks` edge id. It should read `relation:blocks` (a copy from the passing
  derivation), but the SDK job owns that file.
- `crates/graph-wasm/src/contract.rs:34,53` — holds `BuildError` in its public
  `ContractError::Derivation`; relevant to F-107, see the row.
