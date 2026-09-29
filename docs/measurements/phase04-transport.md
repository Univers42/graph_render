# Phase 4 — WASM transport measurements

Every number below was run this phase (`docker run --rm -v "$PWD:/w" ge-rust ...` /
`/home/user/node-slim.sh node ...`); none is estimated. Toolchain: rustc 1.98.1
(48a229cea 2026-09-01) · node v22.23.3.

## Release artifact

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-wasm --release --target wasm32-unknown-unknown
ls -l target/wasm32-unknown-unknown/release/graph_wasm.wasm
```

```
-rwxr-xr-x 2 root root 265838 target/wasm32-unknown-unknown/release/graph_wasm.wasm
```

**265838 bytes** (~259.6 KiB), sha256
`1349dc10490caeef129f1401d42237aa570678cd2797f08a8759927be75019bf`. Re-measured on this
tree (after `develop` was merged into `p4` and after the ledger/evidence wiring, which
touches only graph-cli — the same bytes and the same digest, as expected, since
`graph-wasm` is not a dependency of `graph-cli`'s hash-gate stage list). Re-measured
earlier after the phase-04 review round (`ingest.rs`'s `node`/`edge` split to satisfy
the 40-line-per-function house limit — see `docs/contract/wasm-abi.md`'s review-response
note); 276 bytes smaller than the pre-review 266114, both figures over the same ~250 KB
soft ceiling the phase names (~4% either way) — recorded here rather than left
unremarked, per the phase's own instruction. What pulled it there in the first place:
this is the same binary carrying both the retained hash-gate shim
(`gate_exports::{gm_topology,gm_layout_grid}`, needed to keep Phase 2/3's already-green
cross-target gate hashing what it always hashed) *and* the full new ABI
(`exports/{build,columns,state}.rs`, `alloc.rs`, `handle.rs`, `ingest.rs`'s JSON parser,
`seed_ingest.rs`, `views.rs`) in one module — the JSON parser
(`graph_contract::canonical_json`, shared with the native side, not a wasm-specific
dependency) and the doubled entry points are the two things `gate_exports` alone did not
carry. (An earlier single-file `exports.rs` measured 264908 bytes; splitting it into
`exports/{build,columns,state}.rs` to satisfy the house's 300-line-per-file limit added
1206 bytes, almost certainly embedded `#[track_caller]`/panic-location path strings now
carrying the extra module segment — the pipeline's own output bytes are unaffected, as
the identical stage hashes below confirm.) No wasm-bindgen, no wasm-pack:

```sh
$ cargo tree -p graph-wasm
graph-wasm v0.1.0
├── graph-contract v0.1.0
├── graph-core v0.1.0
│   ├── graph-contract v0.1.0
│   ├── indexmap v2.14.2
│   │   ├── equivalent v1.0.2
│   │   └── hashbrown v0.17.1
│   └── libm v0.2.16
└── libm v0.2.16
```

The wasm artifact hash above matches the one `graph-cli hashgate --seeds 1000` builds
and reports independently (`hashgate: wasm artifact ... sha256
1349dc10...`) — same binary, two build invocations, same bytes.

## 4-way hash gate (native × wasm32, run × run), through `graph-cli hashgate`

```sh
$ docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000
hashgate: stages=topology,layout.grid seeds=1000 control=none
  native run 1  digest 2df5ce2bc0950e76dc6726ec83ecc85bda1a0b18cb2556d88bf3310b5df49e0d
  native run 2  digest 2df5ce2bc0950e76dc6726ec83ecc85bda1a0b18cb2556d88bf3310b5df49e0d
  wasm32 run 1  digest 2df5ce2bc0950e76dc6726ec83ecc85bda1a0b18cb2556d88bf3310b5df49e0d
  wasm32 run 2  digest 2df5ce2bc0950e76dc6726ec83ecc85bda1a0b18cb2556d88bf3310b5df49e0d
  topology: 4-way equal on 1000/1000 seeds
  layout.grid: 4-way equal on 1000/1000 seeds
  4-way equal on 1000/1000 seeds
PASS
```
Exit 0. Quoted verbatim, including the two-stage header: this run predates the transport
stage joining the gate, so it is a measurement of the `topology`/`layout.grid` pair
only. Those two stages still drive the retained `gate_exports` shim and still hash exactly
what Phase 2/3 hashed (`docs/contract/wasm-abi.md` "Deviations"); the gate now prints a
third stage beside them, measured at 8 seeds in "C20" below. The 1000-seed row itself is
the phase gate's, re-run by the orchestrator (`docs/reports/phase-04.md` names it as not
run here — a stale digest block re-labelled with the new stage list would be a report
that lies about which stage list it measured).

Negative controls, run separately, each expected non-zero:

