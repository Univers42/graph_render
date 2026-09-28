> **Status (2026-09-28):** DONE — merged into develop 39d2450; gate green, mutants 512 caught / 0 missed (docs/reports/phase-02.md). See docs/reports/STATUS.md.

# Phase 2 — Geometry contract, the stage registry, and one layout end to end

**Read `prompt.md` first**, especially §4 (the geometry vocabulary). Phase 1's gate must be green.

## Goal

Turn the geometry vocabulary into real types and a real registry, then drive the **simplest possible
layout** — a grid — all the way through the pipeline to a hashed snapshot and a JSON file. Prove the
whole spine works before any interesting mathematics enters it.

## Why this phase exists

Grid layout is ~10 lines and nobody wants it. That is exactly why it goes first: it isolates every
failure to the *plumbing*. When the first real algorithm lands in Phase 3 and a hash mismatches, the
pipeline, the registry, the serializer, the round-trip and the gate are already known-good, so the
mismatch is in the algorithm. Skipping this phase means debugging four things at once.

The second reason is the contract itself. The geometry vocabulary is **the breaking surface** — get it
wrong and every stored snapshot and every frontend has to be migrated later. SciGraphs demonstrates the
exact failure: its layout contract is a bare `(num_nodes, 3)` array (`common.py:175-185`), so when Circle
Packing needed radii they were smuggled into a side-channel Blender attribute
(`circle_packing.py:281`) that nothing else reads. Phase 2 is where we avoid inheriting that.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-contract/src/{version.rs,binary.rs,canonical_json.rs}
crates/graph-core/src/stage.rs              (the Stage trait + pipeline driver)
crates/graph-core/src/registry.rs           (capability registry)
crates/graph-core/src/layout/mod.rs
crates/graph-core/src/layout/grid.rs
crates/graph-cli/src/snapshot_cmd.rs
docs/contract/binary-layout.md              (the byte layout, authoritative)
docs/contract/snapshot-schema.json          (generated, committed)
```

**MODIFY — exactly these:**
```
crates/graph-contract/src/{lib.rs,geometry.rs,snapshot.rs}
crates/graph-core/src/lib.rs
crates/graph-cli/src/{main.rs,capabilities.rs,hashgate.rs}
harness/wasm-run.mjs                        (hash the real snapshot now, not the Phase-0 stub)
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Any layout other than grid. Anything in osionos.

## Steps

### 1. The geometry types — closed set, per-snapshot discriminant

Per `prompt.md` §4. The discriminant is **one tag for the whole snapshot**, not one per element. This is
not a micro-optimisation: a per-element tag would break SoA purity and therefore break the zero-copy
transport that Phase 4 depends on.

Node kinds: `Point{x,y,(z)}` · `Circle{x,y,r}` · `Box{x,y,w,h}`.
Edge kinds: `Line{}` (zero bytes) · `Polyline{offsets,pts}` · `Curve{offsets,pts,degree}`.
Reserved with allocated tag numbers, **no implementation**: `Ribbon`, `Arc`.

`Polyline`/`Curve` point storage is **CSR-shaped** — `offsets: Vec<u32>` (m+1), `pts: Vec<f32>`. Same
structure as the adjacency, same reasoning, and it means one mental model for the whole codebase.

### 2. Versioning that actually refuses

Major/minor. A reader **refuses** a major above what it knows, with a clear error naming both versions.
Copied from `graph_animation.py:264-280`, which gets this right; a missing header there defaults to
version 0 rather than crashing, and that graceful-default behaviour is worth copying too.

Write a test that constructs a snapshot with `major + 1` and asserts the reader rejects it. A version
field nothing validates is decoration.

### 3. The binary face — `docs/contract/binary-layout.md` is authoritative

Columnar, little-endian, explicit field order, explicit padding/alignment rules. Document it as a byte
table. **The hash is taken over these bytes**, so any ambiguity in this document is a latent
cross-target divergence.

Rules: every integer explicitly `u32`/`u64`, **never `usize`** (D6). No NaN/Inf may be hashed (D9) —
assert and fail loudly.

### 4. The canonical JSON face

Sorted keys, stable float formatting, deterministic array order. This is **the universal contract the
project exists to deliver** — the thing any third-party frontend reads.

**The round-trip must be byte-exact:** `binary → JSON → binary` reproduces identical bytes. Test it over
the full seed sweep, not one example. Rust's float `Display` gives shortest-round-trip, so this holds
exactly (`prompt.md` §4.1) — but prove it rather than trusting the reasoning.

### 5. The `Stage` trait and the pipeline

A stage takes an immutable topology plus its own params and produces (or extends) geometry. Pure: no
`&mut` on shared state across stages, no interior mutability, no time (D8).

**Each stage's output is hashed separately.** That is the whole point of staging: a divergence names the
stage. Wire this into `hashgate` now, while there is exactly one stage and it is trivial.

### 6. The registry

Maps a capability id (`layout.grid`) to its implementation plus its metadata: tier, stage, geometry kind,
oracle, complexity, `scale_ceiling`, `degradation`, `ponytail`. The metadata fields are **non-optional**,
so a capability cannot be registered without declaring them — that is how `capabilities --check` stays
honest instead of relying on discipline.

### 7. Grid layout

Reference: `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py`. Deterministic, index-ordered, `Point`
geometry, O(n). Roughly: `cols = ceil(sqrt(n))`, then `(i % cols, i / cols)` scaled.

Two decisions to make explicitly and record: the aspect ratio / spacing convention, and whether the grid
is centred on the origin. Both are arbitrary; both must be *stated* so they are stable, because a
snapshot hash pins them forever.

### 8. `graph-cli snapshot`

Emit both faces for a seed:
`snapshot --seed S --layout grid --out-bin x.bin --out-json x.json`.
This is the command a third-party consumer would use, so its ergonomics matter more than its internals.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

# per-stage 4-way hash equality, now over a real snapshot
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO

# binary <-> JSON round-trip is byte-exact over the sweep
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- roundtrip --seeds 1000           # 0

# a rejected future version
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-contract version_refusal               # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim npm run oracle:diff                             # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

Also: emit one `snapshot.json` for seed 1 at N=50 and **paste it into the report**. It is small, and it
is the first concrete evidence of the deliverable the whole project is for.

## Ledger delta

`layout.grid` reaches `gated`: tier 1, stage `layout`, geometry `Point`, complexity `O(n)`, oracle
`hand` (there is no meaningful third-party grid oracle — say so rather than inventing one).

## Ponytail requirements

- Grid's aspect-ratio choice is a **convention, not a computation**. If it uses `ceil(sqrt(n))` it
  produces a ragged last row; name that and name the direction (cosmetic, never wrong).
- **No marker** on the serializer, the round-trip, or the version check — they are exact, and
  `ponytail.md` forbids markers on deterministic code.

## Stop-and-ask

- The round-trip is not byte-exact → **stop**. Do not add a tolerance. A lossy JSON face breaks the
  project's central promise, and an epsilon here would hide it.
- You need a per-element geometry tag to express something → stop. That breaks SoA and Phase 4's
  zero-copy transport. The vocabulary needs a new *kind*, which is a contract decision, not an
  implementation one.
- A stage seems to need mutable shared state or wall-clock time → stop (D8).
