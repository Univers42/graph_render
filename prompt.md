# graph-motor — the master runbook

**What you are building:** a pure-Rust motor that computes graph and diagram geometry and emits
numbers any application can interpret. Not an application. Not a renderer. A motor with an API, an
SDK, and a contract.

**Why:** the reference design is SciGraphs (`/home/dlesieur/Documents/SciGraphs`), a Blender extension
whose architecture is right — `core/` pure, `engine/` compute, the Blender addon a disposable front —
but whose substrate is Python, and Python cannot iterate graph analysis in real time. Keep the design,
replace the substrate.

**Read these before touching anything:** `prompts/REFERENCES.md` (where the math actually lives, and
which references are not on disk), then the phase file you are on.

---

## 0. Rules that override everything below

**0.1 — `/home/dlesieur/Documents/osionos` is READ ONLY.** Read it freely; never create, modify or
delete a single byte in it. There is no task in this project that requires writing to osionos.

Enforced by **`scripts/guard-osionos.sh`** — in *this* repo, because a guard written into the tree it
protects would violate the rule on its first commit. **Phase 0 creates it** (see
`prompts/phase-00-foundation.md` step 9); until then the rule is discipline only, which is exactly why it
gets an owner in the first phase. Two things it must get right, both learned from the throwaway version
it replaces: osionos is **already dirty**, so it compares against a committed baseline rather than
demanding a clean tree; and it **hashes contents** rather than reading `git status --porcelain`, which
cannot see a second edit to an already-dirty file — the blind spot that would cover nearly every file at
risk here. Exit code **90** means the invariant broke, distinct from 1 meaning the command failed.

**0.2 — Docker only. Nothing is installed on the host.** There is no host `node`, no host `cargo`, no
host `rustup`. Every command runs in a container. If you catch yourself typing a bare `cargo` or `npm`,
stop — it will either fail or silently use something that is not the pinned toolchain.

**0.3 — Prebuilt vendor language images are prohibited.** No `FROM rust:*`, no `FROM
rustlang/*`, no `mcr.microsoft.com/playwright`. A minimal OS base (`debian:trixie-slim`) plus exactly
what we install, pinned. Bootstrapping a compiler from source is *not* what this means.

**0.4 — Stay inside your phase's authorization envelope.** Each `prompts/phase-NN-*.md` lists the exact
paths it may CREATE and the exact paths it may MODIFY. Reading ahead is encouraged. *Acting* ahead is a
stop-and-ask. Anything not on the list → stop and say so; do not improvise scope.

**0.5 — Never claim a result you did not run.** Every factual statement in a report must be the output
of a command re-run at report time. Not remembered, not from an earlier report, not inferred. This
project has already produced three false claims from restated stale numbers — the guard is aimed at
everyone, including whoever wrote this file.

**0.6 — A missing reference is a stop, not an improvisation.** If a phase needs an algorithm whose
reference implementation is not on disk (`prompts/REFERENCES.md` says which), stop and ask for it.
Writing the algorithm from memory of a paper is the one failure this project cannot absorb.

**0.7 — No auto-push, no commits to `main`/`develop`.** Commit message is exactly `updated`. No
`Co-Authored-By`, no "Generated with" trailer. Branch from `master` here.

---

## 1. Toolchain — every command, verbatim

One image, built by us:

```sh
docker build -f docker/rust.Dockerfile -t ge-rust .
```

Then everything runs through it:

```sh
# build / test / gate  (workspace is mounted at /w)
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown
docker run --rm -v "$PWD:/w" ge-rust cargo test  --workspace
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check

# the TypeScript oracle + the wasm hash arm  (reuses the existing node image)
docker run --rm -v "$PWD:/w" -w /w node:22-slim npm run oracle:diff
```

**`oracle:diff` does not exist yet.** `package.json` currently defines only `typecheck`, `test`, `lint`
and `check`. Phase 0 adds the script as a stub and Phase 1 implements it — so treat it as a command you
*create*, not one you inherit. Every gate listing below that invokes it is valid only from the phase that
creates it onward.

The existing TypeScript gate is unchanged and must stay green:

```sh
docker build -t ge-check . && docker run --rm ge-check
```

**`ge-rig` and `ge-parity-rig` are kept as-is: not deleted, not rebuilt, not extended.** An earlier
revision of this file said delete them for ~4 GB; measured, that frees 4.5 MB (0%). See §9 for the
measurement and for why they are a *documented* exception to rule 0.3 rather than a contradiction of it.