```sh
$ GM_MUTATE_REFERENCE_DEGREE=9 cargo run -p graph-cli -- hashgate --seeds 8
  topology: 4-way equal on 0/8 seeds
  layout.grid: 4-way equal on 8/8 seeds
FAIL: 8 of 8 seeds diverge
```
Exit 1 — diverges on `topology` only, as the grid ignores weights and the reference
degree only reaches the topology stage.

```sh
$ GM_MUTATE_GRID_SPACING=999 cargo run -p graph-cli -- hashgate --seeds 8
  topology: 4-way equal on 8/8 seeds
  layout.grid: 4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
```
Exit 1 — diverges on `layout.grid` only, the mirror case.

Both control logs above are quoted from the two-stage gate, for the same reason as the
honest one. Under the current stage list the transport stage's own counts are
`4-way equal on 4/4` for the degree control and `4-way equal on 0/4` for the spacing
control (measured, and pinned by `crates/graph-cli/tests/cli.rs`'s
`each_negative_control_goes_red_on_its_own_stage`): the degree never reaches the grid, so
neither does the transport stage, and the grid's spacing reaches both.

## C20 — hash equality through the *real* ABI, inside `graph-cli hashgate`

`transport.wasm.columnar` is a stage of the hash gate itself
(`crates/graph-cli/src/hashgate/stages.rs`), so the real ABI is compared inside a gate
that already runs rather than in a hand-run harness: the wasm arm drives
`gm_seed_ingest → gm_alloc → gm_build → gm_run → gm_snapshot_bytes` over the same
seeded model `layout.grid` uses, and `hashgate/transport.rs` counts the seeds where that
snapshot equalled the retained shim's.

**Both arms derive their stage list from the registry, and that is pinned.** The gate's
list is `graph_core::registry::LAYOUTS`; the wasm arm resolves any stage that is not one of
its three shim-backed ones through `gm_layout_count`/`gm_layout_id`. A stage list that grew
with the registry while the arms' bytes did not would make the gate refuse its own honest
run the moment a second layout was registered, which is exactly what a temporary second
registry row reproduced and this fix removes — see `docs/reports/phase-04.md` §6a.

```sh
$ GM_GATES_DIR=/w/target/tmp-gates cargo run -p graph-cli -- hashgate --seeds 8
hashgate: wasm artifact .../graph_wasm.wasm sha256 1349dc10...
hashgate: stages=topology,layout.grid,transport.wasm.columnar seeds=8 control=none
  native run 1  digest 0992397c6155a309c12cf42c04a793085077826a05b69ba193c8315d0c8a0ec7
  native run 2  digest 0992397c6155a309c12cf42c04a793085077826a05b69ba193c8315d0c8a0ec7
  wasm32 run 1  digest 0992397c6155a309c12cf42c04a793085077826a05b69ba193c8315d0c8a0ec7
  wasm32 run 2  digest 0992397c6155a309c12cf42c04a793085077826a05b69ba193c8315d0c8a0ec7
  topology: 4-way equal on 8/8 seeds
  layout.grid: 4-way equal on 8/8 seeds
  transport.wasm.columnar: 4-way equal on 8/8 seeds
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 8/8 seeds
PASS
```
Exit 0. All four arms print one digest for the whole run, so the transport stage is
4-way equal by construction of the comparison, not by a hand-checked list.

The record it writes (`target/tmp-gates/hashgate.json`, a scratch gates directory so the
real `target/gates` is not overwritten by a short run):

```json
{
  "equal": { "layout.grid": 8, "topology": 8, "transport.wasm.columnar": 8 },
  "pass": true, "seeds": 8, "mutation": null,
  "transport": { "equal": 8, "reference": "layout.grid", "stage": "transport.wasm.columnar" }
}
```

The negative control that backs the transport row's 4-way verdict is the grid's own,
because the transport stage restates the grid's bytes natively:

```sh
$ GM_MUTATE_GRID_SPACING=2 cargo run -p graph-cli -- hashgate --seeds 8
  topology: 4-way equal on 8/8 seeds
  layout.grid: 4-way equal on 0/8 seeds
  transport.wasm.columnar: 4-way equal on 0/8 seeds
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
FAIL: 8 of 8 seeds diverge
```
Exit 1. The C20 tally stays at 8/8 there, as it must: both sides of that comparison are
wasm-side, and the control perturbs the native arm only (`prompt.md` §7.2). The row's
`hash_4way` reads its control from the *stage* count, which is 0/8.

`harness/wasm-run.mjs` still asserts the same per-seed equality itself and exits `1` on
the first divergence, so the check survives a hand run of the harness as well as inside
the gate. The 1000-seed row is the phase gate's own (`docs/reports/phase-04.md` names it
as not run here).

`capabilities --check` against that scratch record refuses the transport row for its seed
count, exactly as it refuses every other gated row at 8 seeds:

