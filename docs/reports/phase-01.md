# Phase 1 report — topology

Shape: `prompt.md` §12. Every command output quoted below was re-run for this report on
2026-09-27 inside `ge-rust` (or `node:22-slim` where the gate names it), on the final tree
at commit `92dc2be`, tree fingerprint
`cf0d50c534df1c534c2c4f3ca4ece66b59948ee5b89ecba187d8e87cfe1af5af`. The gate rows were run in
order, with nothing committed between them: a commit that touches a fingerprinted path would
void the recorded results the ledger reads (§2). Documentation is outside the fingerprint, so
committing this report and the two decision-doc updates leaves the records current.

**Sandbox note.** As in Phase 0, this container sits behind a TLS-intercepting egress proxy.
`docker run --rm -v "$PWD:/w" <image> …` was run through a wrapper that adds only
`--network host`, the proxy CA and a cargo-registry cache volume. The image and the command are
unchanged. `ge-check` needed one more step, described in §7.

## 1. Authorization compliance

**Created, on the envelope:** `crates/graph-core/src/{arena,ids,index,csr,columns,weights,diff,neighborhood,edgekind,legend}.rs`,
`crates/graph-cli/src/oracle_fixtures.rs`, `harness/oracle-diff.mjs`, `fixtures/adversarial-ids.json`,
`docs/decisions/h1-byte-order.md`.

**Modified, on the envelope:** `crates/graph-core/src/lib.rs`, `crates/graph-cli/src/main.rs`,
`crates/graph-cli/src/capabilities.rs`, `package.json` (the `oracle:diff` script only).

**Deviations. Each is outside the envelope and named with its cause:**

| path | why |
|---|---|
| `crates/graph-core/src/records.rs` | The input types (`NodeRecord`, `EdgeRecord`) and the borrowed views (`NodeView`, `EdgeView`) that `index`, `diff`, `legend` and the fixtures share. Putting them in `index.rs` would take it past 300 lines. |
| `crates/graph-core/src/synthetic.rs` | `buildSyntheticModel` is one of the 17 functions (prompt.md §7.4), but the envelope lists no file for it. |
| `crates/graph-core/src/stage.rs` | The gate's "4-way hash equality on the topology stage" needs a topology stage to hash. Phase 0 had only `synthetic`. |
| `crates/graph-core/src/index/tests.rs` | `index.rs` plus its tests would pass 300 lines. |
| `crates/graph-core/tests/memory.rs` | Review fix-up (§9): the measurement behind `scale_ceiling`, committed so it can be reproduced. |
| `crates/graph-core/Cargo.toml` → `indexmap` | Step 2 requires `IndexMap`. It is on the closed dependency list, but the manifest is not on the MODIFY list. `Cargo.lock` gains indexmap and its two dependencies (`equivalent`, `hashbrown`). |
| `crates/graph-wasm/src/lib.rs` | `gm_topology`: the wasm32 arm of the topology stage. |
| `harness/wasm-run.mjs` | A `hash <seeds> <stage…>` mode, so the wasm arm hashes both stages. |
| `crates/graph-cli/src/hashgate.rs`, `hashgate/{compare,tests}.rs` | Stage-aware comparison and per-stage counts, and the result recorded for the ledger. With its comparison and tests, `hashgate.rs` would be 375 lines, so they moved to `compare.rs` and `tests.rs`. |
| `crates/graph-cli/src/runner.rs` | `run_status` made `pub`, for `oracle-diff`. |
| `crates/graph-cli/src/evidence.rs`, `fingerprint.rs`, `build.rs`, `Cargo.toml` (`[build-dependencies] sha2`) | Phase 0's review (finding 6) required the ledger to read recorded gate results before any row can say `gated`. This is that reader and the fingerprint that pins a record to a tree. The build-time fingerprint is a review fix-up (§9). |
| `crates/graph-cli/src/capabilities/{registry,verdict,tests}.rs` | `capabilities.rs` would pass 300 lines. |
| `crates/graph-cli/src/oracle_fixtures/{cases,cases/tests,eval,generate,pools,wire,tests}.rs` | `oracle_fixtures.rs` alone would be about 1 500 lines. |
| `harness/oracle-wire.mjs`, `harness/oracle-h9.mjs` | Same cause for `oracle-diff.mjs` (the 300-line cap). |
| `crates/graph-cli/tests/cli.rs` | The differential and its negative controls, run inside `cargo test`. |
| `crates/graph-cli` subcommand flags `emit-fixtures --out` and `oracle-diff --fixtures` | The cli test writes into a temporary directory and must not overwrite a real run's fixtures. |
| `.cargo/mutants.toml` | The mutation step (ONBOARDING §6.2). Every exclusion carries its reason. |
| `docs/decisions/h9-group-width.md` | Step 8 says to "record the deviation in `docs/decisions/`" but names no file. |
| `docs/measurements/p1-topology-memory.md` | The ledger delta requires a real `scale_ceiling` for `topology.index`. This is its derivation. |
| `docs/reports/phase-01.md` | This report. |

**Deviations from the phase text, not from the file list:**

