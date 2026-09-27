# Phase 1 — Topology core, and the 14 oracle functions

**Read `prompt.md` first.** Rules 0.1–0.7 apply. Phase 0's gate must be green, **including its negative
control going red**, before starting.

## Goal

Build the topology layer — dense indices, string arena, three CSR adjacencies, SoA attribute columns —
and port the 14 portable pure functions from the TypeScript oracle, proving byte-equality against it.

No layout in this phase. No geometry. Topology and attributes only.

## Why this phase exists

`src/core/model/**` is **already layout-agnostic** — it never touches x/y, velocity or ticks. That makes
it the cleanest possible first port: the TypeScript is a working oracle, the surface is closed, and every
function is pure. If the differential harness cannot prove equality on *this* — the easy case — it will
never prove anything about a force simulation.

This phase also fixes **H1**, the one hazard already measured to be real.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/{arena.rs,ids.rs,index.rs,csr.rs,columns.rs,weights.rs,diff.rs,neighborhood.rs,edgekind.rs}
crates/graph-core/src/legend.rs          (counts ONLY — see step 6)
crates/graph-cli/src/oracle_fixtures.rs
harness/oracle-diff.mjs
fixtures/adversarial-ids.json            (committed, hand-written — see step 5)
docs/decisions/h1-byte-order.md
```

**MODIFY — exactly these:**
```
crates/graph-core/src/lib.rs             (module wiring + public API)
crates/graph-cli/src/main.rs             (add the `emit-fixtures` and `oracle-diff` subcommands)
crates/graph-cli/src/capabilities.rs     (register the topology capabilities)
package.json                             (implement the `oracle:diff` script)
```

**FORBIDDEN:** any file under `src/` (the oracle is read-only — changing it to make the port match is
the one way to make this whole phase meaningless), `verify/`, `tests/`. Any layout algorithm. Anything
in osionos.

## Reference material

Read these from the oracle before porting each function:

| Port target | Oracle |
|---|---|
| ids, `makeEdgeId`, `parseNodeId` | `src/core/model/ids.ts` (read its PONYTAIL at `:61-66`) |
| `indexModel`, `nodesEqual` | `src/core/model/model.ts` (note `:35` first-wins dedupe, `:41-42` Map-ordered adjacency) |
| `applyDegreeWeights` | `src/core/model/weights.ts:12-29` |
| `diffGraph`, `edgesEqual`, `isEmptyPatch` | `src/core/model/diff.ts` |
| `neighborhood`, `neighborhoodEdges` | `src/core/model/neighborhood.ts` |
| `edgeKindFromType` | `src/core/model/edgeKind.ts:41-79` (an ordering-dependent substring classifier — read its existing marker) |
| `deriveLegend` | `src/core/model/legend.ts` — **partially**, see step 6 |
| `hashString` | `src/core/model/value.ts` — **H4** |

## Steps

### 1. String arena and interning

One `Vec<u8>` plus `Vec<(u32,u32)>` offsets (**`u32`, not `usize`** — D6). Interning collapses repeated
group/kind/label values. Return opaque `Interned(u32)` handles; never leak the arena's indices to the
wire.

### 2. Identity — dense index ↔ stable id

`IndexMap<Interned, u32>`. **`IndexMap`, not `HashMap`** — the oracle de-dupes first-wins in JS `Map`
insertion order (`model.ts:35`), and `HashMap` iteration would be non-deterministic (**H2**, **D4**).

The dense index is **internal only**. The stable string id is the only identity that crosses the wire.
This is not stylistic: it is what keeps layout positions stable across rebuilds, and it fixes the
name-keyed-vs-index-keyed split that makes SciGraphs' own two export formats unjoinable
(`prompts/REFERENCES.md`).

### 3. Three CSR adjacencies

`offsets: Vec<u32>` (n+1) and `targets: Vec<u32>` (m), built in **O(n+m)** by counting sort. Three of
them: **out**, **in**, and **parent→child** (the hierarchy). The hierarchy CSR is unused until Phase 3 —
build it now anyway, because it is the same code and retrofitting it later means touching the index
builder after things depend on it.

Preserve the oracle's adjacency **ordering** (`model.ts:41-42` builds it in Map order). A CSR sorted by
target id would be more cache-friendly and **would break the differential**. Match first; optimize only
with a measurement and a recorded decision.

### 4. SoA columns

One typed column per attribute, indexed by dense index: `degree: Vec<u32>`, `weight: Vec<f32>`,
`group: Vec<u32>`, `kind: Vec<u8>`, plus interned label/id handles. Pre-size with `Vec::with_capacity` —
growing by one in a hot loop is a defect (`dsa-and-memory.md`).

### 5. Fix H1 — and prove it with a fixture that can see it

`makeEdgeId` (`ids.ts:87`) sorts undirected endpoints with `localeCompare`. **Byte order (`str::cmp`) is
the contract from now on.** Measured divergence, 3 of 7 adversarial pairs (`prompt.md` §7.3).

Write `fixtures/adversarial-ids.json` **by hand**. It must contain id pairs where `localeCompare` and
byte order disagree — at minimum mixed case (`"a"`/`"A"`), the `Z`-vs-`a` boundary, and a realistic
prefixed pair (`"note:1"`/`"NOTE:1"`). **Do not generate this fixture from `buildSyntheticModel`**: that
function builds ids as literal template strings and never calls `makeEdgeId`, so nothing derived from it
can exercise this at all. That blindness is precisely why the fixture is hand-written and committed.

Write `docs/decisions/h1-byte-order.md`: the measurement, the decision, and the blast-radius check —
`EdgeId` appears only as in-memory `Map`/`Set` keys (`model.ts:41-42`, `neighborhood.ts:23,64`) and is
never persisted, so there is no stored data to migrate. **Re-verify that claim yourself** with a grep
before writing it down.

The differential for `makeEdgeId` will now **intentionally disagree** with the oracle on those pairs.
That is the point. Encode the expected disagreement explicitly as a known-divergence list with the
reason, so the harness proves "differs *only* here" rather than "differs somewhere".

### 6. `legend.rs` — counts only

`legend.ts:8` imports `databaseColor` and emits OKLCH. It is the one file in `core/model/` that must
**not** move wholesale (**H7**). Port the **counting and bucketing** to Rust; **colour stays in
TypeScript**. If you find yourself needing a colour value in Rust, stop — the split is wrong.

### 7. H4 — `hashString`

`Math.imul` → `wrapping_mul` on `i32`. `Math.abs` → note that `Math.abs(i32::MIN)` overflows in Rust;
the oracle's JS silently produces a float. Match the oracle's **observable output**, and document the
edge case with a test that pins `i32::MIN` behaviour explicitly.

### 8. H9 — the 256-group truncation

`layoutBridge.ts:86` does `nodeGroups[i] = g & 0xff`, so group 256 aliases group 0. Do **not** replicate
the truncation. Use `u32` and record the deviation in `docs/decisions/` — it is a bug fix, and a
differential arm must therefore expect divergence above 255 groups. Add a fixture that crosses 255.

### 9. The differential harness

`graph-cli emit-fixtures --seeds N` writes canonical inputs; `harness/oracle-diff.mjs` loads them, runs
the TypeScript oracle, and byte-compares. Seeds must be deterministic and the generator recorded.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                       # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                  # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown # 0

# 4-way hash equality on the topology stage
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000           # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                # NON-ZERO

# oracle differential, incl. the adversarial fixture and the declared known-divergences
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- emit-fixtures --seeds 1000      # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim npm run oracle:diff                            # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check            # 0
docker build -t ge-check . && docker run --rm ge-check                                         # 0
```

