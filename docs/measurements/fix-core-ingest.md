# fix-core-ingest — review-core-base findings in the ingest path

Scope: `crates/graph-core/src/ingest.rs`, `crates/graph-core/src/ingest/**`, and the
`graph` half of `fixtures/ingest/expected-graph.json` (regenerated, never hand-edited).
Ids F-12, F-13, F-14, F-15, F-19, F-100 … F-109.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-12 | MAJOR | fixed | `build::links::a_directed_link_keeps_its_orientation_in_its_id`, `build::derive::every_derived_edge_id_is_the_grammars_own`, `convergence::the_committed_graph_is_exactly_what_the_derivation_produces` | `crates/graph-core/src/ingest/build/builder.rs:195` (was `:184`) |
| F-13 | MAJOR | fixed | `edges::the_same_record_id_in_two_collections_is_two_records_not_a_duplicate` | `crates/graph-core/src/ingest/build/builder.rs:135` (was `:118-124`) |
| F-14 | MAJOR | false | `build::links::a_parent_naming_a_deleted_record_is_stated_and_then_dropped_by_indexing` (green before and after) | `crates/graph-core/src/ingest/build/builder.rs:182` (was `:159-170`) |
| F-15 | MAJOR | fixed | `roles::a_group_role_field_is_read_when_the_collection_declares_no_label_role` | `crates/graph-core/src/ingest/roles.rs:54` (was `:150`) |
| F-19 | MAJOR | fixed | `roles::the_first_field_with_a_role_is_the_lowest_id_whatever_order_they_were_declared_in`, `roles::link_fields_come_out_in_canonical_id_order_not_declaration_order` | `crates/graph-core/src/ingest/roles.rs:170` (was `:143`) |
| F-100 | MINOR | fixed | none (parameter count; clippy/build is the gate) | `crates/graph-core/src/ingest/build/builder/spec.rs:24` — `Spec::new` is 4, `Ends` carries the endpoints |
| F-101 | MINOR | doc-only | — | `crates/graph-core/src/ingest/build.rs:22` — the derivation table now says `has_note`/`icon` are not derived from any role |
| F-102 | MINOR | doc-only | `build::derive::the_whole_derivation_is_pinned_line_for_line` pins `version=0` on tag hubs | `crates/graph-core/src/ingest/build.rs:27` — stated in the table, not changed: a tag is not a record and carries no `updatedAt` |
| F-103 | MINOR | fixed | `build::links::a_reference_carried_twice_in_one_many_link_is_one_edge` | `crates/graph-core/src/ingest/build/builder.rs:195` (was `:176-191`) |
| F-104 | MINOR | doc-only | `roles::the_first_field_with_a_role_is_the_lowest_id_whatever_order_they_were_declared_in` | `crates/graph-core/src/ingest/roles.rs:53-56` — "first" is now stated as *lowest id*, so it is not field order |
| F-105 | MINOR | doc-only | `roles::the_title_comes_from_the_collections_title_field_not_from_searching_for_a_role` | `crates/graph-core/src/ingest/build.rs:14` — the row now says `titleField`, which is what the contract's `Role::Title` declares |
| F-106 | MINOR | doc-only | `build::links::an_edge_to_a_deleted_record_is_never_dropped_by_the_derivation_itself` | `crates/graph-core/src/ingest/build.rs:33-41` — the deleted-record claim is now split into "no node, no edge of its own" and "an edge towards one is stated, and `index_model` drops it" |
| F-107 | MINOR | deferred | none | `crates/graph-core/src/ingest/build.rs:180` — `index_model(...).expect(...)` in `build_topology`. Fixing it means `build_topology` returns `Result<(Derived, Topology), BuildError>` with a new `BuildError::Capacity` variant: a **public** signature change on `graph_core::ingest`, and `crates/graph-wasm/src/contract.rs:34,53` holds `BuildError` in its own public `ContractError::Derivation` and maps it to one ABI code. Neither path is in this job's paths. Recommendation: add `BuildError::Capacity { what: &'static str }` and have `build_topology` return it; `ContractError` keeps its single code (its doc already says one code for both reader and derivation refusals), so wasm needs no change. |
| F-108 | MINOR | fixed | `strength::tests::the_table_is_total_over_all_kinds` (already green) | `crates/graph-core/src/ingest/strength.rs:41-52` — see the note below |
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
committed byte and every existing test intact: **`label` wins where both are declared,
`group` is the fallback for a collection that declares no `label` role.** That wires the
declared role (the reviewer's actual defect — a `group` field was silently dropped
entirely) without re-scoping what the contract says `Label` is. A `Ponytail:` line at
`roles.rs:53` names what the fallback gets wrong.

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
still guards the table.

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