- **`weight` is `f64`, not `f32`** (step 4 says `weight: Vec<f32>`). The oracle's `weight` is a JS `number`. An `f32` column cannot be byte-equal to it, because `applyDegreeWeights` produces values that `f32` rounds. `version` is `f64` for the same reason.
- **Memory is 442 B/node against the 33 B/node that §5.1 budgets.** It is measured and explained in `docs/measurements/p1-topology-memory.md`. Phase 9 owns the budget.
- **`hashString` lives in `src/core/math.ts:11`**, not `model/value.ts` as the reference table says (Phase 0's F2). It was ported from `math.ts`.
- **The ids row's `makeEdgeId` takes one `EdgeIdParts` value**, not five arguments. The house limit is four parameters.

No file under `src/`, `tests/` or `verify/` was touched, and nothing in osionos was touched.
The `.claude` submodule is unchanged.

## 2. Ledger diff

```
capabilities --json   before (Phase 0 tree):  []
capabilities --check  before:                 capabilities --check: 0 rows, 0 problems   exit 0
capabilities --check  after:                  capabilities --check: 8 rows, 0 problems   exit 0
```

After: 8 rows, every one `gated`. Each row's `hash_4way` and `oracle_diff` are **read from the
recorded gate results** in `target/gates/` (`hashgate.json`, `hashgate-control.json`,
`oracle-diff.json`), never typed. A row is `gated` only if all of these hold at the current tree
fingerprint:

- `hashgate.json`: `pass`, at least 1000 seeds, and every seed equal on the row's stage;
- `hashgate-control.json`: `pass == false` **and** the control diverged on that same stage;
- `oracle-diff.json`: `pass`, at least 1000 seeds, and every one of the row's oracle functions
  has at least one case and no unexplained mismatch.

Otherwise `--check` names what is missing (for example `not backed: hashgate ran 2 seeds, need
1000`, which a cli test asserts) and exits 1.

| id | oracle functions | complexity | scale_ceiling | hash_4way | oracle_diff |
|---|---|---|---:|---|---|
| `topology.index` | indexModel, emptyModel, nodesEqual, buildSyntheticModel, layoutGroups | O(n + m) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (3101 cases, 28 declared divergences) |
| `topology.csr` | indexModel | O(n + m) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (1000 cases) |
| `topology.weights` | applyDegreeWeights, buildSyntheticModel | O(n + m) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (2000 cases) |
| `topology.diff` | diffGraph, isEmptyPatch, edgesEqual | O(n + m) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (3000 cases) |
| `topology.neighborhood` | neighborhood, neighborhoodEdges | O(reachable n + m) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (2000 cases) |
| `topology.edgekind` | edgeKindFromType | O(len) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (1000 cases) |
| `topology.legend_counts` | deriveLegend | O(n + m + d log d + t log t) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (1000 cases) |
| `topology.ids` | makeRecordNodeId, makeNoteNodeId, makeTagNodeId, makeEdgeId, parseNodeId, hashString | O(len) | 9 700 000 | equal/1000 seeds (topology stage; negative control red) | byte-equal/1000 seeds (6080 cases, 149 declared divergences) |

<details><summary><code>capabilities --json</code> after, verbatim</summary>

```json
[
  {
    "id": "topology.index",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (3101 cases, 28 declared divergences)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "dedupe exact (first wins); H9 fixed (group is u32, the oracle's & 0xff is not copied). Ponytail (scale_ceiling): an estimate — measured natively on 64-bit with synthetic ids and projected onto wasm32's 4 GiB; wasm32's 4-byte pointers lower the real cost, longer ids raise it, and the caller's input records are not counted. Re-measure with crates/graph-core/tests/memory.rs",
    "complexity": "O(n + m)"
  },
  {
    "id": "topology.csr",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (1000 cases)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "builder exact (counting sort), no marker owed. Evidence gap: the oracle compares only indexModel's merged adjacency, so the out/in split and the hierarchy CSR rest on unit tests and the 4-way hash. Ponytail (orientation): child_of edges enter the hierarchy CSR source-first, i.e. the child as parent (Topology::hierarchy); Phase 3 decides before reading it",
    "complexity": "O(n + m)"
  },
  {
    "id": "topology.weights",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (2000 cases)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "none owed: libm log1p, bit-equal to the oracle",
    "complexity": "O(n + m)"
  },
  {
    "id": "topology.diff",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (3000 cases)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "none owed: exact set difference",
    "complexity": "O(n + m)"
  },
  {
    "id": "topology.neighborhood",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (2000 cases)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "none owed: exact BFS",
    "complexity": "O(reachable n + m)"
  },
  {
    "id": "topology.edgekind",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (1000 cases)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "ordering-dependent substring classifier: 'note_link_hierarchy' is hierarchy because that test runs first; unknown types silently become relation",
    "complexity": "O(len)"
  },
  {
    "id": "topology.legend_counts",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (1000 cases)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "none owed: counts only, colour stays in TypeScript (H7)",
    "complexity": "O(n + m + d log d + t log t)"
  },
  {
    "id": "topology.ids",
    "tier": 1,
    "stage": "topology",
    "geometry": null,
    "status": "gated",
    "oracle": "src/core/model (TypeScript, this repo)",
    "oracle_diff": "byte-equal/1000 seeds (6080 cases, 149 declared divergences)",
    "hash_4way": "equal/1000 seeds (topology stage; negative control red)",
    "scale_ceiling": 9700000,
    "degradation": "past the ceiling wasm32 cannot allocate and the module traps (no partial result); natively, memory permitting, index_model refuses with CapacityError once the arena would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    "ponytail": "parseNodeId: ':' in source or databaseId shifts the parse to a wrong result, not None (H5); hashString: i32::MIN pinned; makeEdgeId orders by bytes, not localeCompare (H1)",
    "complexity": "O(len)"
  }
]
```

</details>

## 3. Gate table

| # | command | expect | exit | result — last output |
|---:|---|---:|---:|---|
| 1 | `cargo fmt --check` | 0 | 0 | PASS |
| 2 | `cargo clippy --workspace -- -D warnings` | 0 | 0 | PASS |
| 3 | `cargo test --workspace` | 0 | 0 | PASS — 143 tests: graph-cli 49 + cli 8, graph-contract 16, graph-core 63, graph-wasm 7; 1 ignored (the memory measurement, run separately below) |
| 4 | `cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | 0 | PASS |
| 5 | `cargo run -p graph-cli -- hashgate --seeds 1000` | 0 | 0 | PASS — `synthetic: 4-way equal on 1000/1000 seeds` / `topology: 4-way equal on 1000/1000 seeds` / `PASS` |
| 6 | `GM_MUTATE_REFERENCE_DEGREE=9 … hashgate --seeds 8` | non-zero | **1** | PASS — `synthetic: 4-way equal on 0/8 seeds` / `topology: 4-way equal on 0/8 seeds` / `FAIL: 8 of 8 seeds diverge` |
| 7 | `cargo run -p graph-cli -- emit-fixtures --seeds 1000` | 0 | 0 | PASS — `17181 cases over 1000 seeds`: 1000 per function, except `emptyModel` 1, `makeEdgeId` 1080 (20 adversarial pairs × 4 argument orders on top) and `layoutGroups` 100 |
| 8 | `docker run --rm -v "$PWD:/w" -w /w node:22-slim npm run oracle:diff` | 0 | 0 | PASS — `oracle-diff: 1000 seeds, 19181 lines, node v22.23.3, icu 78.3` / `H1 pairs observed diverging: 148 · H9 cases crossing 255 groups: 28` / `PASS` |
| 9 | `cargo run -p graph-cli -- capabilities --check` | 0 | 0 | PASS — `capabilities --check: 8 rows, 0 problems` |
| 10 | `docker build -t ge-check . && docker run --rm ge-check` | 0 | 0 | PASS **sandbox-adapted** (§7) — typecheck, lint, `# tests 27` / `# pass 27` / `# fail 0` |

Beyond the listed gate:

| command | expect | exit | result |
|---|---:|---:|---|
| `capabilities --json` | 0 | 0 | PASS — 8 rows, all `gated` (§2) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 0 | PASS |
| `cargo test --workspace --all-features --release` | 0 | 0 | PASS — 143 passed, 1 ignored |
| `graph-cli codegen --check` | 0 | 0 | PASS — both committed files up to date |
| `GM_MUTATE_REFERENCE_DEGREE=nine … hashgate --seeds 8` (a typo in the control) | 2 | 2 | PASS — `GM_MUTATE_REFERENCE_DEGREE="nine": invalid digit found in string`; nothing recorded |
| per-stage digests, 1000 seeds, and 8 seeds with the native arm mutated | 0 | 0 | PASS — §4 |
| `cargo test --release -p graph-core --test memory -- --ignored --nocapture` | 0 | 0 | PASS — §4 |
| the four differential negative controls below | 1 each | 1, 1, 1, 1 | PASS |
| `cargo mutants --in-diff` (3 runs) | — | — | §5: 0 surviving mutants in new code; 2 new written exceptions covering 4 equivalent mutants |
| a scan of the 42 changed `.rs`/`.mjs` files for the house limits | 0 over | 0 over | PASS — no function over 40 lines or 4 parameters, no file over 300 lines |

**RED steps observed** (logs kept with the session evidence; each is the failing run before the
code that made it pass):

- **graph-core, tests before code.** Against stub implementations, 38 of the 53 tests then in
  graph-core failed on value, in every new module (csr, diff, ids, index, legend,
  neighborhood, stage, synthetic, weights). For example `nodes_dedupe_first_wins_in_input_order`
  gave `left: 0, right: 2`.
- **Generator pin.** `the_generator_writes_exactly_the_pinned_cases` failed with `left:
  "9300bdf6…"` against a zeroed pin, which is how the pin was taken; it now holds
  `6173f8b4c89d6d78…9960a731b70f22be1a1d2386`.
- **H9, raw nodes.** The first harness grouped the raw input nodes. With a dropped duplicate id
  the oracle arm printed `[0,0,0,0,0,0,1]` against graph-core's `[0,0,0,0,0,0]`, one unexplained
  mismatch. Fixed by grouping `indexModel`'s nodes, as `rebuild` does.
- **H9, rule too wide.** Under the first acceptance rule, a +256 corruption of one group passed:
  the cli test's "expect exit 1" assertion failed with exit 0. Fixed by the rule in §9 finding 2.
- **Stale binary.** `hashgate` and `emit-fixtures` from a binary built before an edit both exit 2:
  `graph-cli was built from tree 4fb5fde2c64f… but the tree is now 003b2605f826…: rebuild before
  recording`.
- **Mutation misses.** Every mutant that survived one run is caught by the next, which is the
  new test failing against that mutant (§5). The reservation test was also run by hand against
  both of its mutants: capacities `(8, 8)` against `(5, 5)`, and `(4, 4)` against `(3, 3)`.

**Negative controls of the differential**, re-run on the final 1000-seed fixtures. Each copies
the fixtures, injects one fault the way a graph-core bug would (fixing up the manifest digest),
and must exit 1:

| fault injected | exit | first line of the verdict |
|---|---:|---|
| an `indexModel` expectation's `notes` count raised by 1 | 1 | `MISMATCH line 11 seed 0 indexModel {"graph":"g0"}` |
| adversarial pair `["a","A"]` declared `diverges: false` | 1 | `PROBLEM adversarial pair ["a","A"] declares diverges=true, observed false` |
| one crossing `layoutGroups` expectation's last group raised by 1 | 1 | `MISMATCH line 5751 seed 300 layoutGroups …` |
| a directed adversarial `makeEdgeId` expectation with its endpoints swapped | 1 | `MISMATCH line 19104 seed null makeEdgeId {"directed":true,…}` |
| (cli test) one group `[0,…]` → `[256,…]` | 1 | `layoutGroups … 1 unexplained` |

## 4. 4-way hash table

The hashgate now hashes two stages per seed: `synthetic` (Phase 0) and `topology` (new). The
`topology` stage builds a model of `2 + seed % 600` nodes, runs `index_model` and
`apply_degree_weights`, and hashes every column, all three CSRs, the arena and the database
members (`graph_core::stage`).

| stage | seeds | native run 1 | native run 2 | wasm32 run 1 | wasm32 run 2 | equal |
|---|---:|---|---|---|---|---:|
| `synthetic` | 1000 | `5511cfef…0e30acb8` | `5511cfef…0e30acb8` | `5511cfef…0e30acb8` | `5511cfef…0e30acb8` | 1000/1000 |
| `topology` | 1000 | `fbf4779c…e443528b` | `fbf4779c…e443528b` | `fbf4779c…e443528b` | `fbf4779c…e443528b` | 1000/1000 |
| `synthetic`, mutated native (`=9`) | 8 | `b43b0b3f…b5c17cf3` | `b43b0b3f…b5c17cf3` | `24dcbb02…40ed4b43` | `24dcbb02…40ed4b43` | 0/8 |
| `topology`, mutated native (`=9`) | 8 | `4ac4effb…c4839597` | `4ac4effb…c4839597` | `6a90dfc8…1dc9dc0f` | `6a90dfc8…1dc9dc0f` | 0/8 |

The per-stage digests are the SHA-256 of that stage's per-seed lines. They were computed from
the same arm commands the gate runs (`graph-cli hashgate-arm`, `node harness/wasm-run.mjs …
hash`). The gate itself prints one digest per arm over both stages:

- `98520f8d…32944fd9` for all four arms at 1000 seeds;
- `90551adb…7e423535` (native) against `3d0c6d4e…a237cf59` (wasm32) for the control.

The same computation over both stages together gives exactly those values, so the per-stage
split hashes the gate's own lines. The synthetic stage's mutated digests equal Phase 0's: that
stage did not change. The wasm32 artifact is `graph_wasm.wasm`, sha256
`24389f222cf5c2bd819a18e9346f0d82b6979a2690a91bf76f91e11d59832d01`.

**Oracle differential** (`npm run oracle:diff`, 1000 seeds, Node v22.23.3, ICU 78.3):

| function | cases | byte-equal | declared | unexplained |
|---|---:|---:|---:|---:|
| `applyDegreeWeights` | 1000 | 1000 | 0 | 0 |
| `buildSyntheticModel` | 1000 | 1000 | 0 | 0 |
| `deriveLegend` | 1000 | 1000 | 0 | 0 |
| `diffGraph` | 1000 | 1000 | 0 | 0 |
| `edgeKindFromType` | 1000 | 1000 | 0 | 0 |
| `edgesEqual` | 1000 | 1000 | 0 | 0 |
| `emptyModel` | 1 | 1 | 0 | 0 |
| `hashString` (H4) | 1000 | 1000 | 0 | 0 |
| `indexModel` | 1000 | 1000 | 0 | 0 |
| `isEmptyPatch` | 1000 | 1000 | 0 | 0 |
| `layoutGroups` (H9) | 100 | 72 | 28 | 0 |
| `makeEdgeId` (H1) | 1080 | 931 | 149 | 0 |
| `makeNoteNodeId` | 1000 | 1000 | 0 | 0 |
| `makeRecordNodeId` | 1000 | 1000 | 0 | 0 |
| `makeTagNodeId` | 1000 | 1000 | 0 | 0 |
| `neighborhood` | 1000 | 1000 | 0 | 0 |
| `neighborhoodEdges` | 1000 | 1000 | 0 | 0 |
| `nodesEqual` | 1000 | 1000 | 0 | 0 |
| `parseNodeId` | 1000 | 1000 | 0 | 0 |

- **Declared, each with its written reason:** H1 (`docs/decisions/h1-byte-order.md`): 149
  `makeEdgeId` cases over 148 distinct pairs, each accepted only because the edge is undirected,
  the two orders differ, and both ids are exactly the byte- and locale-ordered ones. H9
  (`docs/decisions/h9-group-width.md`): 28 `layoutGroups` cases, every case that crossed 255
  groups, each accepted only because graph-core's line *is* the widened groups and the oracle's is
  exactly them `& 0xff`.
- **Unexplained: 0.** The known-divergence list gained nothing to make this green.
- The 20 adversarial pairs' `diverges` declarations all matched what was observed (13 diverge,
  7 controls). `hashString` includes `"xfjfxtf"`, the `i32::MIN` input.
