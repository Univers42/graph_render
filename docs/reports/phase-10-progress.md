# Phase 10 progress

Branch `p10`, based on `develop` (p3, p4, p5, p6e, p6f, p7 merged). Phase 9's gate is
**not** green (Phase 9 is unbuilt), so this phase was built in slices that do not depend
on it. This file records what landed, what was decided, and what is still open; it is
written to be continued, not read as a close-out.

---

## Slice 2 — the ingest contract (`graph-contract::ingest`) — DONE

**The design question the previous slice flagged** — where `JsonValue` lands — is
answered, and no new dependency was needed. `graph-contract/src/ingest.rs` defines the
types and is **dependency-free**, so `graph-core` reads them with the default feature set
and the motor's closed allow-list (`libm`, `indexmap`, `petgraph`) does not grow.
`serde`/`schemars` live behind the existing `codegen` feature, exactly as
`canonical_json::schema` already does.

### Changed

| Path | |
|---|---|
| `crates/graph-contract/src/ingest.rs` | CREATE — the types, and the decisions they encode |
| `crates/graph-contract/src/ingest/{collection,read,validate,write,error}.rs` | CREATE — the reader, the canonical writer, the cross-checks, the refusals |
| `crates/graph-contract/src/ingest/schema.rs`, `schema/enums.rs` | CREATE — the generated `docs/contract/ingest-schema.json` |
| `crates/graph-contract/src/ingest/tests/{support,roles,reader,writer,schema}.rs` | CREATE — 34 tests |
| `crates/graph-contract/src/lib.rs` | MODIFY — `pub mod ingest;`, and `outputs()` gains the fourth file |
| `docs/contract/ingest-schema.json` | CREATE — generated, committed |

### The contract, in one paragraph

`{ version, source, collections[], records[] }`. A collection declares its fields and
each field declares one of **eight roles** — `title`, `label`, `group`, `tags`, `link`,
`scalar`, `weight`, `parent` — plus, for a `link`, `{ collection, cardinality, symmetric }`.
A record is `{ id, collection, deleted, updatedAt, values: { fieldId: JsonValue } }`. A
ninth role is a stop-and-ask; there is no inference on top of the declaration.

### The H5 decision, recorded

**Constrain, do not re-grammar.** The contract's reader refuses a `source` or collection
id containing `:`, naming the coordinate, and `graph-core`'s derivation refuses a `:`
in a **tag value** (which the reader does not, because `tag:a:b` begins with the
reserved `tag:` prefix and no consumer following the same grammar can read the value back
out of it). A **record id** may contain `:` freely — it is the last segment, which is
exactly what `parse_node_id`'s `splitn(3, ':')` is for, and a test pins the round trip.

Broadening the grammar was the alternative and was **not** taken: it moves every existing
node id, and a node id that moves is a layout that moves, which is the phase prompt's
stop-and-ask. Gate row: `cargo test -p graph-core ingest_id_grammar` → 3 tests.

### Two decisions that are not in the phase prompt and needed making

1. **A collection's fields are sorted by field id, not kept in declaration order.** The
   first implementation kept document order, and the convergence test failed because the
   two adapters list the same fields differently (the rows shape in the order SQL gave
   them, the Notion shape sorted by property id). Sorting at the contract makes the
   canonical order a property of the *contract* rather than of an adapter, so "the first
   field with this role" is the same field in every document. The checks that **report**
   still use the document's own indices, so a refusal names where the field is in the
   file the reader has open.
2. **`parent` reads a bare string or a list of exactly one.** A source whose references
   are always a collection writes a one-element list even for a one-to-one relationship,
   and refusing that would make the contract unable to express a tree from such a source
   at all. A list of **two or more** is refused: picking the first would drop a subtree
   with nothing to say so.

### TDD trail

Each of these was run, not asserted: the tests were written first, observed failing
(`E0425`/`E0432`/`E0599` compile errors, then 8–10 failing assertions), and only then
implemented. Three negative controls were applied and each was killed:

