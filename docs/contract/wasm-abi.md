# WASM ABI — the motor's `extern "C"` surface

Status: **authoritative** (Phase 4; POST and ANALYSIS added after; `gm_build_contract`
added in Phase 10 slice 7). Scope:
`crates/graph-wasm/src/{exports/,alloc.rs,handle.rs,views.rs,ingest.rs,contract.rs,seed_ingest.rs,
errors.rs,post.rs,analysis.rs,stage_exports.rs,lib.rs}`. No wasm-bindgen, no
wasm-pack anywhere in the tree (`cargo tree -p graph-wasm`, verified — see
`docs/measurements/phase04-transport.md`). Every export takes and returns plain `u32`
(D6); nothing wider crosses the boundary. `crates/graph-sdk-js` is the only sanctioned
caller for an application; this document is what it is built against.

## Two families of export

1. **The retained hash-gate shim** (`crate::gate_exports`, wasm32-only): `gm_topology`,
   `gm_layout_grid`, and (only in a `--features probe` build) `gm_probe`. These predate
   this phase's real ABI and are kept unchanged so the two stages they back — `topology`
   and `layout.grid` — keep hashing exactly the bytes Phase 2/3's already-green
   cross-target gate hashed. `graph-cli hashgate` still drives them, and adds the real
   ABI as a stage of its own beside them (see "Hash-gate wiring"); it does not replace
   them. `crates/graph-sdk-js` never calls these.
2. **The real ABI** (`exports/{build,columns,stages,state}.rs`, wasm32-only), below:
   `gm_build`/`gm_build_contract`/`gm_run`/`gm_release` and everything a caller needs
   around them, plus the two stages downstream of LAYOUT — `gm_post_*` and
   `gm_analysis_*` (see POST and ANALYSIS below). This is what the SDK, and C20's own
   proof, actually call.

## Exports

| export | signature | notes |
|---|---|---|
| `gm_abi_version` | `() -> u32` | The ABI's revision: `gm_abi_version()` returns `1` (`crate::ABI_VERSION`). Bumped whenever an export's signature, a refusal code's meaning or an accepted document version changes; never for a registry entry, which is counted at run time (C1). The SDK calls it first and refuses any other number with a message naming both. Additive: a module built before it lacks the symbol and is refused by name, as any module older than the SDK is. |
| `gm_alloc` | `(len: u32) -> u32` | Returns an offset into linear memory, `0` on refusal (`gm_last_error` names it). Fallible: `std::alloc::alloc` under an explicit `Layout::from_size_align(len.max(1), 4)` (C5) — never the infallible, aborting `Vec::reserve`/`Box::new` path. Zero-filled. Zero `len` still reserves 1 byte (`GlobalAlloc` with a zero-size layout is UB) but is tracked and freed as length `0`. |
| `gm_free` | `(ptr: u32, len: u32)` | `(ptr, len)` must be exactly a live, un-freed `gm_alloc` allocation, or the call is refused (`Code::FreeRefused`) and nothing is deallocated — a double free and a length lie are both caught this way, not just an unaligned or out-of-range pointer. |
| `gm_layout_count` | `() -> u32` | The registry's row count (`graph_core::registry::LAYOUTS`). Registry-driven (C1): a new layout changes this with no ABI change. |
| `gm_layout_id` | `(i: u32) -> u32` | Framed UTF-8 id of registry row `i`; `0` (`Code::IndexOutOfRange`) past the end. `gm_run`'s `layout_id` argument *is* this index — a caller finds it by scanning `0..gm_layout_count()` once at load, never a hard-coded constant. |
| `gm_build` | `(ingest_ptr: u32, ingest_len: u32) -> u32` | `(ingest_ptr, ingest_len)` must be a live `gm_alloc` allocation (C5); copies out of it, never frees it — the caller's buffer, the caller's job to free, always, even on refusal. Parses the **provisional** ingest JSON (below), indexes it into a topology, and returns a fresh handle, or `0` on any refusal (`IngestInvalid`, `BuildSourceInvalid`, `HandlesExhausted`). |
| `gm_build_contract` | `(contract_ptr: u32, contract_len: u32) -> u32` | Same buffer contract and the same handle table as `gm_build`, one ownership rule for both. Takes the **phase-10 ingest contract** (`docs/contract/ingest-schema.json`) instead of the provisional node/edge JSON, and derives the graph through `graph_core::ingest`'s single derivation (`crates/graph-wasm/src/contract.rs`). `0` on any refusal (`ContractInvalid`, `BuildSourceInvalid`, `HandlesExhausted`). **Additive: `gm_build` and its provisional format are unchanged**, so nothing already speaking it moves — see "Two build paths" below. |
| `gm_run` | `(handle: u32, layout_id: u32, params_ptr: u32, params_len: u32) -> u32` | Runs registry layout `layout_id` over `handle`'s topology at its default parameters — every registered `run: fn(&Topology)` this phase takes none (C2), so `params_len` must be exactly `0`; any other value is refused (`ParamsMustBeEmpty`), never silently ignored, and `params_ptr` is never read. `1` on success, `0` on refusal. A failed run clears the handle's previous geometry first (`Code::NoGeometryYet` on the next read), so a refusal never serves a stale snapshot. |
| `gm_node_count` | `(handle: u32) -> u32` | Nodes in `handle`'s topology, available right after `gm_build`, before any run. `0` is ambiguous (empty graph vs. invalid handle) — resolved by `gm_last_error`. |
| `gm_geometry_kind` | `(handle: u32) -> u32` | Node geometry tag of the last successful run: `0` Point, `1` Circle, `2` Box (`docs/contract/binary-layout.md`). `u32::MAX` — never a real tag — before any run has succeeded. |
| `gm_edge_geometry_kind` | `(handle: u32) -> u32` | Edge geometry tag: `0` Line, `1` Polyline, `2` Curve. Beyond the phase's stated minimum surface: `gm_geometry_kind` alone only names nodes, and C3 requires edge kind to be readable too. Same `u32::MAX` convention. |
| `gm_dim` | `(handle: u32) -> u32` | How many dimensions the last run carries: `0` 2D, `1` 3D (`docs/contract/binary-layout.md`, header byte 14). A reading, not a refusal — this layer transports 3D, so a consumer that cannot draw it checks this and declines. `0` with `InvalidHandle` or `NoGeometryYet` set, the same convention as the tags. |
| `gm_column_ptr` | `(handle: u32, column_id: u32) -> u32` | Offset of column `column_id`'s data for the last run. `0` if the handle is invalid, there is no geometry yet, or the id is reserved/inapplicable to this run's kind — an ambiguous `0`, resolved by `gm_geometry_kind`/`gm_edge_geometry_kind` (present-but-empty vs. absent) and `gm_last_error` (invalid handle vs. no geometry). A present but empty column also reads `0` (C3). An address the wire's `u32` cannot carry is refused (`IndexOutOfRange`), never truncated; on wasm32 every address fits, so only the native test host reaches that branch. |
| `gm_column_len` | `(handle: u32, column_id: u32) -> u32` | Element count of the same column — never assumed from node/edge count, since a reserved notes column (below) will have its own length `k`. |
| `gm_snapshot_json` | `(handle: u32) -> u32` | The canonical JSON face, framed UTF-8 (`graph_contract::canonical_json::to_json`). Re-validates every coordinate as finite first (D9, C8) and refuses with `Code::TamperedGeometry` if any column view wrote a non-finite value into the handle's buffers since the last run — column views are writable aliases directly into this snapshot's storage, and nothing else re-checks. |
| `gm_snapshot_bytes` | `(handle: u32) -> u32` | The binary face, framed (`docs/contract/binary-layout.md`). Same D9 re-validation. Beyond the phase's stated minimum surface: added so `harness/wasm-run.mjs`'s hash mode can compare the real-ABI path's bytes against the retained `gm_layout_grid` shim's bytes directly (C20) — both are the binary face of the same pipeline. |
| `gm_release` | `(handle: u32)` | Releases `handle`. Ids are monotonic and never reissued (C6): using a released id again always reads `InvalidHandle`, never a later graph that happens to reuse the number. |
| `gm_last_error` | `() -> u32` | The `Code` (below) the most recent fallible call left behind; `0` (`Code::None`) after success. Read-only — polling it does not change it, so it can be checked after any other export without disturbing what it would report. |
| `gm_seed_ingest` | `(seed: u32) -> u32` | Gate-only: the hash gate's model at `seed`, framed as the same provisional ingest JSON `gm_build` reads; `0` with `IngestInvalid` if the model holds a non-finite number, which JSON cannot carry (D9). Not part of the published SDK surface — `harness/sdk-smoke.mjs` never calls it; only `harness/wasm-run.mjs`'s hash mode does, to drive `gm_build`/`gm_run`/`gm_snapshot_bytes` over the gate's own seeded model for C20. |

