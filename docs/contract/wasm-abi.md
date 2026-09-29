# WASM ABI — the motor's `extern "C"` surface

Status: **authoritative** (Phase 4). Scope: `crates/graph-wasm/src/{exports/,alloc.rs,
handle.rs,views.rs,ingest.rs,seed_ingest.rs,errors.rs,lib.rs}`. No wasm-bindgen, no
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
2. **The real ABI** (`exports/{build,columns,state}.rs`, wasm32-only), below:
   `gm_build`/`gm_run`/`gm_release` and everything a caller needs around them. This is
   what the SDK, and C20's own proof, actually call.

## Exports

| export | signature | notes |
|---|---|---|
| `gm_alloc` | `(len: u32) -> u32` | Returns an offset into linear memory, `0` on refusal (`gm_last_error` names it). Fallible: `std::alloc::alloc` under an explicit `Layout::from_size_align(len.max(1), 4)` (C5) — never the infallible, aborting `Vec::reserve`/`Box::new` path. Zero-filled. Zero `len` still reserves 1 byte (`GlobalAlloc` with a zero-size layout is UB) but is tracked and freed as length `0`. |
| `gm_free` | `(ptr: u32, len: u32)` | `(ptr, len)` must be exactly a live, un-freed `gm_alloc` allocation, or the call is refused (`Code::FreeRefused`) and nothing is deallocated — a double free and a length lie are both caught this way, not just an unaligned or out-of-range pointer. |
| `gm_layout_count` | `() -> u32` | The registry's row count (`graph_core::registry::LAYOUTS`). Registry-driven (C1): a new layout changes this with no ABI change. |
| `gm_layout_id` | `(i: u32) -> u32` | Framed UTF-8 id of registry row `i`; `0` (`Code::IndexOutOfRange`) past the end. `gm_run`'s `layout_id` argument *is* this index — a caller finds it by scanning `0..gm_layout_count()` once at load, never a hard-coded constant. |
| `gm_build` | `(ingest_ptr: u32, ingest_len: u32) -> u32` | `(ingest_ptr, ingest_len)` must be a live `gm_alloc` allocation (C5); copies out of it, never frees it — the caller's buffer, the caller's job to free, always, even on refusal. Parses the provisional ingest JSON (below), indexes it into a topology, and returns a fresh handle, or `0` on any refusal (`IngestInvalid`, `BuildSourceInvalid`, `HandlesExhausted`). |
| `gm_run` | `(handle: u32, layout_id: u32, params_ptr: u32, params_len: u32) -> u32` | Runs registry layout `layout_id` over `handle`'s topology at its default parameters — every registered `run: fn(&Topology)` this phase takes none (C2), so `params_len` must be exactly `0`; any other value is refused (`ParamsMustBeEmpty`), never silently ignored, and `params_ptr` is never read. `1` on success, `0` on refusal. A failed run clears the handle's previous geometry first (`Code::NoGeometryYet` on the next read), so a refusal never serves a stale snapshot. |
| `gm_node_count` | `(handle: u32) -> u32` | Nodes in `handle`'s topology, available right after `gm_build`, before any run. `0` is ambiguous (empty graph vs. invalid handle) — resolved by `gm_last_error`. |
| `gm_geometry_kind` | `(handle: u32) -> u32` | Node geometry tag of the last successful run: `0` Point, `1` Circle, `2` Box (`docs/contract/binary-layout.md`). `u32::MAX` — never a real tag — before any run has succeeded. |
| `gm_edge_geometry_kind` | `(handle: u32) -> u32` | Edge geometry tag: `0` Line, `1` Polyline, `2` Curve. Beyond the phase's stated minimum surface: `gm_geometry_kind` alone only names nodes, and C3 requires edge kind to be readable too. Same `u32::MAX` convention. |
| `gm_column_ptr` | `(handle: u32, column_id: u32) -> u32` | Offset of column `column_id`'s data for the last run. `0` if the handle is invalid, there is no geometry yet, or the id is reserved/inapplicable to this run's kind — an ambiguous `0`, resolved by `gm_geometry_kind`/`gm_edge_geometry_kind` (present-but-empty vs. absent) and `gm_last_error` (invalid handle vs. no geometry). |
| `gm_column_len` | `(handle: u32, column_id: u32) -> u32` | Element count of the same column — never assumed from node/edge count, since a reserved notes column (below) will have its own length `k`. |
| `gm_snapshot_json` | `(handle: u32) -> u32` | The canonical JSON face, framed UTF-8 (`graph_contract::canonical_json::to_json`). Re-validates every coordinate as finite first (D9, C8) and refuses with `Code::TamperedGeometry` if any column view wrote a non-finite value into the handle's buffers since the last run — column views are writable aliases directly into this snapshot's storage, and nothing else re-checks. |
| `gm_snapshot_bytes` | `(handle: u32) -> u32` | The binary face, framed (`docs/contract/binary-layout.md`). Same D9 re-validation. Beyond the phase's stated minimum surface: added so `harness/wasm-run.mjs`'s hash mode can compare the real-ABI path's bytes against the retained `gm_layout_grid` shim's bytes directly (C20) — both are the binary face of the same pipeline. |
| `gm_release` | `(handle: u32)` | Releases `handle`. Ids are monotonic and never reissued (C6): using a released id again always reads `InvalidHandle`, never a later graph that happens to reuse the number. |
| `gm_last_error` | `() -> u32` | The `Code` (below) the most recent fallible call left behind; `0` (`Code::None`) after success. Read-only — polling it does not change it, so it can be checked after any other export without disturbing what it would report. |
| `gm_seed_ingest` | `(seed: u32) -> u32` | Gate-only: the hash gate's model at `seed`, framed as the same provisional ingest JSON `gm_build` reads. Not part of the published SDK surface — `harness/sdk-smoke.mjs` never calls it; only `harness/wasm-run.mjs`'s hash mode does, to drive `gm_build`/`gm_run`/`gm_snapshot_bytes` over the gate's own seeded model for C20. |