| Mutation | Killed by | Result |
|---|---|---|
| the `parent` edge routed through `EdgeKind::Relation` in `build.rs` | 8 tests | 8 FAILED |
| the duplicate-tag-value guard removed from `tags_of` | `the_same_tag_value_twice_is_one_edge_and_one_hub` | 1 FAILED |
| `DEFAULT_WEIGHT` `0.5` → `1.0` | 2 tests | 2 FAILED |
| `notion.ts`'s `multi_select` mapped to `group` instead of `tags` | the convergence gate row | 1 FAILED, with the differing bytes printed |

---

## Slice 3 — `roles.rs` and `build.rs`, the one derivation — DONE

`graph-core/src/ingest/{roles.rs, build.rs, build/builder.rs, tests/*}`.

**All graph logic is here and none in an adapter.** A record becomes one `record` node
with id `source:collection:record`; `title` is its label, `label` its group, `weight` its
weight; `parent` is one `hierarchy` edge (parent first); a `link` is one `relation` edge
per referenced record, directed unless `symmetric`; `tags` is one `tag` hub node per
distinct value plus one edge per value; `scalar` is declared and read by nobody; a
**deleted** record derives nothing at all. Every strength comes from
`ingest::edge_strength`, so this module cannot grow a second table.

`to_canonical_json` gives a derivation's **identity** — the one thing a snapshot cannot,
since a snapshot only exists after a layout has run. That is what the convergence fixture
pins.

44 tests, across `tests/{support,roles,build,edges,grammar,convergence}.rs`.

---

## Slice 4 — the two adapters, and the convergence proof — DONE

`crates/graph-sdk-js/src/adapters/{rows.ts,notion.ts}`, `fixtures/ingest/*`,
`harness/adapter-convergence.mjs`, `harness/sdk-smoke.mjs --adapter-convergence`,
`harness/write-expected-ingest.mjs`.

**Both adapters are pure mappings** and say so at the top of the file. `rows.ts` reads
each column's declared role. `notion.ts` maps a property's vendor `type` onto a role
through `typeToRole` — a **table, not a heuristic** — and takes a caller `roles`/`links`
override that outranks it, because a `relation` says what the vendor stored, not what it
*means* (two relations in one database are a parent and a blocking dependency, and the
type is identical for both).

### The proof, and how it is split

`fixtures/ingest/{rows.json,notion.json}` are the **same logical dataset in two source
shapes**. `expected-graph.json` holds both halves of the result — the contract document
(`ingest`) and the graph derived from it (`graph`) — so the two runtimes are pinned to
one artifact rather than to two that can drift:

- **Node** (`sdk-smoke.mjs --adapter-convergence`): both adapters map their source to
  byte-identical documents, equal to `expected.ingest`. The only runtime that can run
  TypeScript.