`gm_layout_count`/`gm_layout_id`/`gm_last_error`/`gm_edge_geometry_kind`/`gm_dim`/
`gm_snapshot_bytes`/`gm_seed_ingest`/`gm_build_contract` are all beyond the phase's
literally stated minimum surface (`gm_alloc`, `gm_free`, `gm_build`, `gm_run`,
`gm_node_count`, `gm_column_ptr`, `gm_column_len`, `gm_geometry_kind`, `gm_snapshot_json`,
`gm_release`) — each is named above with why it exists; none replaces or hides one of the
ten. `gm_build_contract` in particular **adds a path rather than changing one**:
`gm_build` and the provisional document it reads are byte-for-byte what they were.

## The convergence proof, one command

The phase's proof is that the same logical dataset, in two source shapes, produces **one**
contract document and **one** graph. It used to need two runtimes — the adapters are
TypeScript (Node only) and the derivation is `graph_core::ingest`'s (Rust only) — so no
single command ran all of it. `gm_build_contract` closes that: Node can now do the third
step, so one command runs the whole chain.

```sh
# 1. build the module (the gate image has no node)
gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release
# 2. run the whole proof: two adapters -> one document -> the committed graph
node-slim.sh node --experimental-strip-types harness/sdk-smoke.mjs \
  --adapter-convergence target/wasm32-unknown-unknown/release/graph_wasm.wasm
```

Two containers, two commands, and that is the honest shape of it: `ge-rust` has no
`node` and `node:22-slim` has no `cargo`. What is now single is the **proof** — all three
steps are checked by one process against one committed artifact
(`fixtures/ingest/expected-graph.json`), where before the adapter half and the derivation
half were two runtimes reading two halves of the same file. `npm run
sdk:convergence:wasm` is step 2 with the path filled in; step 1 is the standard wasm build
every gate row already needs, so no new build step is introduced.

Run step 2 with **no** wasm path and it checks the adapter half alone — that mode predates
this slice and still works, so the existing gate row does not have to change.

What step 2 asserts, in order: both adapters map their source to byte-identical documents;
the documents equal the committed `ingest`; the document handed to `Motor#buildContract`
derives the committed `graph`'s node count, node ids in derivation order, and each edge
column in derivation order; four mutated documents are each **refused** (an unknown member,
a role outside the eight, an unsupported version, a tag value that cannot round-trip
through the node-id grammar); and a document with no records is a legal empty graph rather
than a refusal. Every mutation is asserted to have actually changed the text first, so a
"mutation" that no longer matches cannot pass as one.

The Rust half of the same proof is unchanged and still runs: `crates/graph-core`'s
`the_committed_graph_is_exactly_what_the_derivation_produces` pins the same committed
graph against the same derivation natively, and `crates/graph-wasm/src/contract/tests.rs`
pins it through this module. Three runtimes checking one artifact is redundant on purpose —
they catch different mistakes (an adapter's mapping, a derivation change, and a boundary
that stopped deriving), and none of them can pass by agreeing with itself.

## Hash-gate wiring

`transport.wasm.columnar` is a **stage of `graph-cli hashgate`**, not a separate
invocation: `hashgate/stages.rs` builds the stage list from the registry —
`topology`, every `graph_core::registry::LAYOUTS` row, then `transport.wasm.columnar` — and
the gate asks `harness/wasm-run.mjs` for all of them by name.

**Both arms are driven by the registry, and both are pinned for it.** The native arm's
bytes come from `stages::stage_bytes_for`, one producer per stage: the topology stage is
the pipeline's own topology bytes, and each registered layout's stage is
`(layout.run)(&topology)` → `graph_core::layout::snapshot`, i.e. that layout's own run over
that seed's topology. The grid is the one exception and it is deliberate: its stage is the
perturbed `run_pipeline::<Grid>` run, so `GM_MUTATE_GRID_SPACING` can reach it. On the
wasm side, the three shim-backed stages keep their frozen hashers and every *other*
registered layout is hashed through the real ABI with its id resolved through
`gm_layout_count`/`gm_layout_id`. `harness/wasm-run.mjs stages` prints that arm's own list,
and `hashgate::tests::stages::arm::the_wasm_arm_can_hash_every_stage_the_gate_asks_for`
compares the two lists in both directions — every stage the gate asks for is offered by the
arm, and the arm offers nothing extra.

This is the part that must not drift: a list that grows with the registry while the arms'
bytes do not makes the gate refuse its own honest run (`native run 1 printed N lines, need
M`, or `unknown stage` from the wasm arm) the moment a second layout is registered.

**A stage the arm does not have is refused by name, in every spelling.** The arm's
shim-backed stages live in a JS table, and a table that inherits from `Object.prototype`
answers `STAGE_BYTES["toString"]` with a *function*: the arm then hashed that member's
return value under a stage nobody ran, and printed a digest with exit 0. `toString`,
`constructor`, `__proto__` and `valueOf` are all names a caller can pass, and a gate that
prints a green line for a stage it did not run is worse than one that refuses — the
divergence the gate exists to catch would be a line nothing is compared against. So the
table has a null prototype and every unknown name is refused with
`unknown stage <name>: not a registered layout` and the harness's exit 2 for "could not
run". Pinned in both directions by
`hashgate::tests::stages::arm::a_stage_the_arm_cannot_hash_is_refused_however_it_is_named`.