```
  transport.wasm.columnar: gated, but hashgate ran 8 seeds, need 1000
  transport.wasm.columnar: gated, but hashgate ran 8 seeds, need 1000
capabilities --check: 11 rows, 20 problems
```
Exit 1 — two verdicts, one per column, both refused for the same honest reason.

## The SDK over its published entry point (`harness/sdk-smoke.mjs`)

```sh
$ node --experimental-strip-types harness/sdk-smoke.mjs \
    target/wasm32-unknown-unknown/release/graph_wasm.wasm
ok - an unknown options key is refused
ok - options.exec other than "auto" is refused
ok - build reports the right node count
ok - the SDK publishes the module's layout registry (C1)
ok - the registry names layout.grid and repeats no id
# layout.grid: 2 nodes, Point nodes / Line edges, bounds x[-0.5, 0.5] y[0, 0]
ok - layout.grid: every node is placed
ok - layout.grid: its bounds are finite and not a single point
ok - layout.grid: its columns match the contract's presence table
ok - layout.grid: its JSON face carries the same nodes
ok - every registered layout ran through the published SDK
ok - toJSON succeeds before any tamper
ok - writing through a column view does not itself move the epoch
ok - a NaN written through a column view refuses toJSON (D9)
ok - release moves the epoch forward
ok - a released handle is refused, not silently answered (C6)
ok - createMotor_never_throws_on_kill_switch
ok - degraded_motor_build_fails_predictably_kill_switch
ok - degraded_motor_layouts_fails_predictably_kill_switch
ok - createMotor_never_throws_on_compile_failure
ok - degraded_motor_build_fails_predictably_compile_failure
# pass
```
Exit 0, 20 checks, on the one-layout registry this branch has. The `#` line is the
per-layout report step 7 asks for (node count, both geometry kinds, bounds), printed for
**every** layout `Motor#layouts` returns rather than for a name the script carries; with
this branch's single layout that is `layout.grid`, whose own two-node fixture lands the
nodes at x ∈ [-0.5, 0.5] and y = 0 (one row of a 2-column lattice, `cols = ceil(sqrt(2))`).

The same run against a temporary three-row registry — the grid plus a Circle/Polyline and a
Box/Curve row, added and reverted for the purpose, see `docs/reports/phase-04.md` §6b —
printed one such line per layout and asserted the contract's whole column table for each,
including the `r`, `w`, `h`, `offsets`, `pts` and `degree` columns that the one-layout tree
never reaches. That probe is evidence about the checks, not a measurement of this tree: the
rows are not in the delivered tree and no number here depends on them.

## Zero-copy and memory-growth hazard (C8, C10, C11)

```sh
$ node --experimental-strip-types harness/wasm-run.mjs \
    target/wasm32-unknown-unknown/release/graph_wasm.wasm --assert-zero-copy
ok - a motor call moves the epoch forward (C10)
ok - a write through a column view reaches the encoded JSON face (zero-copy, C8)
ok - a large enough build really does grow wasm memory
ok - the pre-growth view's old buffer is detached, not silently stale (C10)
ok - re-deriving the same column after growth returns a live, non-empty view
ok - the re-derived view still reads the value written before growth
# view re-derivation: 137.8 ns/call over 200000 calls
# pass
```
Exit 0. Re-run on this tree; the check list and every verdict are unchanged, only the
timing differs (§"View re-derivation cost" below, where a second run of this same row reads
152.0 ns/call).

Method (C8, "prove zero-copy, do not claim it"): a finite sentinel (`918273.5`, not
`NaN` — that would be refused at encode time by D9, proving tamper detection, not
aliasing) is written through a `Motor#column` typed-array view; `Motor#toJSON`'s encoded
output is then asserted to contain that exact value. Only a real alias into the memory
the encoder reads from can make a JS-side write reach the encoder's output; a copying
implementation could not pass this.

Method (C10, the growth hazard): a view is held from a small (3-node) graph, then a
200,000-node graph is built on the *same* motor instance to force `WebAssembly.Memory`
to actually grow. The held view's `.buffer.byteLength` is asserted to become `0`
(detached — the JS engine's own observable behaviour on growth, not something the SDK
has to simulate), and re-deriving the same column afterward is asserted both live
(non-zero `byteLength`, correct `length`) and still holding the sentinel written before
growth — proving Rust-side data survives growth unmoved while the JS-side view correctly
rebuilds rather than reading stale or garbage bytes.