- **Rust** (`cargo test -p graph-core ingest`): `ingest::build` turns `expected.ingest`
  into exactly `expected.graph`, byte for byte, plus a consistency check on the committed
  bytes (unique ids, every endpoint names a node that exists, every strength is the
  table's own).

`graph-cli ingest --from <file> [--member ingest] [--out P | --check P]` runs the same
derivation from the command line, so a third party can see what the motor does with a
document without writing Rust, and a fixture can be regenerated and **diffed** rather
than hand-edited.

### What the graph says, and it is worth reading once

Eight nodes and nine edges from four task records and two person records: the deleted
task derives nothing; `tag:docs`, `tag:p0` and `tag:graph` exist although nobody declared
a tag collection; strengths are `2.0 / 1.0 / 0.75`, from the one table. It is in
`EXAMPLES.md` §3.

---

## Slice 5 — the published SDK — DONE

`crates/graph-sdk-js/{README.md, EXAMPLES.md, package.json, src/index.ts}`,
`package.json` (two scripts), `harness/read-snapshot-raw.mjs`.

The README's first section is **what this package does not do**: no rendering, no data
fetching, no 3D, no mutation or write path, no graph-theory reference (read the ledger
instead), no validator. `EXAMPLES.md`'s five examples are copied from gate rows that
pass, and each carries the command that runs it.

`harness/read-snapshot-raw.mjs` is the universal-contract promise proved: it reads a real
snapshot with `JSON.parse` and **nothing else** — no import from this package, no wasm —
and checks the committed schema against it. It is a gate row.

---

## Slice 6 — the ledger rows — DONE, honestly `Implemented`

`crates/graph-cli/src/capabilities/ingest.rs` adds `ingest.roles`, `ingest.build`,
`adapter.rows`, `adapter.notion`. All four are **`Status::Implemented`, not `Gated`**, and
that is the honest status rather than a shortfall: `problems()` accepts a `Gated` row only
while a recorded gate run backs it, and the oracles these capabilities have are the
convergence fixture and the contract round trip — neither of which is a recorded run yet.
`hash_stage` and `oracle_record` name the records that would back them. **Promoting them
to `gated` is the orchestrator's**, after `gate.sh` has recorded the runs.

## Slice 7 — `gm_build_contract`, the additive contract path — DONE

The envelope was extended to `crates/graph-wasm/src/**`, `crates/graph-sdk-js/src/**`,
`harness/sdk-smoke.mjs` and `docs/contract/wasm-abi.md`, and the orchestrator's decision
on the stop-and-ask was: **do not break `gm_build`**. It has not. `gm_build` and the
provisional node/edge JSON are byte-for-byte what they were; the studio in `app/` and the
hash gate's C20 stage keep working unchanged. What was added is a second door.

### Changed

| Path | |
|---|---|
| `crates/graph-wasm/src/contract.rs` | CREATE — bytes → contract → `graph_core::ingest`'s one derivation → topology. No graph logic in it. |
| `crates/graph-wasm/src/contract/tests.rs` | CREATE — 8 tests |
| `crates/graph-wasm/src/exports/build.rs` | MODIFY — `gm_build_contract`, one export and its docs |
| `crates/graph-wasm/src/errors.rs` | MODIFY — `Code::ContractInvalid = 14`, appended |
| `crates/graph-wasm/src/lib.rs` | MODIFY — `mod contract;` |
| `crates/graph-sdk-js/src/{index,errors,wasm}.ts` | MODIFY — `Motor#buildContract`, `ContractRefusedError`, the raw export |
| `harness/sdk-smoke.mjs` | MODIFY — `buildContract` coverage, and the whole convergence proof in one process |
| `package.json` | MODIFY — `sdk:convergence:wasm` |
| `docs/contract/wasm-abi.md` | MODIFY — the export, `Code` 14, "Two build paths", "The convergence proof, one command", coverage rows, file sizes |

### What it does, and why it is additive rather than a replacement

`gm_build_contract(ptr, len)` takes a **phase-10 ingest contract document**
(`docs/contract/ingest-schema.json`) and builds a handle by calling
`graph_core::ingest::build_topology` — the same call `graph-cli ingest` and the
convergence fixture make. The alternative, breaking `gm_build`, would have moved a
published ABI's meaning; this adds a symbol, an error code and a path that did not exist,
and nothing that already worked moved.

**The two formats are deliberately not interchangeable, and each reader refuses the
other's document.** That is not decoration: it is what stops this quietly becoming one
export. Routing `gm_build_contract` to the provisional parser would derive nothing from a
contract document; routing `gm_build` here would refuse every document the studio sends.
Both directions are pinned by `the_two_ingest_formats_are_not_interchangeable`.

**`Code::ContractInvalid = 14`, not `IngestInvalid`.** Two different documents, two
different codes, so a caller cannot handle a contract rejection as a node/edge one. It
**appends**: `the_new_code_appends_and_does_not_move_any_other` pins all fifteen wire
values, because `CODE_NAMES` in the SDK indexes the same order and a renumber would
silently turn a caller's `UnknownLayoutId` into a `NoGeometryYet`.

### Open item 3 is closed: the whole proof, one command

Before this slice the convergence proof needed two runtimes and no single command ran all
of it — the adapters are TypeScript (Node only) and the derivation is `graph_core::ingest`'s
(Rust only). `gm_build_contract` means Node can do the third step, so one process now
chains all of it:

```sh
gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release
node-slim.sh node --experimental-strip-types harness/sdk-smoke.mjs \
  --adapter-convergence target/wasm32-unknown-unknown/release/graph_wasm.wasm
```

Both adapters → one contract document → `Motor#buildContract` → the **committed
`expected-graph.json` graph**, checked in the same process that produced the document.
Two containers and two commands, and that is honest: `ge-rust` has no `node` and
`node:22-slim` has no `cargo`. What is single is the proof.

Run the second command with **no** wasm path and it checks the adapter half alone — that
mode predates this slice and still works, so the existing gate row does not change.

### Negative controls, each run and each killed

| Mutation | Killed by | Result |
|---|---|---|
| `contract.rs` derives an empty graph instead of calling `build_topology` | 5 tests | 5 FAILED |
| `contract.rs` turns a reader refusal into an empty document | 3 tests | 3 FAILED |
| `contract.rs` turns a **derivation** refusal into an empty graph | `a_coordinate_that_cannot_round_trip_is_refused_where_it_is_made` | 1 FAILED |
| the committed-order node id assertion swaps `t1`/`t2` | `the_committed_contract_derives_exactly_the_committed_graph` | 1 FAILED |
| `gm_build_contract` reports a refusal as `IngestInvalid` | `harness/sdk-smoke.mjs`'s `the refusal names ContractInvalid` | 1 FAILED |
| `Motor#buildContract` fabricates a handle on refusal instead of throwing | the four convergence refusals | 4 FAILED |
| the convergence proof compares against the reversed committed order | `the derived node ids are the committed ones` | 1 FAILED |

### The four refused mutations in the end-to-end proof

An unknown member, a role outside the eight, an unsupported version, and a tag value that
cannot round-trip through the node-id grammar. Each is asserted to have **actually changed
the text** first, so a mutation that stopped matching cannot pass as one — which is what
caught the first attempt at that check while it was being written. Each is a document a
lenient reader would derive *something* from, which is why the assertion is the refusal and
not "it produced a graph".

A document with no records at all is asserted to be a **legal empty graph**, not a
refusal: an empty dataset and a broken document are different facts and only one is a bug.

---

## Checks, with real exit codes

| Command | Exit |
|---|---|
| `gr cargo fmt --all --check` | 0 |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `gr cargo test --workspace --no-fail-fast` | 0 — **995 passed, 0 failed, 11 ignored** (was 833 + 8 + 1 before; the growth is this slice's 8 native tests plus the one code-order test, and the earlier count did not include doc/integration binaries the same way) |
| `gr cargo test -p graph-wasm contract` | 0 — 8 passed |
| `gr cargo test -p graph-core ingest_id_grammar` | 0 — 3 passed |
| `gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release` | 0 |
| `node harness/sdk-smoke.mjs <wasm>` | 0 — 185 checks |
| `node harness/sdk-smoke.mjs --adapter-convergence <wasm>` | 0 — **17 checks: the whole proof** |
| `node harness/sdk-smoke.mjs --adapter-convergence` (no path) | 0 — 4 checks, the adapter half alone |
| `node harness/wasm-run.mjs <wasm> --assert-zero-copy` | 0 |
| `node harness/read-snapshot-raw.mjs` | 0 |
| `tsc -p crates/graph-sdk-js/tsconfig.json --noEmit` | 0 |
| `eslint crates/graph-sdk-js/src --max-warnings=0` | 0 |
| `gr cargo run -p graph-cli -- capabilities --check` | **1** — see below |

**`capabilities --check` exits 1, and it did before this slice.** It reports 34 problems
over 36 rows, every one a pre-existing `Gated` row with no recorded run in `target/gates`:
"gated, but no hashgate record: run the gate". **Zero** name an `ingest.*`, `adapter.*` or
contract row, verified by grepping the output. The row cannot go green here: it needs
`hashgate`, `roundtrip` and `capabilities` themselves recorded, which is `gate.sh`'s job
under the host-wide lock this phase does not take. Not run, and not claimed: `hashgate`,
`roundtrip`, `ge-check`, `gate.sh`, `cargo mutants`.

Note on the `cargo test` total: the earlier slices' progress note reported 833 passed. The
full `--workspace --no-fail-fast` run in this slice reports 995, and the difference is
mostly integration/doc-test binaries counted this way rather than that one — the
authoritative number is the one from the command actually run, recorded above.

## The Ponytails

- **`DEFAULT_WEIGHT = 0.5`** (`roles.rs`): a chosen convention with no measurement
  behind it. Failing input: a dataset whose records declare no `weight`. Direction:
  silent but local. Escape hatch: declare the role.
- **A tag value containing `:` is refused** (`build.rs`): see H5 above. Failing input:
  `"a:b"`. Direction: loud, and refusing is the safe direction. Escape hatch: rename the
  value, or declare the field `scalar`.
- **An absent cell and an empty one are kept apart** (`roles.rs`): the *safe* direction —
  a caller gets a visibly empty group rather than a plausible wrong one.
- **An unmapped Notion property type becomes `scalar`** (`notion.ts`): the safe direction
  (a wrong structural role would add or remove edges) but still silent. Escape hatch: the
  caller's `roles` override. Stated in contrast with the heuristics it replaces: a table
  is a *total* function on a closed set read from the property itself, and its output is
  checked byte for byte by a gate row, which "the first `multi_select`, or a field named
  `/^tags?$/i/`" never was.
- **The strength table** rides forward unchanged from slice 1.

## Coverage

Every changed symbol has a test that exercises it, and slice 7's additions specifically:
`contract::derive` and `ContractError` by `crates/graph-wasm/src/contract/tests.rs` (8);
`Code::ContractInvalid`'s wire value and its not-renumbering by `errors.rs`'s
`the_new_code_appends_and_does_not_move_any_other`; `gm_build_contract` and
`Motor#buildContract` and `ContractRefusedError` by `harness/sdk-smoke.mjs`; the raw
export declaration by `wasm.ts`'s `RawExports` (checked away by `tsc --noEmit`).

Every changed symbol from the earlier slices has a test that exercises it: the contract's types, reader, writer
and schema by `crates/graph-contract/src/ingest/tests/*` (34); the roles and the
derivation by `crates/graph-core/src/ingest/tests/*` (44); the two adapters and the
canonicalizer by `harness/sdk-smoke.mjs --adapter-convergence`; the raw snapshot reader by
`harness/read-snapshot-raw.mjs`; the CLI subcommand by `ingest_cmd.rs`'s own tests and
`tests/cli.rs`; the ledger rows by `capabilities/tests/*`.

## Open

1. **CLOSED (slice 7). The wasm ABI takes the ingest contract.** `gm_build_contract`
   accepts a contract document and derives it through `graph_core::ingest`, additively —
   `gm_build` and the provisional format are unchanged. A JS consumer can now hand a
   contract to the module. See "Slice 7" above.
2. **`graph-cli ingest --check` and `capabilities --check` are not wired into
   `gate.sh`.** Both commands exist and work; adding the rows is the orchestrator's. Still
   open. A third candidate now exists for the same list:
   `sdk:convergence:wasm` (the one-command proof) — whether that is a gate row or a manual
   check is the orchestrator's call, since it needs the wasm build to have run.
3. **CLOSED (slice 7). The two halves of the convergence proof ran in different
   runtimes.** They still *can* be checked in two runtimes, and the Rust half still is
   (`cargo test -p graph-core ingest`), but the whole proof is now also a single Node
   process through wasm: `sdk-smoke.mjs --adapter-convergence <wasm>`. See "Slice 7" above.
4. **`docs/decisions/edge-strength-table.md` is still documentation-incomplete** on the
   per-kind old values: the host source tree is not readable from this worktree, so the
   client's `1.2 / 1.8 / 0.7 / 0.5` per-kind assignment and the query router's third set
   are recorded as the phase prompt states them, not as verified. Unchanged from slice 1.
5. **`Motor#buildContract` is not in `EXAMPLES.md`.** `EXAMPLES.md`'s five examples were
   copied from gate rows that pass (slice 5), and a sixth one for `buildContract` would
   have to be copied from a row that does not exist in `gate.sh` yet — `EXAMPLES.md` is
   outside this slice's envelope in any case. It is the one place a consumer would look
   first for the method, and the gap closes the moment open item 2's rows are wired.