`gm_layout_count`/`gm_layout_id`/`gm_last_error`/`gm_edge_geometry_kind`/
`gm_snapshot_bytes`/`gm_seed_ingest` are all beyond the phase's literally stated minimum
surface (`gm_alloc`, `gm_free`, `gm_build`, `gm_run`, `gm_node_count`, `gm_column_ptr`,
`gm_column_len`, `gm_geometry_kind`, `gm_snapshot_json`, `gm_release`) — each is named
above with why it exists; none replaces or hides one of the ten.

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
and `hashgate::tests::stages::the_wasm_arm_can_hash_every_stage_the_gate_asks_for`
compares the two lists in both directions — every stage the gate asks for is offered by the
arm, and the arm offers nothing extra.

This is the part that must not drift: a list that grows with the registry while the arms'
bytes do not makes the gate refuse its own honest run (`native run 1 printed N lines, need
M`, or `unknown stage` from the wasm arm) the moment a second layout is registered.

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

Every export that returns a buffer (`gm_layout_id`, `gm_snapshot_json`,
`gm_snapshot_bytes`, `gm_seed_ingest`, and the retained shim's `gm_topology`/
`gm_layout_grid`) writes `[len: u32 LE][len bytes]` into the motor's own out-buffer and
returns its address; `0` means the call was refused, not "an empty buffer" (an empty
result is still framed: `[0][]`, a real nonzero address). The buffer is valid until the
next call into the module, on *any* handle — a caller copies out of it (the SDK's
`Motor#frame` does this with `.slice()`) before doing anything else.

## Ownership (C7)

| What | Owner | Freed by |
|---|---|---|
| An ingest buffer (`gm_alloc`'d) | The caller | The caller, via `gm_free` — `gm_build` only reads it |
| A framed return buffer | The motor's shared out-buffer | Overwritten by the module's next call; never explicitly freed |
| A column's `(ptr, len)` | The handle's snapshot | The handle's own storage; invalid the moment `gm_run` re-runs that handle or `gm_release` drops it |
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

A column id past 11 is `Absent`, not a panic. "Absent" (reserved, or inapplicable to
this run's geometry kind) and "present but zero-length" both read `(ptr, len) = (0, 0)`
on the wire — the two are told apart by `gm_geometry_kind`/`gm_edge_geometry_kind`, never
by treating a `0` pointer as "empty": a present-but-empty column's real pointer can be
any nonzero address a bump allocator happens to hand out, so a `ptr === 0` check alone is
not a valid presence test (`crates/graph-sdk-js/src/views.ts`'s `columnApplies` decides
presence from the geometry kind instead, mirroring this table exactly).

Ids 7 and 8 are reserved for exactly the two fields Phase 3's `notes` section (contract
0.3) brings — `note.code` and `note.index` — and they are numbered *before* 9/10/11
deliberately, so that merge fills these two slots instead of renumbering anything shipped
here. Until it does, nothing in this snapshot type has a notes section, so both resolve to
`Absent` unconditionally, for every graph, whatever the geometry kinds. Any *further*
notes field beyond those two lands at 12+, again without renumbering.

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
| 12 | `IndexOutOfRange` | An index argument (e.g. `gm_layout_id`) is past the end of its list |

`0` is both the wire's generic failure sentinel *and* a legitimate data value (an empty
graph's `gm_node_count`, an absent column's `gm_column_ptr`) — every ambiguous `0` is
documented above as resolved by `gm_last_error`, never left for a caller to guess at.

## Ingest — PROVISIONAL (C13)

**Phase 10 owns the real ingest contract.** `gm_build` takes a versioned JSON document in
`graph_core::records`' own shape, parsed by `graph_contract::canonical_json`'s strict
RFC 8259 reader (`crates/graph-wasm/src/ingest.rs`). This exists only so Phase 4 has
something concrete to build `gm_build` against.

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
- `kind` strings are matched by exact `NodeKind`/`EdgeKind` name, never the lossy
  `edge_kind_from_type` heuristic the TS oracle uses for legacy data.
- A duplicate node or edge `id`, or an edge naming a `source`/`target` not present in
  `nodes`, refuses the document outright (C12) — `graph_core::index_model` itself is
  first-wins/drop-silently for exactly these cases, which would make ingest order diverge
  from snapshot order, the one identity this ABI promises a caller.
- Not UTF-8, or not JSON at all, is refused before shape-checking even starts.
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
- `gm_layout_count`, `gm_layout_id`, `gm_last_error`, `gm_edge_geometry_kind`,
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