**View re-derivation cost (C11): 137.8 ns/call**, measured over 200,000 calls to
`Motor#column` on a 200,000-node graph's `NodeX` column (`process.hrtime.bigint()` around
the loop, wall time only — no warm-up discarded, so this includes one-time JIT warm-up cost
amortized over the run). Re-measured on this tree (after `develop` was merged into `p4` and
after the ledger/evidence wiring, which touches no SDK code); a second run of the identical
row on the same tree read **152.0 ns/call**, and later ones **160.3** and **169.0 ns/call**
(the SDK change since touches `index.ts` only, never this loop). Earlier runs in the same
phase measured 228.5, 225.6 and 221.7 ns/call for the identical loop. None of the changes
between any of them touched the hot loop itself (`ColumnViews#get`'s cache check), and two
runs of the *same* binary differ by 10%, so the spread is container scheduling noise on a
wall-clock measurement, not a per-call regression — recorded as the actual numbers observed
rather than picking one to report.

An initial version of this same test had a real, observed bug (not merely a
possibility): the sentinel was written *before* a second `layout()` call made for the
epoch-advance check, and `layout.grid` recomputes node positions from scratch on every
run — so the second run silently overwrote the sentinel before the growth-survival
assertion ever read it back, and `re-deriving the same column after growth returns a
live, non-empty view` passed while `the re-derived view still reads the value written
before growth` failed:
```
not ok - the re-derived view still reads the value written before growth
# 1 failed
```
exit 1, observed once, before the sentinel write was moved to after the last mutating
call on that handle. This was a test-ordering bug in the harness, not a defect in
`crates/graph-sdk-js` — the SDK's own column-view/epoch code did not change to fix it.

## Provisional-ingest pipeline memory (native; `TRANSPORT_CEILING`'s basis)

```sh
$ cargo test --release -p graph-wasm --lib -- --ignored --nocapture provisional_ingest
```

Peak heap (`crates/graph-wasm/src/memory_measure.rs`, a counting `#[global_allocator]`
scoped to this one test) through `ingest::read → index_model → layout.grid's run →
layout::snapshot → to_bytes()`, native (not wasm32 — wasm32 has no comparable
peak-allocator hook available without pulling in the trap this measurement exists to
avoid; the wasm32 number is projected, per the ledger row's own Ponytail):

| n (nodes) | m (edges) | ingest text bytes | snapshot bytes | peak heap bytes | peak / node |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 1,541 | 435,232 | 61,804 | 2,675,886 | 2,675.9 B |
| 10,000 | 15,474 | 4,429,009 | 644,668 | 27,480,101 | 2,748.0 B |
| 100,000 | 154,978 | 44,993,585 | 6,707,248 | 286,233,645 | 2,862.3 B |

Peak/node rises with `n` (more edges per node at this seed's edge-generation rate, and
allocator overhead does not scale away): the largest measured ratio, 2,862.3 B/node, is
the conservative one carried forward. `crates/graph-cli/src/capabilities/registry.rs`'s
`TRANSPORT_CEILING = 1_500_000` is `4 GiB / 2862.3 B ≈ 1,500,511`, rounded down to two
significant figures — wasm32's actual linear-memory ceiling, not an arbitrary round
number. Ponytail (`registry.rs`): projected onto wasm32, not independently re-measured
there; no escape hatch this phase — Phase 10's real ingest contract may cost
differently and would need its own measurement.

## SDK typecheck (report-only, no gate row — C23)

```sh
$ npx tsc -p crates/graph-sdk-js/tsconfig.json --noEmit
```
Exit 0, zero diagnostics.

## Not run this phase (named, not silently skipped)

- **Browser execution.** Every check above ran under Node (`node:22-slim` /
  `--experimental-strip-types`); no browser (Chromium, Firefox, WebKit) instantiated
  `graph_wasm.wasm` or loaded the SDK this phase. `WebAssembly.instantiateStreaming`'s
  MIME-fallback path in `wasm.ts` is written for a browser `fetch` response but was only
  exercised by `compile()`'s string/URL branch against a `file://`-style local read
  under Node, not against a real HTTP server or a real browser's streaming compiler.
  UNKNOWN, not assumed working.
- **wasm32-target peak-memory measurement.** The `TRANSPORT_CEILING` table above is
  native; nothing measured wasm32's own peak allocator use directly (see Ponytail
  above).
- `docker build -t ge-check . && docker run --rm ge-check` — **run** on the tree this report
  describes (27 passed, 0 failed, 0 skipped, 0 cancelled, 0 todo). It was not run during the
  ledger/evidence session; it is recorded here because this file claims to hold every number
  this phase measured, and leaving a stale "not run" beside a row that has since run would
  be a report that lies about its own coverage.
- **`graph-cli hashgate --seeds 1000` on the current stage list.** The transport stage
  was measured at 8 seeds (its honest run, its control, and the record both write, all
  quoted above) and inside `cargo test`'s own 4-seed integration run; the 1000-seed row
  is the phase gate's and was not run here, so the transport row's `gated` claim is
  proven against a 4/8-seed record and not against a 1000-seed one. UNKNOWN, not assumed.
  The 1000-seed log quoted above is the earlier two-stage run, kept verbatim and labelled
  as such rather than re-labelled.
