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
`1349dc10490caeef129f1401d42237aa570678cd2797f08a8759927be75019bf`. Re-measured after the
phase-04 review round (`ingest.rs`'s `node`/`edge` split to satisfy the 40-line-per-function
house limit — see `docs/contract/wasm-abi.md`'s review-response note); 276 bytes smaller
than the pre-review 266114, both figures over the same ~250 KB soft ceiling the phase
names (~4% either way) — recorded here rather than left unremarked, per the phase's own
instruction. What pulled it there in the first place: this is the same binary carrying
both the retained hash-gate shim (`gate_exports::{gm_topology,gm_layout_grid}`, needed to
keep Phase 2/3's already-green cross-target gate unchanged) *and* the full new ABI
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
Exit 0. This arm still drives the retained `gate_exports` shim, unchanged from Phase 2/3
(`docs/contract/wasm-abi.md` "Deviations").

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

## C20 — hash equality through the *real* ABI

`harness/wasm-run.mjs`'s `hash` mode gained a third stage, `transport.wasm.columnar`,
driving `gm_seed_ingest → gm_alloc → gm_build → gm_run → gm_snapshot_bytes` — the real
ABI, not the retained shim — over the same seeded model `layout.grid` uses. When both
stages are named in one invocation, the harness now also asserts their digests match per
seed and exits `1` on the first divergence, rather than only printing two lists a human
would have to compare by eye.

```sh
$ node harness/wasm-run.mjs target/wasm32-unknown-unknown/release/graph_wasm.wasm \
    hash 1000 topology layout.grid transport.wasm.columnar
[3000 stage/seed/sha256 lines]
wasm-run: C20 ok — transport.wasm.columnar == layout.grid on all 1000 seeds
```
Exit 0, real time 5.3s for 1000 seeds × 3 stages. Sample (seed 0):
```
layout.grid              0 ef1a701c2a105a4525ebb2b920ee60e729eb9f3e3619dab58b9f2a33412935de
transport.wasm.columnar  0 ef1a701c2a105a4525ebb2b920ee60e729eb9f3e3619dab58b9f2a33412935de
```

The failure path was also exercised, not just assumed: a one-seed corruption injected
into `transport.wasm.columnar`'s stage function (reverted immediately after) produced

```
wasm-run: C20 FAIL — transport.wasm.columnar diverges from layout.grid at seed 2
  (layout.grid 9a71ed..., transport.wasm.columnar 616055...)
```
exit 1, confirming the check actually fires rather than only ever printing "ok".

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
# view re-derivation: 228.5 ns/call over 200000 calls
# pass
```
Exit 0.

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

**View re-derivation cost (C11): 228.5 ns/call**, measured over 200,000 calls to
`Motor#column` on a 200,000-node graph's `NodeX` column (`process.hrtime.bigint()` around
the loop, wall time only — no warm-up discarded, so this includes one-time JIT
warm-up cost amortized over the run). Earlier runs (pre-split `exports.rs`, and again
just after the phase-04 review round's `index.ts`/`views.ts` changes) measured 221.7 and
225.6 ns/call for the identical loop; none of those changes touched the hot loop itself
(`ColumnViews#get`'s cache check), so the spread is container scheduling noise on a
wall-clock measurement, not a real per-call regression — recorded as the actual number
observed on this run rather than picking one to report.

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
- `docker build -t ge-check . && docker run --rm ge-check` — not run this session per
  explicit instruction (the orchestrator runs it). SKIP, not a pass.