The arm also resolves a layout name in more than one place — the stage list, the `gm_run`
index, and the membership test that decides whether a name is hashable at all — so it reads
the module's registry **once**, through one `layoutIndices()` derivation, rather than
re-scanning `0..gm_layout_count()` per call. That is what `gm_layout_id`'s own row above
promises a caller: find it by scanning once, never by a hard-coded constant, and never
three ways at once.

The wasm arm reaches the transport stage through the real ABI (`gm_seed_ingest → gm_alloc
→ gm_build → gm_run → gm_snapshot_bytes`); the native arm has no transport of its own, so
that stage's native bytes are the pipeline's own snapshot for `layout.grid` — the layout
`abiSnapshotBytes` runs — restated under the transport's name. The stage's claim is
therefore exactly: *the shipped module, through its real ABI, reproduces the native
pipeline's bytes.*

That gives the row two recorded verdicts, both in `hashgate.json`:

| verdict | what it reads | negative control |
|---|---|---|
| `hash_4way` | `equal["transport.wasm.columnar"] == seeds` over all four arms | `GM_MUTATE_GRID_SPACING` (the transport stage restates the grid's bytes, so the grid's control is its own: 0/8 seeds) |
| `oracle_diff` (`wasm-transport`) | `transport.equal == seeds`: the seeds where the real ABI reached the retained shim's bytes, counted from the wasm arm's own lines (C20) | none needed: the same count is what the harness enforces per seed, and a short count is a red gate and a `gated` claim `--check` refuses |

## Ledger

Two rows, `crates/graph-cli/src/capabilities/registry.rs`:

- **`transport.wasm.columnar` — `gated`**, on the two verdicts above. Both come from a
  real record (`hashgate.json`), on the current tree's fingerprint, with a control that
  went red on the same stage.
- **`sdk.js` — `implemented`.** Its gate is `harness/sdk-smoke.mjs`, a smoke script over
  one fixture, not a recorded seed sweep: no record backs it, so both of its verdict
  columns read `not backed: …` and `capabilities --check` would refuse a `gated` claim.
  Raising it needs a sweep that writes a record, not a different status.

## Framed buffers