---

## 2. Ground truth — facts, so you do not re-derive them

### 2.1 What exists here

`/home/dlesieur/Documents/graph-engine`, branch **`master`**. A force-directed knowledge-graph engine:
Canvas2D + Web-Worker d3-force, TypeScript, 74 files. From now on **this TypeScript is the differential
oracle** — test infrastructure, no longer shipped code. It is never deleted.

`src/core/model/**` is **already layout-agnostic** — it never touches x/y, velocity or ticks. That is
why this port is feasible: the topology layer generalizes nearly for free.

### 2.2 The three specializations that block reuse

1. **One layout, hard-wired.** `layoutEngine.ts:9,54,94` types the engine directly to `ForceLayout`;
   there is no injected interface. The postMessage protocol (`layoutEngine.ts:33-48`) bakes in
   force-only verbs (`pin`/`unpin`/`reheat`/alpha) and assumes a continuous `setInterval` tick loop
   (`layoutEngine.ts:112-140`). A one-shot layout cannot ride it.
2. **The output vocabulary cannot express other families.** The de-facto contract is *one `(x,y)` plus
   one scalar `radius` per node, and implicit straight edges* — `sceneState.ts:33-63`, confirmed in the
   draw code: `links.ts:52-53` does `moveTo/lineTo`, `exportSvg.ts:32-38` emits a single `<line>`. No
   width/height. No edge waypoints.
3. **Knowledge-graph semantics are in the core.** `NodeKind = record|note|database|tag`
   (`types.ts:20`), `EdgeKind = relation|tag|note_of|note_link|hierarchy` (`types.ts:23`), tag-hub
   synthesis (`ids.ts:16,21`), a "tags" legend bucket (`legend.ts:35-43`), `GraphStats.notes`
   (`model.ts:55`), kind→shape (`nodeShape.ts:2-4,27-30`), kind→colour (`colors.ts:19-28`), hard-coded
   edge buckets 0-3 (`sceneEdges.ts:11,96-101`). All of this moves behind declared roles.

### 2.3 Cross-language hazards — verified against source, each one silently wrong in a naive port

| # | Hazard | Evidence | Why it bites |
|---|---|---|---|
| H1 | `makeEdgeId` sorts undirected endpoints with **`localeCompare`** | `ids.ts:87` | **Measured: 3 of 7 adversarial pairs diverge from byte order** (§6.3). Different sort → different edge id → different dedupe → different graph. |
| H2 | JS `Map` iteration is insertion-ordered; `indexModel` de-dupes first-wins in that order | `model.ts:35,41-42` | a `HashMap` port is non-deterministic → **D4** |
| H3 | `weight = clamp(0.2 + 0.8·log1p(deg)/log1p(8), 0.2, 1)` | `weights.ts:12,24` (`REFERENCE_DEGREE = 8`) | `Math.log1p` vs Rust `ln_1p` may differ by 1 ULP → **D1**. **Exactly 9 distinct weights are reachable**: `deg=0 → 0.2`, `deg=1..7 → 7 intermediate values`, and `deg=8 → exactly 1.0` *by the formula* (`0.2 + 0.8·1`), with `deg>8` clamped to the same `1.0`. Enumerate **9**, not 8 — this count scopes the bit-comparison test, so an off-by-one leaves a value untested. |
| H4 | `hashString` uses `Math.imul` then `Math.abs` | **`src/core/math.ts:11`** | needs `wrapping_mul` on `i32`; `Math.abs(i32::MIN)` overflows. (An earlier revision cited `model/value.ts`, which does not exist — see §14 in `prompts/ONBOARDING.md`.) |
| H5 | Node-id grammar cannot represent `:` in `source`/`databaseId` | `ids.ts:61-66` (its own PONYTAIL) | "any data source" makes this **worse** — arbitrary sources contain `:` |
| H6 | `groupValue` depends on `Object.values()` ordering | `deriveGraph.ts:92-98` | non-deterministic across JSON reserializations; declared roles fix it |
| H7 | `legend.ts` is **not** pure — imports `databaseColor`, emits OKLCH | `legend.ts:8` | the one file in `core/model/` that must not move wholesale: **counts to Rust, colour stays in TS** |
| H8 | Standalone ESLint firewall bans React but **dropped** the host's `@/*` ban | `eslint.config.js:16-17` | nothing currently stops a host import or a `fetch` in `core/` |
| H9 | `nodeGroups[i] = g & 0xff` silently truncates at 256 groups | `layoutBridge.ts:86` | group 256 aliases group 0 |

