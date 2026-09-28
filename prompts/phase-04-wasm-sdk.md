# Phase 4 — WASM transport and the JS SDK. The motor becomes usable here.

**Read `prompt.md` first**, especially §3.2 (no wasm-bindgen) and §4.1 (two faces). Phase 3's gate must
be green.

## Goal

Expose the motor through a real zero-copy columnar WASM transport, and wrap it in a JS/TS SDK that an
application can use without knowing Rust, WASM, or anything about this repository.

**This is the phase where the project delivers its promise.** Phases 0–3 build a motor nobody can call;
Phase 4 makes it callable. Everything after this broadens the catalogue.

## Why this is the hinge

The user's requirement is that the motor be reusable — *"anyone that uses this graph nodes with the
algorithm that already exists can be parsed and be interpreted by a program front to put the look that
they want."* That is an SDK requirement, not an algorithm requirement. Five layouts behind a clean SDK
is more valuable than thirty behind none.

It is also where the performance design either holds or quietly dies. The naive version of this phase is
*slower* than the TypeScript it replaces: a JSON serialize per frame, a copy per boundary crossing, a
400 KB blob on the critical path. None of those announce themselves.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-wasm/src/{alloc.rs,exports.rs,views.rs}
crates/graph-sdk-js/{package.json,tsconfig.json,src/index.ts,src/wasm.ts,src/views.ts,src/types.ts}
crates/graph-sdk-js/README.md
harness/sdk-smoke.mjs
docs/contract/wasm-abi.md              (the exported ABI, authoritative)
docs/measurements/phase04-transport.md
```

**MODIFY — exactly these:**
```
crates/graph-wasm/src/lib.rs
crates/graph-contract/src/lib.rs       (re-export the generated TS types for the SDK)
crates/graph-cli/src/capabilities.rs
harness/wasm-run.mjs                   (drive the real ABI, not the Phase-0 shim)
package.json
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Any new layout. Anything in osionos. **No `wasm-bindgen`, no
`wasm-pack`** anywhere in the dependency tree — if either appears, the phase has gone wrong.

## Reference material

- `osionos/src/shared/notion-database-sys/src/lib/engine/bridge.ts:63-86` — **copy this loading pattern
  verbatim in structure**: module-level singleton, a deduped `initPromise`, an `initFailed` latch, a
  compile-time kill switch, and graceful degradation where every public function returns a safe default
  rather than throwing. It is the house pattern and it is correct. (osionos is read-only — read it, do
  not touch it.)
- `src/core/render/sceneState.ts:33-63` — the columnar shape the renderer already wants. The motor
  emitting columnar **removes** a conversion step rather than adding one; this is the evidence for that
  claim.
- `docs/contract/binary-layout.md` from Phase 2 — the ABI must agree with it exactly.

## Steps

### 1. Raw `extern "C"` exports — no bindgen

`docs/contract/wasm-abi.md` is authoritative. The minimum surface:

```
gm_alloc(len: u32) -> u32                  // returns an offset into linear memory
gm_free(ptr: u32, len: u32)
gm_build(ingest_ptr: u32, ingest_len: u32) -> u32     // handle
gm_run(handle: u32, layout_id: u32, params_ptr: u32, params_len: u32) -> u32
gm_node_count(handle: u32) -> u32
gm_column_ptr(handle: u32, column_id: u32) -> u32     // offset of a column's f32/u32 data
gm_column_len(handle: u32, column_id: u32) -> u32
gm_geometry_kind(handle: u32) -> u32
gm_snapshot_json(handle: u32) -> u32                  // ptr to a length-prefixed UTF-8 buffer
gm_release(handle: u32)
```

Every parameter and return is `u32` (**D6** — wasm32 is 32-bit; a `usize` in this ABI is a latent
divergence). Ownership is explicit: every `gm_alloc` has a `gm_free`, every `gm_build` has a
`gm_release`, and the ABI doc states who owns what (`refactor-common.md`: every allocation has an owner
and a free path).

### 2. Zero-copy views — the invariant that must be asserted, not assumed

The SDK constructs `Float32Array`/`Uint32Array` **views** over the wasm instance's `memory.buffer` at the
returned offsets. No copy. No JSON. No per-element loop.

**The hazard that will bite:** wasm linear memory **moves when it grows**. Every existing view is
detached and silently reads garbage or throws. So:

- views are re-derived after any call that can allocate, never cached across calls;
- the SDK exposes a documented "views are valid until the next motor call" contract;
- a test grows memory between acquiring a view and reading it, and asserts the SDK does the right thing.

