# Phase 10 — The ingest contract, two adapters, and the published SDK

**Read `prompt.md` first**, especially §4.1 (two faces). Phase 9's gate must be green.

## Goal

Close the loop at the front of the pipeline: a neutral, role-based ingest contract, two real adapters
proving it, and the SDK published as something a stranger can use.

## Why this is last, not first

The ingest contract is the **most externally visible** surface in the project — the thing third-party
adapters are written against — so it should be designed once the motor's real needs are known rather than
guessed at the start. By now we know exactly which fields every stage actually reads, which is the only
honest basis for a contract.

It is also where a **real, live defect** gets fixed. Graph derivation currently exists in osionos three
times over and the copies have **already diverged**: the client path uses edge strengths
`1.2 / 1.8 / 0.7 / 0.5`, the bridge path (`scripts/bridge-graph.mjs:88-121` `pagesToGraph`, consumed via
`mapGraphResponse.ts:54-57`) uses `3.2` for hierarchy and `1.4` for everything else, and the query
router's generators are a third. **Two live code paths produce different layouts for the same data.** One
implementation in `graph-core` with one documented constant table is the fix.

(osionos stays read-only. We fix the divergence *in the motor*, so osionos can later adopt one source of
truth. We do not edit osionos to do it.)

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-contract/src/ingest.rs
crates/graph-core/src/ingest/{mod.rs,roles.rs,build.rs,strength.rs}
crates/graph-sdk-js/src/adapters/{rows.ts,notion.ts}
crates/graph-sdk-js/EXAMPLES.md
fixtures/ingest/{rows.json,notion.json,expected-graph.json}
docs/contract/ingest-schema.json              (generated, committed)
docs/decisions/edge-strength-table.md         (REQUIRED — the divergence fix)
```

**MODIFY — exactly these:**
```
crates/graph-contract/src/lib.rs
crates/graph-core/src/lib.rs
crates/graph-core/src/registry.rs
crates/graph-sdk-js/{package.json,src/index.ts,README.md}
crates/graph-cli/src/{capabilities.rs,main.rs}
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. A declarative mapping DSL (out of scope — `minimalism-ladder.md`
rung 7 wants three real adapters; we have two). Anything in osionos.

## Steps

### 1. `roles.rs` — declared roles, not type sniffing

```
collections: [{ id, name, titleField, fields: [{ id, name,
                 role: "title"|"label"|"group"|"tags"|"link"|"scalar"|"weight"|"parent",
                 link?: { collection, cardinality: "one"|"many", symmetric: bool } }] }]
records:     [{ id, collection, deleted, updatedAt, values: { fieldId: JsonValue } }]
```

This is the whole point of the contract. The existing code *reverse-engineers* semantic roles from Notion
type strings — `deriveTagConfig.ts:23-32` picks "the first `multi_select`, or a field named `/^tags?$/i`";
`deriveGraph.ts:92-98` picks "the first `status`, else the first `select`". Those heuristics are why the
engine is Notion-shaped. Replacing a 20-member `PropertyType` enum with **eight declared roles** dissolves
the coupling, and it is what makes "any data source" true rather than aspirational.

It also fixes **H6**: `groupValue` currently depends on `Object.values()` ordering, which is
non-deterministic across JSON reserializations. A declared role has no ordering dependency.

### 2. `build.rs` — one derivation, not three

Ingest → topology, using the roles. Node synthesis (including tag-hub nodes), edge derivation, and
hierarchy from the `parent` role. **All logic here, none in adapters.** That is what makes the hard part
unrepeatable per source.

Respect **H5**: the node-id grammar cannot represent `:` in `source`/`databaseId`
(`ids.ts:61-66` carries its own PONYTAIL). Broadening to arbitrary sources makes this worse, not better.
Either constrain ingest ids (reject `:` in those coordinates, with a clear error) or change the grammar —
**decide and record it**. Silently producing ids that do not round-trip is the current behaviour and it is
a defect.

### 3. `strength.rs` — the divergence fix, with a table

One documented constant table for edge strengths by edge kind, replacing the three divergent sets.
`docs/decisions/edge-strength-table.md` records: the three existing sets with citations, the chosen
values, and the reason. The visible consequence — layouts will differ from both current behaviours — was
pre-accepted; state it plainly rather than burying it.

### 4. Two adapters, deliberately different

- `rows.ts` — flat SQL-ish rows plus a declared schema. The general case.
- `notion.ts` — the Notion shape, mapping its property types onto the eight roles.

Both are **dumb mappers**: source shape → ingest contract. If an adapter contains graph logic, that logic
belongs in `build.rs`. An adapter that is more than a mapping is the abstraction leaking.

### 5. The convergence test — this is the phase's proof

`fixtures/ingest/{rows.json,notion.json}` describe **the same logical dataset** in two source shapes.
Both must produce a **byte-identical** graph — same nodes, same edges, same ids, same strengths, same
snapshot hash. `expected-graph.json` pins it.

That single test is what demonstrates the contract is genuinely neutral. Without it, "any database engine"
is a claim; with it, it is a passing assertion.

### 6. Publish the SDK properly

`README.md` and `EXAMPLES.md` with examples **copied from passing tests**, never composed by hand
(`documenter.md`). The README must state what the motor does **not** do — no rendering, no data fetching,
no 3D, no mutation, and the not-ported algorithm list from `prompt.md` §10. A page with only a happy path
is marketing.

Include the JSON snapshot schema and a worked example of reading it **without** the SDK — in any language.
That is the universal-contract promise, and the example is what makes it real.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- roundtrip --seeds 1000           # 0

# THE convergence test: two adapters, one graph, identical bytes
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs --adapter-convergence # 0

# ingest id-grammar constraint is enforced, not hoped for
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core ingest_id_grammar                 # 0

# the JSON face is readable with no SDK at all
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/read-snapshot-raw.mjs               # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check              # 0
docker build -t ge-check . && docker run --rm ge-check                                           # 0
```

## Ledger delta

To `gated`: `ingest.roles`, `ingest.build`, `adapter.rows`, `adapter.notion`.

Then the final ledger is the project's deliverable summary: every capability, its status, its oracle, its
measured ceiling, its degradation, its Ponytail. **That table is the answer to "what does this motor do."**

## Ponytail requirements

- **Role inference**, if any convenience inference is offered on top of declared roles: name the failing
  input and the direction (a wrong role silently changes the graph — **dangerous**), and require the
  explicit declaration as the escape hatch.
- **The id grammar** (H5): carry `ids.ts:61-66`'s marker forward. `source`/`databaseId` containing `:`
  cannot round-trip, and the parse returns a shifted wrong result rather than `null`.
- **The strength table** is a chosen convention, not a derived truth. Name that changing it changes every
  layout, and that snapshot hashes are pinned to it.
- **No marker** on the adapters if they are pure mappings — say so plainly.

## Stop-and-ask

- The two adapters do **not** converge on an identical graph → **stop**. Do not relax the test to a
  tolerance. Non-convergence means the contract is not neutral, which is the one thing this phase exists to
  establish.
- The eight roles cannot express a real source's semantics → stop and ask before adding a ninth. Each role
  is a permanent public-surface commitment.
- Fixing H5 appears to require changing the id grammar in a way that moves existing node ids → stop.
  `NodeId == BaaS global id` exists to keep layout stable across rebuilds, and that is a decision with
  consequences outside this repo.