- The record the ledger reads (`target/gates/oracle-diff.json`) carries the runtime it ran under:
  Node v22.23.3, ICU 78.3, locale en-US.

**Memory, as two numbers** (`docs/measurements/p1-topology-memory.md`; re-run with
`cargo test --release -p graph-core --test memory -- --ignored --nocapture`):

| n | m | string arena | columns + 3 CSRs + indices | held | held / node | peak (records included) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 000 | 1 541 | 39 320 B (39.3 B/node) | 356 450 B | 395 770 B | 395.8 B | 947 787 B |
| 10 000 | 15 474 | 427 990 B (42.8 B/node) | 4 794 882 B | 5 222 872 B | 522.3 B | 10 901 147 B |
| 100 000 | 154 978 | 4 636 520 B (46.4 B/node) | 39 566 064 B | 44 202 584 B | 442.0 B | 101 430 310 B |

## 5. Coverage table

Each graph-core function the oracle has a counterpart for is exercised twice: by its unit tests,
and by the 1000-seed differential ("differential"), which reaches graph-core through
`oracle_fixtures::eval`.
"cli" means `crates/graph-cli/tests/cli.rs`, which runs the real binary.

| symbol | exercised by |
|---|---|
| `arena::{StringArena::{intern,find,get,len,is_empty,byte_len,resolve}, Interned}` | `interning_collapses_repeats_and_keeps_first_seen_order`, `find_never_stores_and_tells_near_misses_apart`, `the_last_u32_is_never_handed_out_as_an_index` |
| `arena::CapacityError` Display | `a_capacity_error_names_what_overflowed` |
| `arena::{Fnv1a, FixedState}` | `fnv1a_matches_the_published_64_bit_vectors` |
| `ids::{make_record_node_id,make_note_node_id,make_tag_node_id}` | `node_ids_join_their_coordinates`; differential |
| `ids::{parse_node_id, RecordRef}` | `parse_reads_record_ids_back_and_refuses_note_and_tag_ids`, `parse_rejoins_composite_keys_and_defaults_missing_segments`, `parse_shifts_a_colon_in_source_instead_of_refusing_h5`; differential |
| `ids::{make_edge_id, EdgeIdParts}` (H1) | `edge_ids_keep_direction_or_order_endpoints_by_bytes`; differential with the 20 adversarial pairs |
| `ids::{hash_string, code_points_at}` (H4) | `hash_string_matches_the_oracle` (surrogates, `i32::MIN`, the sum past `i32::MAX`); differential |
| `index::{index_model, admit_node, admit_edge, next_index, intern_opt}` | `nodes_dedupe_first_wins_in_input_order`, `edges_skip_taken_ids_and_dangling_endpoints_without_claiming_the_id`, `the_last_u32_is_never_handed_out_as_an_index`, `index_model_reserves_each_column_once_for_its_input`; differential |
| `index::{build_adjacency, Topology::{incident,out,inbound,hierarchy}}` | `incident_lists_edges_in_edge_order_and_a_self_loop_twice`, `hierarchy_rows_hold_each_parents_hierarchy_edges_only`; `the_adjacency_and_database_members_reach_the_bytes` |
| `index::{group_nodes}` (H9) | `group_is_the_first_seen_index_of_source_past_255`; differential `layoutGroups` |
| `index::{Stats, Topology::{by_database,stats,node_count,edge_count,node_index,edge_index,node,edge,strings,nodes,edges}}` | `by_database_is_first_seen_and_stats_count_kept_things`, the index tests above |
| `index::{empty_model}` | differential `emptyModel` (the mutant that swaps it for `Default::default()` is equivalent; see Mutation testing below) |
| `index::nodes_equal` | `nodes_equal_ignores_id_and_compares_floats_like_js_strict_equality`, `nodes_equal_sees_every_compared_field`; differential |
| `csr::{Csr::{from_pairs,row,rows,len,is_empty,byte_len}, build}` | `rows_keep_arrival_order_not_value_order`, `empty_and_zero_row_adjacencies_are_well_formed`, `a_repeated_pair_is_kept_twice`, `byte_len_counts_offsets_and_values`, `a_row_out_of_range_panics` |
| `csr::Incident::{merge,next}` | `incident_merges_two_ascending_rows_keeping_ties_twice` |
| `columns::{NodeKind::{ALL,name,from_name}, NodeColumns, EdgeColumns}` | `node_kind_names_round_trip`, `with_capacity_reserves_room_in_every_column`, `byte_len_counts_one_element_of_every_column` |
| `weights::{apply_degree_weights, apply_degree_weights_against, degree_weight, js_clamp, REFERENCE_DEGREE}` | `the_nine_reachable_weights_match_the_oracle_bit_for_bit`, `degree_counts_raw_edges_including_dangling_duplicate_and_self_loops`, `the_reference_degree_moves_the_saturation_point`, `js_clamp_propagates_nan_where_f64_max_would_drop_it`; differential; the hashgate negative control |
| `diff::{diff_graph, is_empty_patch, edges_equal, Patch}` | `a_model_diffed_against_itself_is_empty`, `nodes_are_added_updated_and_removed_in_the_oracles_order`, `edges_are_added_updated_and_removed_by_id`, `each_patch_list_alone_makes_it_non_empty`, `edges_equal_sees_every_compared_field_and_ignores_id`; differential |
| `neighborhood::{neighborhood, neighborhood_edges, Neighborhood}` | `one_hop_takes_every_incident_edge_and_its_far_ends`, `two_hops_walk_the_frontier_in_discovery_order`, `depth_zero_is_the_node_alone_and_a_huge_depth_stops_when_nothing_is_new`, `an_unknown_id_is_empty_not_a_phantom`; differential |
| `edgekind::{EdgeKind::{ALL,name,from_name}, edge_kind_from_type}` | `names_round_trip_and_unknown_names_are_none`, `classifier_follows_the_oracle_branch_by_branch`, `branch_order_decides_a_type_with_two_markers`; differential |
| `legend::{derive_legend, LegendCounts, DatabaseCount, TagCount}` | `databases_sort_by_count_keeping_first_seen_order_on_ties`, `tags_count_incident_edges_and_sort_stably`, `kinds_are_counted_in_first_seen_order`; differential |
| `records::{NodeRecord, EdgeRecord, NodeView, EdgeView}` | every index, diff and legend test builds through them |
| `synthetic::{build_synthetic_model, synthetic_count, synthetic_records, synthetic_node, synthetic_edges, Mulberry32}` | `count_floors_clamps_and_reads_non_finite_as_two`, `six_nodes_match_the_oracle`, `stats_match_the_oracle_at_40_nodes_and_at_the_cap`, `no_draw_a_synthetic_model_can_reach_is_exactly_one_half`; differential |
| `stage::{topology_stage, remix, encode, encode_node, encode_edge, put_*}`, `StageError` | `the_stage_is_deterministic_and_seed_and_reference_reach_it`, `a_non_finite_float_is_refused_not_hashed`, `the_remix_populates_every_edge_kind_and_crosses_256_groups`, `the_layout_is_pinned_for_a_tiny_topology`, `the_seed_sizes_the_graph_at_two_plus_seed_mod_600_nodes`, `the_adjacency_and_database_members_reach_the_bytes`; hashgate |
| `graph_wasm::exports::gm_topology` (wasm32 only) | cli `hashgate_*` through `harness/wasm-run.mjs` |
| graph-core `tests/memory.rs` | itself (`--ignored`; a measurement, not a check) |
| `evidence::{Stamp::{take,against,fingerprint,still_current,unchanged}}` | `a_stamp_needs_the_built_tree_and_goes_stale_when_the_tree_moves`, `the_real_tree_is_the_one_this_binary_was_built_from` |
| `evidence::{write, write_to, read, read_from, gates_dir}` | `a_record_reads_back_as_written_and_only_absence_is_none`; cli |
| `fingerprint::{fingerprint_of, collect, hex_sha256, FINGERPRINTED}`, `build.rs` | `the_fingerprint_moves_with_content_names_and_new_files_only`, `the_listing_is_path_nul_digest_lines_in_byte_order`, `the_real_tree_is_the_one_this_binary_was_built_from` |
| `hashgate::{run, report, print_arms, record, collect_arms}` | cli `hashgate_passes_and_its_negative_control_goes_red` (digest and `DIVERGED` lines, both records), `a_failed_wasm_build_is_could_not_run_and_seed_counts_are_capped` |
| `hashgate::{arm, stage_bytes}` | cli `hashgate_arm_prints_one_line_per_stage_and_seed`, `stage_bytes_knows_both_stages_and_refuses_others` |
| `hashgate::{reference_degree, parse_reference_degree}` | `the_mutation_variable_parses_strictly`; cli (`=9` → 1, `=nine` → 2) |
| `hashgate::compare::{diverged, per_stage, digest, well_formed, Tally}` | `agreeing_arms_have_no_divergence`, `one_arm_differing_on_one_line_names_that_line`, `vacuous_comparisons_are_refused`, `a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input`, `per_stage_counts_equal_seeds_per_stage_and_distinct_bad_seeds` |
| `oracle_fixtures::{run, emit, write_cases, write_expected, create, adversarial_pairs}` | `every_case_has_one_expected_line_and_graph_definitions_expect_null`, `emitting_twice_writes_the_same_bytes_and_a_manifest_that_pins_them`, `nothing_to_emit_and_malformed_pairs_are_refused` |
| `oracle_fixtures::remove_stale` | `a_stale_manifest_is_removed_and_only_absence_is_not_an_error` |
| `oracle_fixtures::{diff, diff_code}` | `the_harness_verdict_passes_through_and_everything_else_is_could_not_run`; cli `oracle_diff_passes_on_emitted_fixtures_and_goes_red_on_a_wrong_line` |
| `oracle_fixtures::cases::*` | `the_generator_writes_exactly_the_pinned_cases` (sha256 of every case for seeds 0–20, 100, 200, 300 and 399), `a_third_of_id_coordinates_are_adversarial_and_the_rest_vary`, `neighborhoods_start_mostly_at_a_real_node_and_sometimes_nowhere`, `synthetic_sizes_cover_the_special_inputs_and_three_digested_giants`, `group_cases_have_one_plus_seed_mod_400_sources_five_repeats_and_a_dropped_id`, `seed_0_reaches_all_17_functions_and_the_two_extra_arms` |
| `oracle_fixtures::generate::*`, `pools::*` | the pinned-cases test above (any change to a draw changes the pin) |
| `oracle_fixtures::eval::{Evaluator, eval, canonical, …}` | `canonical_json_is_compact_with_sorted_keys_at_every_depth`, `a_graph_definition_is_null_and_unknown_functions_are_refused`; every expected line of the differential |
| `oracle_fixtures::wire::{hex, unhex, Wire*}` | `hex_is_the_bit_pattern_and_round_trips_every_special_value`; the differential (the harness decodes the same wire) |
| `capabilities::{ledger, problems, run, Status, Capability}` | `every_registered_row_stands_on_honest_evidence_and_reads_it_back`, `empty_required_fields_zero_ceiling_and_duplicate_ids_are_refused`, `a_row_serialises_to_the_section_8_keys_in_order`, `status_serialises_to_the_four_ledger_words`; cli `capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs`, `the_ledger_reads_a_recorded_run_and_names_what_it_lacks` |
| `capabilities::verdict::{load, current, seeds_of, hash_4way, diverged, oracle_diff}` | `without_records_every_gated_row_is_refused_twice`, `a_record_from_another_tree_or_too_few_seeds_is_refused`, `a_failed_run_a_short_stage_or_a_green_control_is_refused`, `a_function_without_cases_or_with_an_unexplained_mismatch_is_refused` |
| `capabilities::registry::{registry, TOPOLOGY, TOPOLOGY_CEILING}` | `the_registry_covers_every_oracle_function_once_its_ids_are_unique` |
| `runner::run_status` (made `pub`) | `a_child_past_its_time_limit_is_killed_and_reported`; cli |
| `harness/oracle-diff.mjs`, `oracle-h9.mjs`, `oracle-wire.mjs` | cli `oracle_diff_passes_on_emitted_fixtures_and_goes_red_on_a_wrong_line` (both corruptions), the negative controls in §3, and the gate row |
| `harness/wasm-run.mjs` `hash` mode | cli `hashgate_*` |