## Ledger delta

Register and drive to `gated`: `topology.index`, `topology.csr`, `topology.weights`,
`topology.diff`, `topology.neighborhood`, `topology.edgekind`, `topology.legend_counts`,
`topology.ids`.

Each needs its `scale_ceiling`, `degradation` and `ponytail` fields populated — they are non-optional.
`topology.index`'s ceiling is a real number: state the node count at which the arena or the `u32`
index space becomes the binding constraint, and how it degrades.

## Ponytail requirements

- `edgeKindFromType` — ordering-dependent substring classifier, silently degrades unknown input to
  `"relation"`. Name the failing input concretely (a type string containing two matching substrings,
  where order decides) and the direction (silent misclassification, not an error).
- `parseNodeId` — carry forward the oracle's own PONYTAIL (`ids.ts:61-66`): `source`/`databaseId`
  containing `:` cannot be represented, and the parse returns a **shifted, wrong** result rather than
  `null`. This gets worse as data sources broaden (**H5**).
- `hashString` — the `i32::MIN` edge case.
- **No marker** on the CSR builder or the arena. They are exact.

## Stop-and-ask

- A differential mismatch you cannot explain → **stop**. Do not add it to the known-divergence list to
  make the gate green. The list is for *decided* deviations with a written reason, not for unexplained
  ones. This is the single most likely way this phase silently fails.
- Matching the oracle appears to require non-determinism (e.g. real `HashMap` ordering) → stop; the
  oracle's behaviour needs a recorded decision, not a non-deterministic port.
- You want to change anything under `src/` → stop. The oracle is read-only.