Every export that returns a buffer (`gm_layout_id`, `gm_post_id`, `gm_analysis_id`,
`gm_analysis_run`, `gm_snapshot_json`, `gm_snapshot_bytes`, `gm_seed_ingest`, and the
retained shim's `gm_topology`/`gm_layout_grid`) writes `[len: u32 LE][len bytes]` into the
motor's own out-buffer and returns its address; `0` means the call was refused, not "an
empty buffer" (an empty result is still framed: `[0][]`, a real nonzero address). The
buffer is valid until the next call into the module, on *any* handle — a caller copies out
of it (the SDK's `Motor#frame` does this with `.slice()`) before doing anything else. The
two refusals every framed export shares: a body longer than `u32` can count is
`AllocFailed`, and an out-buffer address the wire cannot carry is `IndexOutOfRange`
(native test host only). A nonzero return always clears the code (C4).

## Ownership (C7)

| What | Owner | Freed by |
|---|---|---|
| An ingest buffer (`gm_alloc`'d) | The caller | The caller, via `gm_free` — `gm_build` only reads it |
| A framed return buffer | The motor's shared out-buffer | Overwritten by the module's next call; never explicitly freed |
| A column's `(ptr, len)` | The handle's snapshot | The handle's own storage; invalid the moment `gm_run` or `gm_post_run` re-runs that handle, or `gm_release` drops it. A POST pass replaces the snapshot, so a column view taken before one is stale after it |
| A handle | The handle table | `gm_release`; the id is never reissued (C6) |

## Columns (C3)

Column ids are append-only — a shipped id is never renumbered or reused for a different
meaning.

| id | name | element type | length | applies to |
|---:|---|---|---|---|
| 0 | `NODE_X` | `f32 × n` | node count | every node kind |
| 1 | `NODE_Y` | `f32 × n` | node count | every node kind |
| 2 | `NODE_R` | `f32 × n` | node count | Circle only |
| 3 | `NODE_W` | `f32 × n` | node count | Box only |
| 4 | `NODE_H` | `f32 × n` | node count | Box only |
| 5 | `EDGE_SOURCE` | `u32 × m` | edge count | every edge kind — dense node index |
| 6 | `EDGE_TARGET` | `u32 × m` | edge count | every edge kind — dense node index |
| 7 | `NOTE_CODE` | `u32 × k` | reserved | **reserved for Phase 3's notes section (contract 0.3) as `note.code`; always [`Column::Absent`] this phase, for every graph** |
| 8 | `NOTE_INDEX` | `u32 × k` | reserved | as above — `note.index` |
| 9 | `EDGE_OFFSETS` | `u32 × (m+1)` | — | Polyline and Curve only |
| 10 | `EDGE_PTS` | `f32 × 2×offsets[m]` | — | Polyline and Curve only |
| 11 | `EDGE_CURVE_DEGREE` | `u32 × 1` | 1 | Curve only |
| 12 | `NODE_Z` | `f32 × n` | node count | 3D only (`dim = 1` per `gm_dim`), whatever the node kind |

A column id past 12 is `Absent`, not a panic. "Absent" (reserved, or inapplicable to
this run's geometry kind) and "present but zero-length" both read `(ptr, len) = (0, 0)`
on the wire — the two are told apart by `gm_geometry_kind`/`gm_edge_geometry_kind`, never
by the pointer. A present-but-empty column reads `(0, 0)` rather than its `Vec`'s
dangling non-null address, which a view would otherwise be built over, so a `ptr === 0`
check is not a presence test (`crates/graph-sdk-js/src/views.ts`'s `columnApplies`
decides presence from the geometry kind instead, mirroring this table exactly). The force
session's columns follow the same rule.

Ids 7 and 8 are reserved for exactly the two fields Phase 3's `notes` section (contract
0.3) brings — `note.code` and `note.index` — and they are numbered *before* 9/10/11
deliberately, so that merge fills these two slots instead of renumbering anything shipped
here. Until it does, nothing in this snapshot type has a notes section, so both resolve to
`Absent` unconditionally, for every graph, whatever the geometry kinds. Any *further*
notes field beyond those two lands at 12+, again without renumbering.

Id 12 is `NODE_Z`, the 3D z column (contract 0.4). It is the one node column whose
applicability keys on the run's **dimension** rather than its node kind: every node kind
carries a `z` when `dim = 1`, and a 2D run has none at all, so `NODE_Z` is `Absent` for it —
absent, not a zero-length column a caller might read as a plane at depth 0. The wasm layer is
**transport**: it carries a 3D snapshot rather than refusing one, so `gm_dim` is how a
consumer learns a run is 3D instead of parsing byte 14 of the raw bytes itself. A consumer
that draws in 2D checks `gm_dim` and declines; the renderer and studio do, refusing by the
name `dimension-3d` rather than silently projecting z away.

**The whole table is exercised per layout, not per kind in the abstract.**
`harness/sdk-smoke.mjs` restates it — a consumer's copy, deliberately not the SDK's own
`columnApplies`, so a mistake in one copy cannot agree with itself — and for every
registered layout asserts that each id is present exactly when the run's declared node/edge
kind says it should be, that each present node column is `nodeCount` long, that the edge
source/target columns agree with each other, that `offsets` is `m + 1` and `pts` is
`2 × offsets[m]`, that a curve degree column is one element, and that both note columns are
absent. `docs/reports/phase-04.md` §6b records the two temporary registry rows
(Circle/Polyline and Box/Curve) used to observe that check going red on a wrong table: with
only the grid registered, every "absent" branch is the only branch there is, and a wrong
row in this table would pass unnoticed until a layout lands that needs it.

## POST — `gm_post_count` / `gm_post_id` / `gm_post_run`

The edge-geometry stage (`prompt.md` §3), over the same handle. A POST pass takes a
finished layout's geometry and returns geometry again, so **it composes with every
layout**: no capability here is told which layout produced the positions, or which node
kind it emitted.

| export | signature | notes |
|---|---|---|
| `gm_post_count` | `() -> u32` | The registry's row count (`crates/graph-wasm/src/post.rs::CAPABILITIES`). Registry-driven (C1), like `gm_layout_count`: a capability added there changes this with no ABI change. |
| `gm_post_id` | `(i: u32) -> u32` | Framed UTF-8 id of registry row `i`; `0` past the end (`Code::IndexOutOfRange`). `gm_post_run`'s `post_index` *is* this index. |
| `gm_post_run` | `(handle: u32, post_index: u32) -> u32` | Runs capability `post_index` at its **default parameters** and **replaces the handle's edge geometry** with the result, so `gm_column_ptr`/`gm_column_len` and the two snapshot faces read the new edges on the next call. `1` on success, `0` on refusal. |

Rows, in order: `post.bundle.fdeb`, `post.bundle.mingle` (both
`graph_core::post::POSTS`, reached through their own `PostRun`s), `post.route.grid`, and
`post.style.{straight,orthogonal,quadratic,bezier}` — the last five through **thin
adapters in `graph-wasm`**, not by changing a signature `graph-core` already publishes.
Routing and the styles take their parameters explicitly (`GridParams`,
`StyleParams::for_style`), so a row is a wrapper that supplies the pinned default; a
future capability registered in `graph-core` is one row away.

Edge geometry kind per row, which is what `gm_edge_geometry_kind` reports after a pass:
`Line` for `post.style.straight` (it stores no interior point, so it costs zero bytes),
`Polyline` for routing, orthogonal and both bundlers, `Curve` for quadratic and bezier.

**Two faces of a run, and why the ABI keeps both.** A handle holds the snapshot *and* the
layout's own geometry. A POST pass reads the **layout's** edges, never the previous pass's,
so `post` then `post.bundle.fdeb` gives the same answer as `post.bundle.fdeb` alone. A
caller cannot observe the difference through the transport, and that is the point: POST is
a stage, not a second transport.

**Refusals.** `InvalidHandle` (never issued, or released), `NoGeometryYet` (`gm_run` has
not succeeded on this handle — a pass has no positions to draw over), `IndexOutOfRange`
(`post_index` is not `< gm_post_count()`), `PostFailed` (the capability returned a
`StageError`, or its edges did not fit the snapshot). **A refused pass leaves the
handle's geometry exactly as it was**, so the next column read serves the previous good
drawing rather than nothing or a half-applied one.

`Bundled::unbundled` is the count a caller acts on where a pass has one: routing's
straight-segment fallbacks (the case a node fully enclosed by other nodes' cells, which
`routed.rs` reports as a field of its output and not as a log line — a downstream program
cannot read stderr) and FDEB's edges whose compatibility never cleared the threshold.
`pairs` is `0` for routing and the styles, honestly: neither attracts anything into a
bundle, so there is no pair count to report.

## ANALYSIS — `gm_analysis_count` / `gm_analysis_id` / `gm_analysis_run`

| export | signature | notes |
|---|---|---|
| `gm_analysis_count` | `() -> u32` | The registry's row count (`crates/graph-wasm/src/analysis.rs::ANALYSES`). Registry-driven (C1). |
| `gm_analysis_id` | `(i: u32) -> u32` | Framed UTF-8 id of registry row `i`; `0` past the end (`Code::IndexOutOfRange`). |
| `gm_analysis_run` | `(handle: u32, index: u32) -> u32` | Runs analysis `index` over `handle`'s topology and returns its **canonical JSON face**, framed UTF-8; `0` on refusal. |

**No geometry is required.** Every analysis in `graph_core::analysis` is a pure function of
the topology, so this works straight after `gm_build` and before any `gm_run`. The
refusals are `InvalidHandle`, `IndexOutOfRange`, and `AnalysisFailed` for a report holding a
non-finite number, which is refused rather than written as `NaN` (D9). Closeness and
betweenness over a graph with any negative edge `strength` (ingest admits every finite
number) are refused with `AnalysisFailed`: Dijkstra has no shortest path to state there.

Rows, in order: `analysis.components.weak`, `analysis.components.strong`,
`analysis.communities.louvain`, `analysis.centrality.{degree,closeness,betweenness,
eigenvector}`, `analysis.depth.bfs`. Each row calls the `graph_core::analysis` function
its id names, as it is — nothing is re-derived, re-weighted or re-ordered here. Depth
uses `graph_core::layout::hierarchy::Hierarchy` directly as a `Roots` — the re-point
named in `analysis/depth.rs`'s module doc has landed, and `Hierarchy` implements
`Roots` by delegation. There is no forwarding adapter and no second derivation of the
convention.

The face, one line, **keys in ascending order** so two runs are byte-comparable (D7):

```json
{"id":"analysis.components.weak","kind":"u32","nodeCount":3,"values":[0,0,0]}
{"id":"analysis.communities.louvain","kind":"u32","modularity":0,"nodeCount":3,"values":[0,0,0]}
{"converged":false,"id":"analysis.centrality.eigenvector","kind":"f64","nodeCount":3,"values":[0.5773502588272095,0.5773502588272095,0.5773502588272095]}
{"id":"analysis.depth.bfs","kind":"u32","max":2,"nodeCount":3,"values":[0,1,2]}
```

`values` is one entry per node, in the motor's dense-index order — the same order every
column and both snapshot faces use. `kind` is `"f64"` for a centrality (graph-core states
those as `f32`; widening is exact) and `"u32"` for a labelling, a community id, a depth
level or the degree count, so a consumer never has to know which analysis it asked for to
know how to read `values`. Three optional members are present exactly when the analysis
hands one back, and each is the escape hatch that analysis's own `Ponytail` marker names:
`converged` for the eigenvector power iteration (`false` on a bipartite or disconnected
graph, where the iteration oscillates and returns the last normalised iterate — dropping
the flag would show a caller three equal numbers as a centrality), `modularity` for the
partition Louvain returned, `max` for the deepest level depth reached.

**A path query is not exposed.** `analysis/paths.rs` needs a source node and a mode,
neither of which `(handle, index)` can carry without inventing a convention; it stays a
graph-core-only capability.

## Errors (`Code`, `gm_last_error`)

| value | name | when |
|---:|---|---|
| 0 | `None` | The previous call succeeded, or none has run yet |
| 1 | `InvalidHandle` | The handle was never issued, or has been released |
| 2 | `AllocFailed` | `gm_alloc` could not reserve the requested bytes |
| 3 | `FreeRefused` | `gm_free`'s `(ptr, len)` is not exactly a live `gm_alloc` allocation |
| 4 | `IngestInvalid` | The ingest buffer failed the provisional JSON contract |
| 5 | `UnknownLayoutId` | `gm_run`'s `layout_id` is not `< gm_layout_count()` |
| 6 | `ParamsMustBeEmpty` | `gm_run`'s `params_len` was not `0` |
| 7 | `HandlesExhausted` | Every `u32` handle id has been issued this instance; none is reused (C6) |
| 8 | `LayoutFailed` | The registered layout returned an internal error for this topology |
| 9 | `TamperedGeometry` | A column read back NaN or infinite: a view wrote through the handle's buffers since the last run (D9, C8) |
| 10 | `NoGeometryYet` | The handle has no geometry yet: `gm_run` has not succeeded on it |
| 11 | `BuildSourceInvalid` | `gm_build`'s `(ptr, len)` is not exactly a live `gm_alloc` allocation |
| 12 | `IndexOutOfRange` | An index argument (`gm_layout_id`, `gm_post_id`, `gm_analysis_id`) is past the end of its list |
| 13 | `PostFailed` | The registered POST capability returned a `StageError`, or its edges did not fit the snapshot. The handle keeps the geometry it had |
| 14 | `ContractInvalid` | `gm_build_contract`'s buffer is not a valid ingest contract document — the contract's strict reader refused it (unknown member, unnamed role, unsupported version, dangling collection, a `:` in a coordinate that cannot round-trip) **or** the derivation refused the graph it describes (a tag value containing `:`). One code for both, because "was my document accepted" is the question a caller asks and the reader's checks all run first; which of the two said no is a question about the document's content, and both are loud. |
| 15 | `InvalidSession` | The session id does not name a live force session (never issued, or released). The session's own id space, never the graph handle's `InvalidHandle` |
| 16 | `SessionParamsInvalid` | A force session's `(params_ptr, params_len)` is neither `0` (the defaults) nor exactly the parameter buffer's length |
| 17 | `SessionRefused` | The force session refused: a parameter out of its range (never clamped), a row past the last node, or a non-finite coordinate (D9) |
| 18 | `AnalysisFailed` | `gm_analysis_run` ran the analysis but its report has no JSON text: a non-finite score or modularity (`NaN` is not a JSON number, D9), or a column longer than `u32` can count |
| 19 | `IngestTooLarge` | `gm_build`'s buffer is longer than `MAX_INGEST_BYTES` (774,568,785 bytes), refused on its length before any of it is read. **Not** `IngestInvalid`: that code means the document was read and found malformed, while this one means the document must be split or shrunk |

Codes are **append-only**: `ContractInvalid` was added as `14` and moved no existing
code, which `crates/graph-wasm/src/errors.rs`'s
`the_new_code_appends_and_does_not_move_any_other` pins — an SDK indexes
`CODE_NAMES` (`crates/graph-sdk-js/src/errors.ts`) by the same order, so renumbering
would silently turn a caller's `UnknownLayoutId` into a `NoGeometryYet`. The three copies
(`Code`, this table, `CODE_NAMES`) are pinned to one another by
`crates/graph-wasm/src/errors/mirrors.rs`. `ContractInvalid`
is deliberately **not** `IngestInvalid`: the two name different documents, and a code that
did not say which was refused would let a caller handle a contract rejection as a
node/edge rejection.

`0` is both the wire's generic failure sentinel *and* a legitimate data value (an empty
graph's `gm_node_count`, an absent column's `gm_column_ptr`) — every ambiguous `0` is
documented above as resolved by `gm_last_error`, never left for a caller to guess at.

## Two build paths — `gm_build` and `gm_build_contract`

Two exports take a document and return a handle. They take **different documents**, they
hand back handles from the same table with the same rules, and neither accepts the
other's document.

| | `gm_build` | `gm_build_contract` |
|---|---|---|
| document | the **provisional** node/edge JSON (below, C13) | the phase-10 **ingest contract** (`docs/contract/ingest-schema.json`) |
| who derives the graph | the caller already wrote nodes and edges | `graph_core::ingest`'s single derivation |
| refusal code | `IngestInvalid` | `ContractInvalid` |
| who uses it | the host studio, `harness/wasm-run.mjs`, `gm_seed_ingest`'s output | this package's `rowsToIngest`/`notionToIngest` adapters |

**`gm_build` and its format are unchanged.** The provisional shape is what the host studio
and the hash gate's C20 stage already speak, and rewriting it would move a published ABI's
meaning without adding anything a caller can use. `gm_build_contract` is purely additive:
a new symbol, a new code, and a path that did not exist before.

The two formats are deliberately **not interchangeable**, and each reader refuses the
other's document — `crates/graph-wasm/src/contract/tests.rs`'s
`the_two_ingest_formats_are_not_interchangeable` pins both directions. That is what stops
this from quietly becoming one export: routing `gm_build_contract` to the provisional
parser would derive an empty graph from a contract document, and routing `gm_build` here
would refuse every document the studio sends.

What the contract path adds is that the graph is derived **once, by the motor**. Before it
existed, a JS consumer could produce a contract document but had no way to hand it to the
module — and deriving the graph in JS instead would have been a second copy of the
derivation, which is the exact thing this phase exists to end (graph derivation existed in
three copies in the host and they had already diverged, so two live code paths produced
different layouts for the same data).

## Ingest — PROVISIONAL (C13)

`gm_build` takes a versioned JSON document in
`graph_core::records`' own shape, parsed by `graph_contract::canonical_json`'s strict
RFC 8259 reader (`crates/graph-wasm/src/ingest.rs`). This exists only so Phase 4 has
something concrete to build `gm_build` against, and it remains the format the host studio
and the hash gate speak — see "Two build paths" above.

```json
{
  "version": 1,
  "nodes": [
    { "id": "a", "kind": "record", "database_id": null, "source": "s",
      "label": "A", "group": null, "weight": 0.5, "version": 0,
      "has_note": false, "icon": null }
  ],
  "edges": [
    { "id": "e", "source": "a", "target": "b", "kind": "relation",
      "label": "", "strength": 0.5, "directed": false, "record_id": null }
  ]
}
```

Rules, all refused loudly (never silently coerced or dropped):

- `version` must be exactly `1`.
- Every member above is required; a field that may be absent is a present `null`, never
  an omitted key. An unknown member anywhere (a stray camelCase `hasNote`, say) refuses
  the whole document.
- One exception: an edge's `child_first` is optional in version 1: omitted, it reads `false`
  (parent-first, `graph_core::records`); present, it must be a boolean. Making it required
  needs a version 2, since senders written to the example above omit it.
- `kind` strings are matched by exact `NodeKind`/`EdgeKind` name, never the lossy
  `edge_kind_from_type` heuristic the TS oracle uses for legacy data.
- A duplicate node or edge `id`, or an edge naming a `source`/`target` not present in
  `nodes`, refuses the document outright (C12) — `graph_core::index_model` itself is
  first-wins/drop-silently for exactly these cases, which would make ingest order diverge
  from snapshot order, the one identity this ABI promises a caller.
- Not UTF-8, or not JSON at all, is refused before shape-checking even starts.
- A document longer than `MAX_INGEST_BYTES` (774,568,785 bytes) is refused with
  `IngestTooLarge` on its length alone, before it is read at all. The number is measured,
  not chosen: it is the largest document that built on the wasm32 artifact, byte for byte,
  and the next one up, 799,922,860 bytes, trapped inside `index_model`'s string arena
  (`docs/measurements/fix-wasm-ingest.md`). It has no margin, because it *is* the
  measurement — nothing between it and that first trap has been shown to build — and it is
  a ceiling rather than a promise: a document under it with an unusually high edge-to-node
  ratio can still exhaust memory exactly as it does today. `fix-ingest-scale` owns that
  defect and raises this number with a new measurement once it lands.
- A number is refused wherever JSON does not admit non-finite values in the first place —
  D9's "no NaN/Inf reaches the wire" is enforced again on the way out (`gm_snapshot_json`/
  `gm_snapshot_bytes`), since a column view can still write one in after `gm_build`.

## SDK surface

`crates/graph-sdk-js/src/index.ts`'s `createMotor`/`Motor` class is the sanctioned
caller; `crates/graph-sdk-js/README.md` documents its own ownership and error policy in
JS terms. `harness/sdk-smoke.mjs` is a third party exercising only that published entry
point — it never imports from `crates/`, never touches the ABI directly.

`Motor#layouts()` returns every registered layout id, in registry order, from
`gm_layout_count`/`gm_layout_id` (C1). It is the only way a consumer is meant to learn
what a module can run, and `Motor#layout` resolves the id it is given through the same
map, so the registry is read once per motor. Without it the surface could only name a
layout a caller already knew: a layout registered after this ABI shipped would be
reachable but undiscoverable, and the phase's own smoke test — which is written to run
*every* registered layout and print its node count and bounds — would have had to hard-code
names, covering one layout forever. On a degraded motor `layouts()` refuses like every
other method that needs the module; it never answers `[]`, which would be
indistinguishable from "this module has no layouts".

**`Motor#buildContract(contractJson)`** is the SDK's method for `gm_build_contract`: it
takes an ingest contract document — the shape `rowsToIngest`/`notionToIngest` produce, and
`docs/contract/ingest-schema.json` describes — stages and frees the buffer exactly as
`Motor#build` does, and returns a `Handle` usable with every other method from there on
(`nodeCount`, `layout`, `column`, `toJSON`, `analysis`, `release`). It is a **separate**
method rather than an overload of `build`, and the two refuse each other's documents with
different error classes (`ContractRefusedError`, `codeName: "ContractInvalid"`, vs.
`BuildRefusedError`, `"IngestInvalid"`), so a caller that catches one can never mistake a
contract rejection for a node/edge one. On a degraded motor it throws the latched
`WasmUnavailableError` like every other method that needs the module.

**`Motor#posts()`, `Motor#post(handle, id)`, `Motor#analyses()`,
`Motor#analysis(handle, id)`** extend the same rule to the two stages downstream of
LAYOUT, and each pair refuses on a degraded motor for the same reason. `post` returns a
`PostResult` — the handle, the capability that ran, and the two geometry kinds, the edge
kind being the one a caller cannot predict (it is the capability's own declaration).
`analysis` returns an `AnalysisResult`: the ABI's face parsed, narrowed and **checked**
member by member rather than cast, because a face whose `id`, `kind` or `nodeCount` did
not agree with itself would otherwise reach a caller as a plausible-looking object holding
someone else's numbers. Its three optional members (`converged`, `modularity`, `max`) are
`undefined` for the analyses that do not hand one back, and a member of the *wrong* type
is an `AnalysisRefusedError` rather than a silent `undefined` — a caller told
`converged: undefined` would read it as "no flag was handed back" and trust numbers that
were never verified. The three registries are read through one shared scan
(`index.ts`'s `#readRegistry`), so they cannot drift into three differently-shaped reads.

## Loader pattern

`crates/graph-sdk-js/src/wasm.ts`'s `loadMotor`: one module-level singleton, a deduped
in-flight `initPromise` (concurrent `createMotor` calls before the first resolves share
one instantiation, never two), an `initFailed` latch (a failed load is not retried for
the process lifetime — a deliberate trade, escape hatch: reload the page, or the
test-only `resetForTests()`), and a `globalThis.__GM_DISABLE_WASM__` kill switch checked
before the latch. Modeled on
`refs/notion-database-sys/docs/cheatsheet/wasm_native/wasm-bridge.md`'s pattern (see
Deviations). `loadMotor` itself still rejects on any of these; `index.ts`'s
`createMotor`/`Motor.create` is the layer that never does — it catches that rejection and
returns a degraded `Motor` (see Deviations).

## Coverage

| export | exercised by |
|---|---|
| `gm_alloc` / `gm_free` | `crates/graph-wasm/src/alloc.rs` unit tests (native); `Motor#build`'s stage/free (`harness/sdk-smoke.mjs`) |
| `gm_layout_count` / `gm_layout_id` | `Motor#layouts` (`harness/sdk-smoke.mjs`, `harness/wasm-run.mjs`'s `layoutIndex` helper) |
| `gm_build` | `crates/graph-wasm/src/ingest.rs` unit tests (native, the parser); `harness/sdk-smoke.mjs`, `harness/wasm-run.mjs --assert-zero-copy` |
| `gm_build_contract` | `crates/graph-wasm/src/contract/tests.rs` (native, 8: the committed document derives the committed graph byte for byte, the derivation is `graph_core`'s and not a copy, the two formats are not interchangeable, every reader refusal, a tag value that cannot round-trip, deletion honoured); `harness/sdk-smoke.mjs` (via `Motor#buildContract`: node count and derivation order, both non-interchangeability directions, an unknown member refused, the code named) |
| `gm_run` | `harness/sdk-smoke.mjs`, `harness/wasm-run.mjs --assert-zero-copy`, `abiSnapshotBytes` (C20) |
| `gm_node_count` | `harness/sdk-smoke.mjs` (including the released-handle refusal, C6) |
| `gm_geometry_kind` / `gm_edge_geometry_kind` | `harness/sdk-smoke.mjs` (`layout.grid reports Point/Line geometry`) |
| `gm_column_ptr` / `gm_column_len` | `crates/graph-wasm/src/views.rs` unit tests (native, every kind combination); `harness/sdk-smoke.mjs`'s per-layout column checks, which restate this table kind by kind for **every** registered layout and read each present column's length; `harness/wasm-run.mjs --assert-zero-copy`'s zero-copy and growth proofs |
| `gm_snapshot_json` | `harness/sdk-smoke.mjs` (`toJSON`, the D9 tamper refusal) |
| `gm_snapshot_bytes` | `abiSnapshotBytes` (`harness/wasm-run.mjs`, C20) |
| `gm_release` | `harness/sdk-smoke.mjs` (C6) |
| `gm_last_error` | Every refusal path above — `crates/graph-wasm/src/errors.rs` unit tests natively |
| `gm_seed_ingest` | `crates/graph-wasm/src/seed_ingest.rs` unit tests (native, round-trips to `seeded_model`); `harness/wasm-run.mjs`'s `abiSnapshotBytes` |
| `gate_exports::gm_topology` / `gm_layout_grid` | `graph-cli hashgate` (the `topology` and `layout.grid` stages, unchanged) |
| `gm_layout_count` / `gm_layout_id` as hash-gate stages | `crates/graph-cli/src/hashgate/tests/stages.rs::the_stages_are_the_topology_then_every_registered_layout_then_the_transport`; `…::a_second_registered_layout_joins_the_gate_with_no_change_to_the_stage_list`; `…::arm::the_wasm_arm_can_hash_every_stage_the_gate_asks_for`; `harness/wasm-run.mjs`'s `layoutIndex` and its `stages` mode |
| a stage the wasm arm cannot hash being **refused by name**, never hashed as something else | `crates/graph-cli/src/hashgate/tests/stages/arm.rs::a_stage_the_arm_cannot_hash_is_refused_however_it_is_named` — `toString`, `constructor`, `__proto__`, `valueOf`, `hasOwnProperty` and a genuinely unregistered id each exit 2 with `unknown stage <name>`, and the three shim-backed stages still hash |
| a *newly registered* layout being hashable by the wasm arm without editing the harness | `…::stages::the_wasm_arm_can_hash_every_stage_the_gate_asks_for`; `docs/reports/phase-04.md` §6a, where a temporary second registry row was used to observe the pre-fix failure and the post-fix `PASS` |
| `transport.wasm.columnar` as a hash-gate stage | `graph-cli hashgate`'s own output and record (`crates/graph-cli/tests/cli.rs::hashgate_passes_on_an_honest_run`, `…::each_negative_control_goes_red_on_its_own_stage`); `hashgate/transport.rs`'s tally unit tests; the `gated` row's two verdicts (`capabilities/tests/transport.rs`) |
| `createMotor`/`Motor.create` degrading rather than throwing (kill switch, compile failure) | `harness/sdk-smoke.mjs`'s `createMotor_never_throws_on_kill_switch` / `createMotor_never_throws_on_compile_failure` / `degraded_motor_build_fails_predictably_*` checks |
| `Motor#layouts` (the registry, and every registered layout run through the published SDK with its node count, bounds and columns) | `harness/sdk-smoke.mjs`'s registry checks and its per-layout loop — the list is the module's own, so a newly registered layout is covered with no edit to the harness; `docs/reports/phase-04.md` §6b, where three temporary registry rows (Point/Line, Circle/Polyline, Box/Curve) were used to observe the pre-fix script covering one layout of three and the post-fix script covering all three |
| a degraded motor refusing `layouts()` rather than answering an empty registry | `harness/sdk-smoke.mjs`'s `degraded_motor_layouts_fails_predictably_kill_switch` |
| `gm_post_count` / `gm_post_id` | `crates/graph-wasm/src/post.rs` unit tests (native: the id list, and a refusal past the end); `Motor#posts` (`harness/sdk-smoke.mjs`) |
| `gm_post_run` | `crates/graph-wasm/src/post/tests.rs` (every capability over three layouts and over hand-written geometry; node positions and notes preserved; the CSR well formed at every boundary; a routing fallback reported); `crates/graph-wasm/src/stage_exports/tests.rs` (the handle's edge kind after a pass, a second pass reading the layout, all four refusals, a refusal leaving the geometry untouched, a released handle refused); `harness/sdk-smoke.mjs`'s per-capability loop, which runs **every** id the module's own registry names and checks the contract's edge-kind table, the unmoved nodes, the columns and both snapshot faces |
| a post pass with no layout run yet | `crates/graph-wasm/src/stage_exports/tests.rs` (`NoGeometryYet`); `harness/sdk-smoke.mjs` (`a post pass with no layout run yet is refused`, and the refusal's `codeName`) |
| `gm_analysis_count` / `gm_analysis_id` | `crates/graph-wasm/src/analysis/tests.rs` (the id list, a refusal past the end); `Motor#analyses` (`harness/sdk-smoke.mjs`) |
| `gm_analysis_run` | `crates/graph-wasm/src/analysis/tests.rs` — the whole face pinned **byte for byte** for all eight rows, the keys' ascending order, the contract's own parser reading it, and each row cross-checked against the `graph_core::analysis` function it names over three fixtures; `crates/graph-wasm/src/stage_exports/tests.rs` (runs before any layout, both refusals, a released handle); `harness/sdk-smoke.mjs`'s per-analysis loop plus its pinned weak components, hierarchy depth and degree values |
| an analysis face that does not name itself | `harness/sdk-smoke.mjs` — the per-analysis loop catches the refusal and reports `not ok` rather than aborting, so a broken face is red *and* the rest of the smoke still runs (observed going red with `analysis.components.weak` reported for every id) |
| `Motor#posts` / `#analyses` / `#post` / `#analysis` on a degraded motor | `harness/sdk-smoke.mjs`'s `degraded_motor_{posts,analyses,post,analysis}_fails_predictably_kill_switch` |

## File-size deviations (the house's ≤300-line limit)

**There are none.** This section used to record four, and every one of the four has since been
retired by the house's own answer — split into child modules, never compress — so the record
is replaced by what replaced it rather than left to mislead the next reader:

| retired entry | what it was | where it went |
|---|---|---|
| `crates/graph-sdk-js/src/index.ts` at 554 | the `Motor` class *and* the entry point's export list in one file | `index.ts` is now the barrel a consumer imports (22 lines) and holds the published surface; the class is `motor.ts` (297) and the four blocks it delegates are `stages.ts` (123) |
| `harness/sdk-smoke.mjs` at 727 | one script with one registry of checks | `harness/sdk-smoke.mjs` (66) and `harness/sdk-smoke/{lib,build,layouts,post,analysis,transport,force,degraded,convergence}.mjs`; the `check`/`failures` counters stayed process-global in `lib.mjs`, which is what made the split possible |
| `crates/graph-wasm/src/post/tests.rs` at 379 | one test module | `crates/graph-wasm/src/post/tests/{mod,fixtures,rows}.rs`, largest 196 |
| `crates/graph-wasm/src/analysis/tests.rs` at 404 | one test module | `crates/graph-wasm/src/analysis/tests/{mod,fixtures,json}.rs`, largest 161 |

The claim this section used to make — that splitting `index.ts` "is not available without a
restructuring outside this task's envelope", because splitting the `Motor` class "would mean
exporting an implementation detail or re-exporting through a barrel the type surface then has
to mirror" — was wrong, and is withdrawn. The barrel *is* the answer for a published entry
point: `index.ts` re-exports the class and every name the docs, the README and a harness file
import, and `package.json`'s `"."` still points at `src/index.ts`, so no consumer path moved.

`crates/graph-wasm/src/contract.rs` (79) and `contract/tests.rs` (269) are both **under** the
limit, as recorded.

## Deviations

- **`osionos/src/shared/notion-database-sys/src/lib/engine/bridge.ts:63-86` no longer
  exists upstream** — verified: the file that path names does not have those lines in
  the read-only `osionos` checkout this session has access to. Followed
  `refs/notion-database-sys/docs/cheatsheet/wasm_native/wasm-bridge.md` instead, which
  documents the same house pattern (singleton, deduped `initPromise`, `initFailed`
  latch, kill switch, degrade) in prose; `wasm.ts` implements it from that description.
  `wasm.ts`'s own `loadMotor` still rejects on failure (kill switch, the latched
  `initFailed`, or a compile/instantiate error) — a plain, typed `Promise` rejection is
  the ordinary shape for an async loader and is exercised directly where useful. The
  phase's literal "never throw" requirement is met one level up, at `createMotor`/
  `Motor.create` (`index.ts`), the actual published entry point: it never rejects.
  A load failure there resolves to a *degraded* `Motor` instead — its `available` getter
  reads `false`, and every method that would need the real module (`build`, `layout`,
  `column`, `toJSON`/`toBytes`, `release`) throws the latched `WasmUnavailableError`
  predictably at first use via a shared `#requireLoaded` guard, rather than at
  `createMotor` itself. This is deliberately not a fabricated safe-default return value
  (a fake handle would itself need to answer `gm_node_count`/`gm_run`/etc. with more
  fabricated values, silently, which is a worse failure mode) — it is "the host page
  does not go down at load", the literal harm the requirement names, met without
  inventing data. See `harness/sdk-smoke.mjs`'s `createMotor_never_throws_on_kill_switch`
  / `createMotor_never_throws_on_compile_failure` checks.
- `crates/graph-cli/src/capabilities.rs` in the phase's literal MODIFY list is
  `crates/graph-cli/src/capabilities/registry.rs` in this tree — `capabilities.rs` was
  already split into a `capabilities/` module (registry, verdict, tests) before this
  phase by an earlier one; the two new ledger rows were added to `registry.rs`, the file
  that actually holds the registry, plus `capabilities/tests/{mod.rs,registry.rs}` and
  `crates/graph-cli/tests/cli.rs` to keep their row-count assertions honest.
  `capabilities.rs` itself (the `ledger`/`problems` driver) was not touched.
  `crates/graph-cli/src/hashgate{.rs,/stages.rs,/transport.rs,/compare.rs,/tests.rs}` is
  outside the envelope too, and cannot be avoided: the phase's own Ledger delta asks for
  `transport.wasm.columnar` "with its own gate", and the only gate that hashes the real
  ABI is `hashgate` (see "Hash-gate wiring"). `capabilities/tests/transport.rs` and
  `capabilities/verdict.rs` follow from the same row.
  `Cargo.toml`/`Cargo.lock` picked up `graph-wasm`'s dependency on `graph-contract`
  (already a workspace member; no new external crate).
  `canonical_json.rs` gained one export (`pub use parse::{Value, parse};`, was `Value`
  only) so `ingest.rs` can call the existing parser — no parsing logic changed.
  `crates/graph-wasm/src/lib.rs`'s original `mod exports { ... }` (the hash-gate shim)
  was renamed to `mod gate_exports` to avoid an `E0428` collision with the new
  `exports.rs` file (C21) — its body and every export it defines are unchanged.
- House limits (≤300 lines/file) split the wasm-side logic across
  `alloc.rs`/`handle.rs`/`ingest.rs`/`seed_ingest.rs`/`views.rs`, each with its own
  `mod tests` submodule, plus `crates/graph-wasm/src/memory_measure.rs` (new — a
  `#[cfg(test)]`-only unit-test measurement file, not part of the ABI itself; see
  `docs/measurements/phase04-transport.md`). The real-ABI export surface itself
  (`exports.rs` in the phase's literal CREATE list) is `exports/{mod.rs,state.rs,
  build.rs,columns.rs}` — a single-file `exports.rs` measured 312 lines, over the limit;
  `state.rs` holds the shared handle table and out-buffer, `build.rs` is graph lifecycle
  through a successful run, `columns.rs` is reading a finished run back out. The split is
  invisible on the wire: every `#[unsafe(no_mangle)] extern "C"` symbol is a real crate
  export regardless of which of the three files defines it.
- **POST and ANALYSIS add four files**, for the same reason and the same invisibility:
  `src/post.rs` and `src/analysis.rs` hold the two registries and their work (unit-tested
  natively, like the rest of C21's target-independent layer), `src/stage_exports.rs` holds
  what the two `gm_*_run` exports delegate to — the handle table and the refusal table,
  which is what makes every branch of both exports testable without a wasm build in the
  loop — and `exports/stages.rs` holds the six `#[unsafe(no_mangle)]` functions over the
  shared out-buffer. `post.rs` needed its tests in `post/tests.rs` and `analysis.rs` its
  tests in `analysis/tests.rs` for the same 300-line reason `views.rs` did.
- `gm_layout_count`, `gm_layout_id`, `gm_last_error`, `gm_edge_geometry_kind`, `gm_dim`,
  `gm_snapshot_bytes` and `gm_seed_ingest` are exports beyond the phase's literally
  stated minimum surface — each is justified in the export table above.
- The reviewer's BLOCKER on the two transport rows was accepted as **"amend the
  envelope"** (`docs/reports/STATUS.md` §3 p4), so the `capabilities/` and `hashgate/`
  paths above are an accepted amendment rather than an undeclared overrun. Recorded in
  full, with each path and its cause, in `docs/reports/phase-04.md` §1.
- `gm_alloc`/`gm_free` are implemented with `std::alloc::{alloc, dealloc, Layout}`
  directly, not a `Vec<u8>`-backed buffer — needed to make the documented 4-byte
  alignment guarantee (`alloc::ALIGN`) actually load-bearing rather than incidental.
- **C20 (hash equality through the real ABI) is wired into `graph-cli hashgate` as a
  stage of its own**, not left to a hand-run harness. `hashgate/stages.rs` appends
  `transport.wasm.columnar` to the registry's stages after the topology and every
  registered layout, so the wasm arm drives `gm_seed_ingest → gm_alloc → gm_build →
  gm_run → gm_snapshot_bytes` inside the gate that already runs, and a divergence names
  the transport rather than the layout it re-derives. `harness/wasm-run.mjs` still
  enforces the per-seed equality itself and exits `1` on the first divergence;
  `hashgate/transport.rs` counts the same thing from the arm's lines and records it as
  `hashgate.json`'s `transport` tally, which is what the ledger reads (see "Hash-gate
  wiring"). The two shim-backed stages are untouched, so Phase 2/3's already-green gate
  still hashes what it always hashed — the transport is added beside them, not folded
  into them.
  What the native arm contributes to this stage is the pipeline's own snapshot for
  `layout.grid`, restated under the transport's name: the native side has no
  provisional-ingest transport of its own (that path exists only on the wasm32 target),
  so the cross-target comparison is the whole of the stage's claim, and it is the claim
  C20 is about. The `GM_MUTATE_GRID_SPACING` control moves both grid-derived stages
  together, so the transport stage has a red control of its own.