No row reads "none".

### Mutation testing

`cargo mutants --in-diff` over the Phase 1 diff (`git diff 4689e1e HEAD`), in `ge-mutants`
(cargo-mutants 27.1.0), with `test_workspace = true`, so every mutant faces every test in the
workspace, the cli tests included.

| run | tree | mutants | caught | unviable | timeout | missed |
|---|---|---:|---:|---:|---:|---:|
| 1 | before the review fix-up | 663 | 464 | 57 | 2 | 140 |
| 2 | after the review fix-up | 683 | 616 | 59 | 2 | 6 |
| 3 | the 6 misses of run 2 plus 2 siblings, on the final tree (`-F`) | 8 | 8 | 0 | 0 | **0** |

- **Run 1's 140 misses** were mostly in the fixture generator (`generate.rs` 76, `cases.rs` 25,
  `eval.rs` 11): a mutant there changes which cases are written, and nothing checked the cases
  themselves. `the_generator_writes_exactly_the_pinned_cases` now pins the SHA-256 of every case
  for 25 seeds, and the shape tests pin what each function's cases must cover. Of the 18 misses
  in graph-core:
  - 11 are caught in run 2 by new tests (`the_last_u32_is_never_handed_out_as_an_index`, the
    `stage` byte-layout tests, the `hash_string` vectors, the `with_capacity` tests,
    `a_capacity_error_names_what_overflowed`);
  - 1 no longer exists: `code_point_at` was rewritten as `code_points_at`, whose 4 mutants
    are all caught;
  - 4 are excluded as equivalent (below);
  - 2, the struct-field deletions in `index_model`, survived to run 2 (below).