This is the single most likely correctness bug in the phase and the least likely to be caught by a happy
path.

### 3. Prove zero-copy, do not claim it

Write an assertion, not a sentence. Options, pick at least one and record it:

- compare `memory.buffer.byteLength` and the view's `byteOffset` to prove the view aliases wasm memory
  rather than a copy;
- mutate one f32 through the view and observe the change from Rust on the next call.

`docs/measurements/phase04-transport.md` records the method and the result. "No serialization on the
render path" is an invariant with a test, not a design intention.

### 4. The SDK surface — small, obvious, framework-free

```ts
const motor = await createMotor();                 // lazy, deduped, degrades
const graph = motor.build(ingest);                 // ingest JSON -> handle
const geom  = graph.layout("layout.tree.tidy", { /* params */ }, { /* exec options */ });
geom.nodeCount; geom.kind;                         // "point" | "circle" | "box"
geom.x; geom.y; geom.r?; geom.w?; geom.h?;         // typed-array views
geom.toJSON();                                     // the canonical snapshot
graph.release();
```

The third argument of `layout(id, params, options)` is reserved **now** for execution options
(`options.exec`, Phase 11 — `docs/decisions/compute-tiers.md` rule 3), so adding a compute tier later is
additive and the call never changes shape. In this phase it accepts `{}` or `{ exec: "auto" }` and
refuses any other value with an error (never silently ignores it); `sdk-smoke.mjs` passes it, so it has a
live caller (§11 guardrail 2).

Constraints: **no React, no framework, no DOM**. Works in a browser, in a worker, and under Node — the
hash harness is itself an SDK consumer, which is a useful forcing function. Generated contract types are
**compile-time only**: zero runtime bytes, no validator on the hot path.

### 5. Loading, degradation, kill switch

Follow `bridge.ts:63-86`: one module-level singleton, a deduped `initPromise`, an `initFailed` latch so a
failure is not retried per call, a `__GM_DISABLE_WASM__` global kill switch, and **warn-and-degrade
rather than throw** on init failure. A motor that throws on load takes the host page down with it.

### 6. Lazy, off the critical path

The `.wasm` must be fetched on first use, not at module import. Record its byte size in
`docs/measurements/phase04-transport.md`. If it exceeds ~250 KB release-built, say so and say why rather
than letting it pass unremarked.

### 7. `harness/sdk-smoke.mjs` — the third-party test

A script that imports **only the published SDK entry point**, builds a graph from a JSON fixture, runs
each gated layout, and prints node counts and bounds. It must not import from `crates/`, must not know
the ABI, and must not reach into wasm. If it needs internals, the SDK surface is wrong.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0

# the real wasm artifact, release
docker run --rm -v "$PWD:/w" ge-rust \
  cargo build -p graph-wasm --release --target wasm32-unknown-unknown                          # 0

# no bindgen anywhere
docker run --rm -v "$PWD:/w" ge-rust sh -c \
  'cargo tree -p graph-wasm | grep -qi wasm-bindgen && exit 1 || exit 0'                       # 0

# 4-way hash equality THROUGH the real ABI
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO

# zero-copy invariant + the memory-growth hazard
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/wasm-run.mjs --assert-zero-copy    # 0

# the SDK works for a consumer that knows nothing
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                      # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

Report the release `.wasm` byte size, read from `ls -l`, not estimated.

## Ledger delta

No new layouts. Register `transport.wasm.columnar` and `sdk.js` as capabilities with their own gates, so
the ledger records that the transport itself is verified rather than assumed.

## Ponytail requirements

- **The memory-growth/view-detachment rule** is the important one: name the failing sequence concretely
  ("a view held across a call that allocates"), the direction (**silent garbage reads, the dangerous
  direction**), and the escape hatch (re-derive views after every motor call).
- The lazy loader's `initFailed` latch means a transient network failure is **not** retried for the
  process lifetime. That is a deliberate trade; name it and name the escape hatch (reload, or an
  explicit reset).
- **No marker** on the ABI itself or the column accessors — they are exact.

## Stop-and-ask

- Zero-copy cannot be demonstrated → **stop**. A copy per frame is the regression this architecture
  exists to avoid, and shipping it silently makes every later benchmark meaningless.
- The SDK smoke test needs to import from `crates/` or touch the ABI → stop; the surface is wrong.
- `wasm-bindgen` appears in the tree → stop (§3.2).
- The release `.wasm` is far larger than ~250 KB → stop and report before proceeding; investigate what
  pulled it in.