`d3-force` is the **only** third-party import in all of `core/` (`forceLayout.ts:9-26`) — porting layout
removes core's last dependency.

---

## 3. Architecture — a staged pipeline, not a layout function

SciGraphs confirms this shape: layout and edge-routing are architecturally decoupled stages there, not
one seam (layout returns positions; `mesh/edge_styles.py` and `engine/scigraphs_engine/bundling/*` are
separate downstream passes).

```
INGEST      collections + records + declared field ROLES   (adapters are dumb mappers)
   │
TOPOLOGY    dense index ↔ stable id · CSR adjacency · SoA attribute columns
   │        derived: degree, weight, component
   │
ANALYSIS    optional: communities, centrality, shortest paths, hierarchy depth
   │
LAYOUT      the pluggable seam:  topology → geometry
   │
POST        optional: edge routing, edge bundling, edge styles
   │
SCALE       optional: LOD, simplification, culling hints
   │
GEOMETRY    the universal output — two faces, one meaning
```

Every stage is pure, ordered, optional, and **independently hashable**. That last property is the point:
the gate emits **one hash per stage**, so a cross-target divergence *names the failing stage* instead of
reporting "the final numbers differ."

Only LAYOUT varies per diagram family. Everything above is shared; everything below is a fixed
vocabulary.

### 3.1 Crates

```
crates/
  graph-contract/   types + codegen. THE single source of truth for the wire format.
  graph-core/       the motor. PURE: no I/O, no async, no wasm-bindgen, no HTTP, no DOM, no time.
  graph-wasm/       thin extern "C" glue. Browser + the Node hash harness.
  graph-sdk-js/     the published JS/TS SDK over graph-wasm.
  graph-cli/        hash gate, capabilities ledger, fixtures, benchmarks, differential driver.
```

`graph-core`'s purity is a **gate, not a guideline**: it compiles for `wasm32-unknown-unknown` *and*
native in every phase, and its dependency allow-list is closed — `libm`, `indexmap`, `petgraph`. Adding
to that list is a stop-and-ask.

### 3.2 No wasm-bindgen — deliberate

The house precedent (`formula-engine`, in the osionos submodule) uses wasm-bindgen + `wasm-pack`. **We
copy its loading pattern and reject its binding layer.** Reason: formula-engine passes strings and JSON,
where bindgen earns its keep; this motor passes numeric buffers. Raw `wasm32-unknown-unknown` with
`extern "C"` exports plus `WebAssembly.instantiate` costs ~40 lines of Rust and ~30 of TS, removes a
build tool and a version-pinned codegen step, produces a smaller artifact with no generated JS on the
hot path, and makes the Node hash harness trivial.

Copy verbatim from `formula-engine/bridge.ts:63-86`: the lazy deduped singleton, the `initFailed` flag,
the `__*_DISABLE_WASM__` kill switch, and graceful degradation (warn and fall back, never throw).

---

## 4. The geometry vocabulary — get this right now or pay for it forever

This is the breaking surface. Ship `{x,y}` only and every non-force family forces a wire-format change
later, invalidating stored snapshots and every frontend.

**SciGraphs proves this failure rather than hypothesising it.** Its layout contract is a bare
`(num_nodes, 3)` array, validated as exactly that (`common.py:175-185`). Of ~30 algorithms, exactly one
needs more — Circle Packing returns radii — and with no room in the contract they are smuggled
out-of-band into a Blender mesh attribute (`circle_packing.py:281`) that no other consumer reads.

A **closed, versioned set**. The discriminant is **per-snapshot, not per-element**, so SoA columns stay
pure and the cost is one byte for the whole payload:

| Node kind | Columns | Serves |
|---|---|---|
| `Point` | `x[]`, `y[]`, `(z[])` | force, spectral, MDS, circular, grid, geographic |
| `Circle` | `x[]`, `y[]`, `r[]` | circle packing — **the case SciGraphs could not express** |
| `Box` | `x[]`, `y[]`, `w[]`, `h[]` | treemap, matrix cell, layered node, Gantt bar |