- **Run 2's 6 misses** were all in code the fix-up added or kept: `Stamp::still_current → Ok(())`,
  `print_arms → ()`, `remove_stale → Ok(())` and its guard forced to `false`, and the two
  struct-field deletions in `index_model` below. Each now has a new test, and run 3 shows every
  one of those tests failing against its mutant. The other 122 graph-cli misses of run 1 are all
  caught in run 2.
- **Timeouts (2):** `Incident::next → Some(0)` and `→ Some(1)` make the iterator infinite, so
  the tests hang until killed. A hang is a detected mutant, not a survivor.
- **Unviable (59):** the mutant does not compile (for example a `Default` the type lacks).

**Exceptions**, each with its reason in `.cargo/mutants.toml`:

- `empty_model → Default::default()`: equivalent, since `empty_model` *is* `Topology::default()`.
- `< → <=` in `synthetic_node` and `synthetic_edges`: equivalent on every reachable input.
  mulberry32 draws `k / 2^32`, so a draw never equals 0.6 or 0.8. It equals 0.5 only when
  `k = 2^31`, and none of the first 1 000 000 draws from the synthetic seed does. That is pinned by
  `no_draw_a_synthetic_model_can_reach_is_exactly_one_half`, and the largest model uses fewer
  draws.
- Carried from Phase 0: `exports::publish` (wasm32-only), `from_bits` `|`→`^` and
  `StageCount::get → 1` (equivalent).
- **Withdrawn:** the exclusion for deleting `nodes`/`edges` from `index_model`'s struct literal
  ("equivalent in output"). cargo-mutants 27.1.0 does not apply `exclude_re` to that mutant kind
  (even `-E 'index.rs:47'` leaves it listed), and the reason was weak anyway: the reservation is
  observable. `index_model_reserves_each_column_once_for_its_input` now kills both. **RED:**
  capacities `(8, 8)` against `(5, 5)`, and `(4, 4)` against `(3, 3)`.

## 6. Ponytail markers added

| where | failing input | direction | escape hatch |
|---|---|---|---|
| `graph_core::edgekind::edge_kind_from_type` | `"note_link_hierarchy"`: two markers, and branch order makes it `Hierarchy`. A renamed type such as `"child-of"` becomes `Relation`. | **silent misclassification**, never an error | compare the wire type against the literals at the call site; tightening the function would change host behaviour |
| `graph_core::ids::parse_node_id` (H5, the oracle's own marker at `ids.ts:61-66`) | `make_record_node_id("my:db", "x", "1")` parses back as `{source: "my", databaseId: "db", recordId: "x:1"}` | **shifted, wrong result**, not `None`; worse as data sources broaden | reject `:` in `source` and `databaseId` before building the id |
| `graph_core::ids::hash_string` | an accumulated value of exactly −2³¹ (`"xfjfxtf"`) | returns 2³¹ like the oracle's `Math.abs`; a caller narrowing to `i32` is silently wrong | pinned by a test, and `"xfjfxtf"`/`"xfjfxtfa"` are in the fixture id pool |
| `graph_core::index::Topology::hierarchy` | an A→B `child_of` edge | it lands in row A: the child is read as the parent (**silently inverted tree**) | nothing reads this CSR yet; Phase 3 decides the orientation first (§8) |
| `topology.index` `scale_ceiling` | ids much longer than the synthetic ones; the caller's records | the estimate is **too high** in both cases | re-measure with `crates/graph-core/tests/memory.rs` |
| `graph-cli fingerprint::FINGERPRINTED` | a registry that moves the `debian:trixie-slim` or `node:22-slim` tag | an old record counts as current (**under-reports**, the dangerous direction) | `oracle-diff.json` records Node, ICU and locale; `ge-rust` pins Rust by version |

As the phase requires, there is **no marker on the CSR builder or on the arena**: both are exact.
The `hierarchy()` marker is about which endpoint the caller treats as the parent, not about the
builder.

## 7. What could not be verified

- **UNKNOWN — whether osionos persists edge ids.** H1 changes `makeEdgeId`'s output for
  undirected edges whose endpoints `localeCompare` and UTF-8 bytes order differently. Nothing in
  this repo stores an edge id (`docs/decisions/h1-byte-order.md`, re-verified by grep). The one
  known host, osionos, is not in this container and is read-only, so whether *it* stores edge ids
  has **not** been checked.
- **UNKNOWN — the osionos guard rows.** Unchanged from Phase 0: osionos is not in this container
  and `scripts/osionos-baseline.txt` is still uncaptured. Phase 1 touched nothing in osionos, but
  that is not proven by the guard.
- **PASS only sandbox-adapted — `docker build -t ge-check . && docker run --rm ge-check`.** The
  Dockerfile's `RUN npm ci` cannot receive the proxy's CA. So `node:22-slim` was retagged, for the
  duration of the build only, to a one-layer image that adds `NODE_EXTRA_CA_CERTS` and nothing
  else. It was then restored to its original id (`43ac6c60b8f8`, checked after the build). The
  Dockerfile and the command are unchanged. The literal command, on a host without TLS
  interception, has **not** been run.
- **Not measured — performance.** Phase 1 has no layout compute, so DoD 12's tick budget has
  nothing to measure. Memory is reported as two numbers (§4), measured natively; the wasm32 figure
  is a projection (the `scale_ceiling` Ponytail).
- **Not added — property-based tests.** §6.3 makes them mandatory for code that parses external
  input, which it defines as the ingest contract and the binary deserializer; Phase 1 adds neither.
  `parse_node_id` parses ids, and the 1000-seed differential feeds it generated ids (a third of
  them adversarial), but without shrinking. `proptest` is not on graph-core's dependency list.
- **Architecture fitness functions.** None is defined yet, so there is none to run. This is not a
  pass.
- **Review.** The `reviewer` agent was run once, fresh, over the Phase 1 diff. Its verdict was
  **CHANGES REQUESTED**; §9 shows each finding and its fix. The fix-up itself was not re-reviewed.
  **SKIP — `devil`.** It is not due until before Phase 6.

## 8. Stop-and-ask items

1. **Ratify the deviations in §1.** Most come from the 300-line and 4-parameter house limits. The
   others are the evidence reader and fingerprint that Phase 0's review required, and the two
   phase-text deviations (`weight: f64`; memory at 442 B/node against §5.1's 33 B/node).
2. **Host items** (osionos, on the host that has it):
   - capture the osionos baseline and run the guard rows (Phase 0, item 2, still open);
   - check whether osionos persists `makeEdgeId` output. If it does, the H1 ids of the diverging
     pairs change, and the stored ids need a migration or a decision;
   - run the literal `ge-check` command on a host without TLS interception.
3. **Decide the orientation of `child_of` before Phase 3 reads the hierarchy CSR.** Today an A→B
   `child_of` edge lands in row A, so the child reads as the parent (the Ponytail on
   `Topology::hierarchy`). Phase 3's tree layouts are its first reader.
4. **Documentation corrections** found while reading, not acted on because no envelope covers
   them:
   - **F1–F7** from Phase 0 still stand.
   - **F2, refined.** The Phase 1 reference table sends `hashString` to `model/value.ts`; it is in
     `src/core/math.ts:11`. It was ported from there (§1).
   - **F8.** `src/core/model/edgeKind.ts:23-39` carries a PONYTAIL describing a substring match
     for the tag branch. The code at `:76` compares the exact literals `tagged` and `tag`. The
     marker is stale; the Rust port follows the code, and the differential confirms it.
5. **Memory budget.** 442 B/node is 13× §5.1's 33 B/node. Phase 9 owns the budget; the number is
   measured, not a guess (§4). Most of the gap is the hash indices: 27.5 MB of the 44.2 MB held
   at 100 000 nodes, against 9.6 MB for all the columns and 2.4 MB for the three CSRs.

## 9. Review fix-up

The reviewer's verdict was **CHANGES REQUESTED**. Each finding and what was done:

| # | severity | finding | resolution | evidence |
|---:|---|---|---|---|
| 1 | BLOCKER | No gate evidence for the reviewed tree. | The whole gate was re-run on the final tree, in order, with nothing committed between rows (§3). | §3, §4; the records carry fingerprint `cf0d50c534df…` |
| 2 | MAJOR | The H9 acceptance rule was too wide: any graph-core group ≥ 256 whose low byte matched the oracle's passed. | `harness/oracle-h9.mjs` computes the widened groups (the same loop with neither `Uint8Array` nor mask). A mismatch is H9 only if graph-core's line **is** the widened groups and the oracle's line is exactly them `& 0xff`. | **RED:** under the old rule, the cli test's +256 corruption exited 0. **GREEN:** it exits 1 with a `layoutGroups` MISMATCH. |
| 3 | MAJOR | The ledger accepted a negative control that failed on any stage, not the row's own. | `verdict::diverged(control, stage)`: the control must have `equal[stage] < seeds`. Otherwise the row reports `the negative control did not go red on the <stage> stage`. | `a_failed_run_a_short_stage_or_a_green_control_is_refused` (blind-control cases) |
| 4 | MAJOR | The tree fingerprint missed the toolchain pins. | `FINGERPRINTED` now has 13 entries, including `docker`, `.cargo`, `Dockerfile`, `package-lock.json`, `tsconfig.json` and the test loader. Tags and ICU, which no path can pin, carry a Ponytail. | `the_fingerprint_moves_with_content_names_and_new_files_only` |
| 5 | MAJOR | The fingerprint was taken when the record was written, so a binary built from an older tree could record a result for a newer one. | `build.rs` embeds the tree's fingerprint in the binary. `Stamp::take()` refuses unless the tree still equals it, and `write` refuses if the tree moved during the run. | **RED→GREEN:** a stale binary now exits 2, `rebuild before recording`, for both `hashgate` and `emit-fixtures`; `a_stamp_needs_the_built_tree_and_goes_stale_when_the_tree_moves` |
| 6 | MAJOR | The `scale_ceiling` could not be reproduced. | The probe is committed as `crates/graph-core/tests/memory.rs`. The ceiling is labelled an estimate, with a Ponytail on what it misses. `byte_len` and `strings().len()` feed the table. | §4 memory table, re-run |
| 7 | MAJOR | The deviations were not reported. | §1 lists every one with its cause, including the phase-text deviations. | §1 |
| 8 | MINOR | The transcription guard checked lines, not one contiguous block, and ran over raw nodes. | `checkTranscription` requires the twelve lines as one contiguous block of `layoutBridge.ts`. The groups run over `indexModel`'s nodes, as `rebuild` does, and every group case adds a dropped duplicate id. | **RED:** the raw-node arm gave `[0,0,0,0,0,0,1]` against `[0,0,0,0,0,0]` |
| 9 | MINOR | The adversarial fixture held raw non-ASCII, which an editor could renormalise. | All non-ASCII is now `\u` escapes; the file is pure ASCII and decodes to the same strings. | differential declarations check (20/20) |
| 10 | MINOR | `"xfjfxtf"`, the `i32::MIN` input, was pinned in a unit test but never reached the differential. | `"xfjfxtf"` and `"xfjfxtfa"` are in `ID_POOL`. | `hashString` row in §4 |
| 11 | MINOR | `child_of` orientation in the hierarchy CSR. | Ponytail on `Topology::hierarchy`; decision deferred to Phase 3's first reader (§8 item 3). | §6 |
| 12 | MINOR | The CSR row's evidence gap: the oracle compares only the merged adjacency. | Stated in the `topology.csr` row's ponytail field; the split and the hierarchy CSR rest on unit tests and the 4-way hash. | §2 |
| 13 | MINOR | Functions over 40 lines or over 4 parameters, and too-wide exports. | Split: the registry into a `TOPOLOGY` table; `stage::encode` into node and edge encoders; `cases::model_cases` into `walk` and `equality_cases`; `oracle-diff.mjs` into `graphStore`, `valueFunctions`, `modelFunctions`, `evaluator`, `compare`, `coverageProblems`, `printReport`, `writeRecord` and `main`. `make_edge_id` takes one `EdgeIdParts`. `lib.rs` now exports only what callers use. | a scan of the changed files finds no function over 40 lines or 4 parameters |

**Mutation fix-up.** The re-run over the final diff found six surviving mutants. Four were in the
fix-up's own code (`Stamp::still_current → Ok(())`, `print_arms → ()`, `remove_stale → Ok(())`,
and `remove_stale`'s guard forced to `false`). The other two were the struct-field deletions
whose exclusion cargo-mutants ignored. Each is now killed by a new assertion (§5).