| Edge kind | Columns | Serves |
|---|---|---|
| `Line` | none — endpoints derived from node geometry | force (today's behaviour, **zero bytes**) |
| `Polyline` | `offsets[]`, `pts[]` (CSR-shaped) | Sugiyama bends, orthogonal routing, bundled paths |
| `Curve` | `offsets[]`, `pts[]`, degree tag | bezier/quadratic styles, FDEB output |

**Reserved, tagged, deliberately unimplemented:** `Ribbon` (Sankey), `Arc` (chord). Tag numbers
allocated now so adding them is additive — *reserve the tag, not the implementation*. Also reserved: a
**format version** a reader refuses when the major exceeds what it knows (copied from
`graph_animation.py:264-280`), a **`z` channel**, and a **stage count** so one snapshot can carry
several named geometries for one topology.

**Identity:** the stable string id is the only thing that crosses the wire; the dense index never leaves
the motor. This fixes a real defect in the reference — `.sgraphs` is name-keyed while
`export_positions` (`export_utils.py:177-198`) is index-keyed with no names and no edges, so two
artifacts from one system cannot be joined.

### 4.1 Two faces, never conflated

- **Semantic face** — canonical JSON: schema'd, versioned, sorted keys. Ingest, API, fixtures,
  debugging, third-party fronts. **This is the universal contract the project exists to deliver.**
- **Transport face** — columnar typed arrays viewed directly over WASM linear memory. The render hot
  path. **Zero serialization.**

The hash is taken over the **binary** face (unambiguous byte order); the JSON face must round-trip to
the identical binary. `f32 → shortest-round-trip decimal → f64 → f32` is **exact**, not lossy: Rust's
float `Display` emits the shortest decimal that round-trips (≤9 significant digits for f32), f64 carries
15+, so double rounding through `JSON.parse` cannot lose a bit and `Math.fround` recovers the original.

---

## 5. Data structures — chosen before the code, not discovered after

| Concern | Structure | Why | At 100k nodes / 300k edges |
|---|---|---|---|
| Adjacency (out, in, **and** parent→child) | **CSR**: `offsets: Vec<u32>` (n+1), `targets: Vec<u32>` (m) | O(1) neighbour range, cache-linear traversal; every layout family needs neighbour iteration | ~1.6 MB per direction; 3 CSRs ≈ **4.8 MB** vs ~30 MB for adjacency-lists-of-objects |
| Attributes | **SoA typed columns** by dense index | cache-friendly, no per-node header, **and it is already the transport format** | **33 B/node** (§5.1) vs 200–400 B for a JS object graph |
| Identity | `IndexMap<interned, u32>` + string arena | insertion-ordered → deterministic iteration (**H2**) | — |
| Strings | one `Vec<u8>` arena + `(offset,len)`, interned | labels dominate graph memory | — |
| Spatial (Barnes-Hut, hit-test, grid routing) | quadtree / uniform grid over the dense arrays, into a **reused** buffer | no per-tick allocation in the layout loop | — |
| Ordering | stable `sort_by` on a **total** key only | `sort_unstable` leaves equal elements unspecified | — |

Complexities to state and hold: index build **O(n+m)** · force tick **O(n log n + m)** (Barnes-Hut) ·
tidy tree **O(n)** · squarified treemap **O(n log n)** · Sugiyama **O(n+m)** per phase (crossing
reduction is heuristic) · Dijkstra **O((n+m) log n)** · **Pivot MDS O(k(n+m)) time, O(nk) memory** ·
dense symmetric eigendecomposition **O(n³)**, which is why `_DENSE_EIG_LIMIT = 256` exists.

**Honesty requirement.** d3's `forceManyBody` *already* uses a quadtree, so a Rust Barnes-Hut port is
**not an asymptotic win** — it is a constant-factor win (SoA memory, no GC, no JS object overhead). The
same applies to the Python→Rust claim overall: the real win is removing the interpreter from the
iteration loop, and it must be **measured**, never asserted. No O-notation argument substitutes for a
number.

### 5.1 The memory budget, with its arithmetic

Phase 9 gates peak memory against this number, so it has to be re-derivable. A budget you cannot
re-derive is not a budget.

| Column | Type | Bytes |
|---|---|---|
| `x`, `y` | `f32` ×2 | 8 |
| `weight` | `f32` | 4 |
| `degree` | `u32` | 4 |
| `component` | `u32` | 4 |
| `group` (interned id) | `u32` | 4 |
| `label` slice into the arena | `(u32, u32)` | 8 |
| `kind` | `u8` | 1 |
| **total** | | **33 B/node** |

**Radius is derived from `weight` at render time, not stored** — that is why the total is 33 and not 37,
and it is the one row a reader would otherwise reconstruct wrongly. At 100k nodes: **3.3 MB** of columns
+ **4.8 MB** of CSR = **8.1 MB** of topology and attributes.

**What the 33 B excludes, and it matters:** the string arena itself. A label is a `(offset, len)` slice
here; the bytes live in the arena and are **data-dependent and unbounded** — 100k nodes with 40-char
labels adds ~4 MB, more than everything above it combined. So Phase 9 reports **two** numbers, columns
and arena, never one total. A single figure would make a fixed cost look variable and hide which half
grew.

### 5.2 The performance budget — derived from the frame budget

`scale_ceiling` in the ledger is a **node count**: it says where an algorithm stops being usable, never
how fast it is at any N. So without this section the premise *"Python is too slow, Rust will be fast
enough"* has no target it can be checked against, and Phase 9 would gate against nothing.

| Budget | Number | Derivation |
|---|---|---|
| One frame | **16.67 ms** | 60 FPS |
| Ticks to settle | **112** | `src/core/layout/forceLayout.ts:150` sets `alphaDecay(0.06)`; d3's default `alphaMin` is `0.001`, so `0.94^k < 0.001` → k = 112 |
| Tick budget, 60 FPS interactive | **≤ 16.67 ms** | one tick per frame |
| Tick budget, ≤ 2 s settle | **≤ 17.9 ms** | 2000 / 112 |

The two budgets landing on the same figure is a coincidence, but a usable one: **one tick under ~16.7 ms
buys both interactivity and a sub-2-second settle.** That is the single number to hold.

**The deliverable is a crossover N, not a pass/fail.** For each arm — native, wasm32, and the TypeScript
oracle — report the largest N whose tick fits 16.67 ms. Three numbers, and the ratio between them *is*
the project's justification, expressed in the units the premise was stated in. A single pass/fail would
hide both the win and the regression at small N. Per `minimalism-ladder.md`, under 3% is noise and is
reported as noise.

**Measure the oracle's compute, not its rendering.** The baseline is
`src/core/layout/forceLayout.ts:163` `tick()`. Its own header states it is *"DOM-free … driven by manual
`tick()` calls … never touches React or the canvas"*, and the constructor `.stop()`s the simulation
(`:150`), so it can be driven from `node:22-slim` in a plain loop — no page, no worker, no renderer.
Same d3-force, same parameters, same graph: an apples-to-apples compute comparison.

**Do not use `osionos/scripts/graph-bench.mjs`.** An earlier revision named it as the baseline. It waits
`page.waitForTimeout(12000)` for worker layout to *finish* (`:83`), then drags (`:97`) and scrolls
(`:112`) and reports `fps` (`:51`). That is **Canvas2D pan/zoom draw cost, measured after layout is
already done** — the one component a motor does not replace. Gating the project's justification on it
would produce a number that could not move whatever we built.

---

## 6. Determinism — the engineering behind "bit-identical"

WASM mandates IEEE-754 for `+ - * / sqrt` and native x86-64 SSE2 matches, so basic arithmetic is
bit-identical for free. Nine things break it. All nine are constraints on our code.

| # | Constraint | What breaks without it |
|---|---|---|
| D1 | **`libm` crate for every transcendental** (`ln_1p`, `ln`, `exp`, `sin`, `cos`, `pow`, `atan2`) on *both* targets | `std` uses glibc natively and a bundled libm on wasm32 — **they differ**. The most likely single cause of divergence. |
| D2 | Ban `f32/f64::mul_add`; no FMA contraction | one rounding vs two; wasm has no FMA instruction |
| D3 | No parallel float reduction (no rayon sums); fixed iteration order | float addition is not associative |
| D4 | `IndexMap`/`BTreeMap` only where iteration is observable — never `std::HashMap` | random seed per process |
| D5 | Stable `sort_by` on total keys only | equal-element order unspecified |
| D6 | **Every wire integer explicitly `u32`/`u64` — never `usize`** | wasm32 is 32-bit, native is 64-bit |
| D7 | Output hash is an explicit algorithm (BLAKE3 or SHA-256), never `DefaultHasher` | not stable across versions or platforms |
| D8 | No wall-clock in the motor — tick counts only; time is an input | — |
| D9 | Assert no NaN/Inf before hashing | WASM does not mandate NaN bit patterns |

D6 and D7 fail *silently and only across targets* — the worst failure mode available.

**D1 is unmeasured.** Phase 0 measures it (`ln_1p` over a fixed sweep, native vs wasm32, bit-compared)
rather than trusting this file. The constraint stands either way as cheap insurance, but the recorded
reason must be the measurement.

**Determinism beyond floats — inherited from the reference, not invented here.** Eigen-based layouts
need a *fixed* start vector, eigenvector **sign pinning**, and residual-verified convergence; RNG
seeding must also seed any library that ignores your seed. SciGraphs already solved all of this and
`prompts/REFERENCES.md` §"determinism engineering to carry forward verbatim" lists each piece with its
line number and its reason. Do not re-derive it.

---

## 7. The gate

### 7.1 Primary — snapshot hash. Four artifacts per seed, per stage.

```
native  run 1 ─┐
native  run 2 ─┼── all four hashes MUST be equal   (cross-target + run-to-run, one check)
wasm32  run 1 ─┤
wasm32  run 2 ─┘
```

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000   # expect 0
```

The wasm arm runs **under Node** — already in the manifest, already an image we have — driving the same
artifact the browser loads. No wasmtime, no new runtime, and it tests the *real* shipped binary.

This is primary, not pixel parity, because it sits **inherently on the changed surface**, is
renderer-independent (so it survives the front being replaced, which is the premise), needs no browser,
and covers the id/ordering/float hazards directly.

### 7.2 The negative control — non-negotiable

```sh
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8        # expect NON-ZERO
```

A gate that cannot fail is not a gate. This proves it is wired to something.

### 7.3 The adversarial fixture — what finally reaches H1

`buildSyntheticModel` is exported and is the obvious fixture source, but it **builds ids as literal
template strings and never calls `makeEdgeId`** — so no fixture derived from it can exercise the
riskiest cross-language function. The existing unit test (`tests/graph-engine.test.ts:61-64`) uses ids
`"a"` and `"b"`, where `localeCompare` and byte order **agree**. Both the pixel rig and the unit test are
blind to H1 by construction.

Measured in `node:22-slim`:

```
DIVERGE ["a","A"]            localeCompare=-1  byteOrder= 1
DIVERGE ["Z","a"]            localeCompare= 1  byteOrder=-1
DIVERGE ["note:1","NOTE:1"]  localeCompare=-1  byteOrder= 1
agree   ["_x","ax"] ["é","e"] ["10","9"] ["a-b","ab"]
```

So a dedicated mixed-case fixture is required, and **byte order (`str::cmp`) is the contract** —
`localeCompare` is the defect, not the spec. Safe to change: `EdgeId` appears only as in-memory
`Map`/`Set` keys (`model.ts:41-42`, `neighborhood.ts:23,64`) and is never persisted, so there is no
stored-data blast radius.

### 7.4 Secondary — differential against the TypeScript oracle

These **17** portable pure exports from `src/index.ts` must match byte-for-byte over ≥1000 seeded inputs:
`indexModel`, `emptyModel`, `nodesEqual`, `makeRecordNodeId`, `makeNoteNodeId`, `makeTagNodeId`,
`makeEdgeId`, `parseNodeId`, `applyDegreeWeights`, `edgeKindFromType`, `diffGraph`, `isEmptyPatch`,
`edgesEqual`, `deriveLegend`, `neighborhood`, `neighborhoodEdges`, `buildSyntheticModel`.

**`buildSyntheticModel` is on this list deliberately, and it is the subtle one.** It generates the
benchmark and differential inputs, so if a Rust reimplementation of it drifts from the TypeScript, the two
arms compare *different graphs* and the differential reports green while proving nothing. Two defences,
both required: it is byte-compared like any other function, **and** fixtures are treated as **data, not
code** — `graph-cli emit-fixtures` writes them once and *both* arms load the same file, so there is no
second generator to drift.

Each layout additionally differentials against its JS oracle. **Verified present in the lockfile, not
assumed:** `mermaid@11.15.0` transitively resolves `d3-hierarchy@3.1.2` (tree/cluster/treemap/pack/
partition), `dagre-d3-es@7.0.14` (layered DAG), `d3-sankey@0.12.3`, `d3-chord@3.0.1`, plus `cytoscape` +
`cose-bilkent`/`fcose`. Standalone `dagre` and `elkjs` are **absent**. Promote what you need to a direct
devDependency of the oracle harness only — oracle code, never shipped.

### 7.5 What the demoted pixel rig cannot see

Kept only as a migration safety net, and **its report must say this**: it is blind to fallback colour
values (the rig injects real tokens, making every fallback dead code), to anything `synthetic.ts` does
not call, and to anything measured on the frozen parity page (`verify/parity/main.tsx:158` calls
`engine.freeze()`, so FPS and settle-time read there are meaningless-but-excellent).

---

## 8. Orchestration — the ledger is generated, never hand-written

A hand-maintained `PROGRESS.md` **will** rot; that is the exact defect class this project has already
hit three times. So progress is derived:

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --json
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check   # expect 0
```

One row per capability, from the registry plus the last recorded gate run:

```json
{ "id": "layout.treemap.squarified", "tier": 1, "stage": "layout",
  "geometry": "Box", "status": "gated",
  "oracle": "d3-hierarchy@3.1.2", "oracle_diff": "byte-equal/1000 seeds",
  "hash_4way": "equal", "scale_ceiling": 200000, "degradation": "none",
  "ponytail": "areas <=0 are clamped to 1e-6; a zero-area subtree collapses to a hairline",
  "complexity": "O(n log n)" }
```

`status` is one of `absent | stub | implemented | gated`. A row may print **`gated` only** if its 4-way
hash and its oracle diff both passed in the current tree; `capabilities --check` exits non-zero
otherwise, and it runs in every phase gate. **Every phase report is the diff of this ledger**, not prose.

`scale_ceiling`, `degradation` and `ponytail` are **required fields**. A capability without them cannot
reach `gated`. That is deliberate: SciGraphs documents its own ceilings honestly (per-algorithm
`_SLOW_ABOVE` thresholds at `igraph_layouts.py:13-16`; a Sugiyama dummy-vertex budget of 200,000 beyond
which long arcs are left unrouted *and reported as such*, `hierarchical.py:7` and `:670-672`; Circle
Packing's non-planar fallback to a packer that is **not** guaranteed tangent, `circle_packing.py:308-311`)
and we hold the same standard.

---

## 9. Docker — build one, delete nothing

- **Build:** `docker/rust.Dockerfile` — `debian:trixie-slim` + rustup `--profile minimal`, pinned
  version, `wasm32-unknown-unknown` target, `CARGO_HOME`/`RUSTUP_HOME` under `/opt` so a mounted `/w`
  cannot shadow them. Keep the header-comment convention of the existing `./Dockerfile`.
- **Reuse:** `node:22-slim`, already the base of the existing `Dockerfile`, for the oracle and the wasm
  hash arm.
- **Do NOT delete `ge-rig` / `ge-parity-rig`.** An earlier revision of this file told you to, claiming it
  would reclaim ~4 GB. **That was wrong, measured:**

  ```
  docker system df
  TYPE      TOTAL  ACTIVE  SIZE      RECLAIMABLE
  Images    18     2       5.216GB   4.542MB (0%)
  ```

  The 16 unused images share nearly all their layers with the 2 active ones, so deleting them frees
  **4.5 MB**, not 4 GB. The only real slack is **271 MB of reclaimable build cache**
  (`docker builder prune`), on a disk with 19 GB free. There is no space problem to solve.

  The deletion was also justified by the claim that "the CDP reverse tunnel replaces them." That reasoning
  is unsound: the tunnel is **one-way**. It forwards VM→host so the VM can *drive* host Chrome's CDP, but
  the VM is NAT'd (user-mode QEMU, `10.0.2.15`), so host Chrome **cannot load a page the VM serves** without
  an explicit `hostfwd`. Driving a browser and delivering it a page are different capabilities.

- **A documented §0.3 exception.** `ge-parity-rig` is built `FROM mcr.microsoft.com/playwright`, the exact
  base §0.3 bans, and §7.5 keeps the pixel rig as a migration safety net — so §0.3, §7.5 and the old §9
  were mutually contradictory. Resolution: **§0.3 governs images we build; this pre-existing rig is
  grandfathered, not rebuilt, and not extended.** If it ever needs rebuilding, it gets a slim base or it
  goes. Recording the exception is the point — an unstated exception is how a rule quietly dies.

---

## 10. The phases

Each file is self-contained and executable. Do them in order. Do not start a phase whose predecessor's
gate is not green.

| # | File | Delivers |
|---|---|---|
| 0 | `prompts/phase-00-foundation.md` | image, workspace, contract types, codegen, hash harness, **negative control**, ledger, the D1 measurement |
| 1 | `prompts/phase-01-topology.md` | dense index ↔ stable id, arena, 3× CSR, SoA columns, the **17** oracle functions, **H1 fixed to byte order** |
| 2 | `prompts/phase-02-contract-registry-grid.md` | geometry vocabulary, stage/layout registry, **grid layout end-to-end**, JSON↔binary round-trip |
| 3 | `prompts/phase-03-deterministic-layouts.md` | tidy tree, squarified treemap, circular/radial, **circle packing** (`Circle` geometry) |
| 4 | `prompts/phase-04-wasm-sdk.md` | columnar zero-copy transport, the JS SDK. **The motor becomes usable by any app here.** |
| 5 | `prompts/phase-05-sugiyama.md` | layered DAG: FAS, layering, crossing reduction, dummy vertices → `Polyline` |
| 6 | `prompts/phase-06-iterative-spectral-mds.md` | force/Barnes-Hut, ForceAtlas2, Yifan Hu, spectral, **Pivot MDS**. `devil` verdict required first. |
| 7 | `prompts/phase-07-analysis.md` | communities, centrality, Dijkstra + **a correctly wired Bellman-Ford**, components |
| 8 | `prompts/phase-08-post-routing-bundling.md` | edge styles, FDEB, MINGLE, obstacle-avoiding grid routing |
| 9 | `prompts/phase-09-scale-bench.md` | LOD, simplification, adaptive budgets, the 10k/100k/1M benchmarks |
| 10 | `prompts/phase-10-ingest-sdk-publish.md` | role-based ingest contract, two adapters, published SDK surface |

**Value lands at Phase 4, not Phase 10.** Phases 0–4 produce a genuinely reusable motor with a working
SDK and five layouts. 5–10 broaden the catalogue. The ordering deliberately puts *deterministic one-shot*
layouts before *chaotic iterative* ones, so the hash gate is proven on easy cases before force arrives.
That is the reverse of the obvious order, which is why it is written down.

### Out of scope — requires a human decision, do not take it

The 8 Graphviz engines · igraph DrL/LGL/Graphopt/Davidson-Harel · SBEB bundling · 3D and the 3D-only
geometric layouts · `graph-server` (axum/HTTP) · the mutation/write path · a declarative mapping DSL ·
`SharedArrayBuffer` (needs COOP/COEP, touches the Vercel config, can break embeds) · **any change to
osionos**.

---

## 11. Guardrails — each with its reason, because a rule whose reason is visible survives a phase that wants to bend it

1. **Per-phase authorization envelope.** *Because* a previous run added 3 modules and 6 public exports
   outside its scope.
2. **No dead exports** — every new public export needs a live caller in the same phase. *Because* 6
   exports shipped with zero consumers, and `GraphPatch` still has no applier.
3. **Reports are generated, not narrated** — structurally: the report *is* the ledger diff. *Because*
   `EXTRACTION_REPORT.md:12` claimed "byte-for-byte copy" after 8 files had diverged, and the author of
   this runbook then made the same class of error three times, restating a stale count without re-running
   the command. Nobody is exempt.
4. **Fresh review per phase, over that phase's diff only.** *Because* one sign-off at the start covered
   nothing that came after it.
5. **Coverage table** mapping every changed symbol to the test that exercises it; any row reading "none"
   is tested or deleted before the gate passes. *Because* a previous verification and the change it
   claimed to cover were disjoint surfaces.
6. **`Ponytail:` marker on every heuristic**, naming the **failing input** (not "complex input"), the
   **direction** of failure (under-reporting is dangerous, over-reporting merely noisy) and the **escape
   hatch**. It is a required ledger field, so a heuristic without one cannot reach `gated`.
7. **`devil` verdict before Phase 6** (physics, wide blast).
8. **No algorithm counts as done without its oracle differential and its declared scale ceiling.**
   *Because* SciGraphs shipped a Bellman-Ford menu entry that silently ran Dijkstra
   (`docs/tutorials/panels/scigraphs/algorithms.qmd:74-75`) — the exact failure mode of an unverified
   capability claim.

---

## 12. What every phase report contains

No prose summaries. This exact shape:

1. **Authorization compliance** — the files created and modified, checked against the phase's envelope.
   Any deviation named explicitly.
2. **The ledger diff** — `capabilities --json` before and after.
3. **The gate table** — every command from the phase's gate section, with its real exit code and real
   output. Re-run at report time, not quoted from earlier.
4. **The 4-way hash table** — per stage touched.
5. **Coverage table** — changed symbol → the test that exercises it.
6. **Ponytail markers added** — each with its failing input and direction.
7. **What you could not verify** — named as `UNKNOWN`, never assumed green. A skipped check is `SKIP`,
   not a pass.
8. **Stop-and-ask items** — anything the phase revealed that needs a human decision.
