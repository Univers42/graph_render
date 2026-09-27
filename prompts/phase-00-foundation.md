# Phase 0 — Foundation: toolchain, contract skeleton, and an instrument that can fail

**Read `prompt.md` first.** Rules 0.1–0.7 apply. This file adds only what is specific to Phase 0.

## Goal

Stand up the Rust toolchain image, the cargo workspace, the contract crate, and — most importantly —
**a gate that is proven capable of failing**. No algorithm is written in this phase.

## Why this phase exists

Every later phase's claim of correctness rests on the hash gate. If the gate is built alongside the
first algorithm, there is no moment at which it is tested against a known-wrong input, and a gate that
has never gone red is indistinguishable from a gate that cannot. So the instrument is built first, and
its **negative control** is part of this phase's definition of done.

The second reason: `D1` in `prompt.md` §6 asserts that `std`'s transcendentals differ between native and
wasm32. That assertion is **unmeasured**. Phase 0 measures it. If it turns out `std` agrees, the `libm`
constraint still stands as cheap insurance, but the recorded reason becomes a number instead of a claim.

## Authorization envelope

**CREATE — exactly these:**
```
docker/rust.Dockerfile
Cargo.toml                       (workspace root)
crates/graph-contract/{Cargo.toml,src/lib.rs,src/geometry.rs,src/snapshot.rs}
crates/graph-core/{Cargo.toml,src/lib.rs}          (skeleton only — no algorithms)
crates/graph-wasm/{Cargo.toml,src/lib.rs}          (one numeric export, to prove the arm works)
crates/graph-cli/{Cargo.toml,src/main.rs,src/hashgate.rs,src/capabilities.rs,src/determinism_probe.rs}
harness/wasm-run.mjs             (the Node-side wasm arm)
docs/measurements/d1-ln1p.md     (the D1 measurement, committed)
scripts/guard-osionos.sh         (the read-only invariant, with an owner at last)
scripts/osionos-baseline.txt     (the committed dirty-state snapshot the guard compares against)
.gitignore                       (add /target)
```

**MODIFY — exactly these:**
```
package.json                     (add the `oracle:diff` script stub only)
opencode.json                    (close the deny-list prefix gap — step 9)
```

**FORBIDDEN in this phase:** any file under `src/`, `tests/`, `verify/`. Any layout algorithm. Any
change to the existing `Dockerfile`. Anything in osionos.

## Reference material

- `./Dockerfile` — match its header-comment convention and its `npm ci` reasoning style.
- `prompt.md` §6 (D1–D9) — the determinism constraints the contract types must respect (especially
  **D6: never `usize` on the wire**).
- `prompt.md` §4 — the geometry vocabulary the contract crate encodes.
- `src/core/types.ts` — the shape `graph-contract` mirrors for the topology half.

## Steps

### 1. `docker/rust.Dockerfile`

- Base **`debian:trixie-slim`**. Not `rust:*` (rule 0.3).
- Install rustup with `--profile minimal`, **pinned to an exact stable version** — check what rustup
  actually offers and hard-code a real one; do not guess a version number.
- Add target `wasm32-unknown-unknown`.
- `ENV CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup` and put them on `PATH`. This matters: `/w` is a
  bind mount, and a `CARGO_HOME` under the workdir gets shadowed at runtime.
- Remove apt lists; do not leave a cargo registry cache in the final layer.
- `WORKDIR /w`, `CMD ["cargo","--version"]`.

### 2. Cargo workspace

Root `Cargo.toml` with the five members. Set `resolver = "2"`.

In `crates/graph-core/Cargo.toml`, the dependency list is **closed**: `libm`, `indexmap`. (`petgraph`
arrives in Phase 7, not now.) No `serde` in `graph-core` — serialization belongs to `graph-contract`.

### 3. `graph-contract` — types only, no logic

Encode `prompt.md` §4 exactly:

- The node geometry discriminant (`Point | Circle | Box`) and the edge geometry discriminant
  (`Line | Polyline | Curve`), **per-snapshot, not per-element**, with explicit tag numbers and the
  reserved tags for `Ribbon` and `Arc` allocated but unimplemented.
- A **format version** with a major/minor split, and a reader that **refuses a major above what it
  knows**.
- A reserved `z` channel and a reserved stage count.
- **Every integer that reaches the wire is explicitly `u32`/`u64` (D6).** Audit this; it is the
  constraint most likely to be violated by reflex.
- Codegen to TypeScript types and JSON Schema, committed and diffable. Generated artifacts must add
  **zero runtime bytes** — types only, no validator.

### 4. The binary snapshot format and its hash

- Columnar, little-endian, explicit field order. Document the byte layout in a comment block — this is
  the thing the hash is taken over, so ambiguity here is a latent divergence.
- Hash with an **explicit algorithm** (BLAKE3 or SHA-256). **Never `DefaultHasher`** (D7).
- **Assert no NaN/Inf before hashing** (D9) and fail loudly rather than hashing a NaN whose bit pattern
  WASM does not mandate.

### 5. The hash harness — `graph-cli hashgate`

Four artifacts per seed, per stage (`prompt.md` §7.1): native ×2, wasm32 ×2. The wasm arm runs under
Node via `harness/wasm-run.mjs`, using plain `WebAssembly.instantiate` — no wasm-bindgen, no wasm-pack.
In this phase there is no real stage yet, so hash a **trivial deterministic synthetic buffer** produced
by `graph-core`; the point is the plumbing, not the content.

### 6. The negative control — this is the deliverable that matters most

Read an env var (`GM_MUTATE_REFERENCE_DEGREE`) that perturbs one constant used by the synthetic buffer.
With it set, `hashgate` **must exit non-zero**. Wire it so the mutation is genuinely inside the hashed
computation — a mutation that bypasses the hash proves nothing.

### 7. The capabilities ledger — `graph-cli capabilities`

Implement `--json` and `--check` per `prompt.md` §8. In this phase the registry is empty, so `--json`
emits `[]` and `--check` exits 0. The required fields (`scale_ceiling`, `degradation`, `ponytail`) must
already be **non-optional in the type**, so a later phase cannot add a capability without them.

### 8. The D1 measurement — `graph-cli determinism-probe`

Compute `ln_1p` (and `ln`, `exp`, `sin`, `cos`, `pow`, `atan2`) over a **fixed** input sweep — include
denormals, values near 1.0, large magnitudes, and negatives where defined — twice: once via `std`, once
via the `libm` crate. Run natively and under wasm32. Compare **bit patterns**, not values.

Write `docs/measurements/d1-ln1p.md` with: the exact sweep, the command, the raw counts of differing
bit patterns per function per target pair, and the conclusion. If `std` and `libm` agree everywhere,
say so — that is a valid and useful result, and the `libm` constraint stays as insurance with the
measurement as its recorded reason.

### 9. `scripts/guard-osionos.sh` — rule 0.1 finally gets an enforcement owner

Rule 0.1 says osionos is read-only. Until now nothing enforced it: the wrapper that checked the invariant
existed only in a session scratchpad, and **a rule whose only enforcement lives in a temp directory
expires with the session.** Phase 0 promotes it.

It lives in **graph-engine**, not osionos — writing the guard into the tree it protects would violate the
rule on its first commit.

Three requirements, each from a defect in the scratchpad version:

1. **Compare against a committed baseline, not against "clean".** osionos is *already dirty*: a staged
   `.claude`, plus untracked `AGENTS.md`, `opencode.json`, `prompt_opencode*.md`, `.opencode/`,
   `.playwright-mcp/`. A guard that demands a clean tree fires on its first run and gets switched off,
   which is worse than no guard. Capture the baseline once into `scripts/osionos-baseline.txt` and diff
   against that.
2. **Hash contents; do not read status codes.** The scratchpad version diffed `git status --porcelain`,
   which is **blind to further edits of an already-dirty file** — a file listed `??` or ` M` keeps the
   same two characters no matter how its contents change. Since every interesting file in osionos is
   already dirty, that blind spot covers exactly the files most at risk. Hash the tracked **and**
   untracked set instead.
3. **Exit 90 on any change**, distinct from 1, so a wrapper can tell "the invariant broke" from "the
   command failed".

Interface: `guard-osionos.sh --snapshot` writes the baseline; `--check` verifies; `-- <cmd…>` snapshots,
runs, re-checks, and returns 90 if the invariant broke even when the command itself succeeded.

**Also close the deny-list prefix gap in `opencode.json`.** Its patterns are prefix-matched, so
`git -C <path> push` slips straight past a `git push*` rule. Add `git commit`, `git -C * push`,
`git switch`, `git merge`, `git submodule*`. This matters because `"ask"` was *measured* to execute
silently in headless runs, so the deny-list is the only real barrier.

## Gate — every command, with its expected exit code

```sh
# the image exists and carries both targets
docker build -f docker/rust.Dockerfile -t ge-rust .                                   # 0
docker run --rm ge-rust cargo --version                                               # 0
docker run --rm ge-rust rustc --print target-list | grep -qx wasm32-unknown-unknown    # 0

# graph-core purity: it must compile for BOTH targets
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core                         # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

# strict gates
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                 # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings            # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                             # 0

# the gate is green on an unmutated tree
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 100      # 0

# THE ONE THAT MATTERS: the gate can fail
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                          # NON-ZERO

# ledger plumbing
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check      # 0

# the D1 measurement produced a file with real numbers in it
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- determinism-probe          # 0

# the existing TypeScript gate is untouched and still green
docker build -t ge-check . && docker run --rm ge-check                                   # 0

# the osionos read-only guard works in BOTH directions — a guard that cannot fire is not a guard
bash scripts/guard-osionos.sh --snapshot                                                 # 0
bash scripts/guard-osionos.sh --check                                                    # 0
#   then, as a deliberate self-test: append a byte to an ALREADY-DIRTY osionos file
#   (this is the case `git status --porcelain` cannot see), re-check, and REVERT it.
#   --check must return 90 while the byte is present.                                     # 90
bash scripts/guard-osionos.sh --check                                                    # 0  (after revert)
```

The guard self-test is the one command in this phase that must be run **and then undone**. Report the
exact file touched and the diff proving it was reverted — a self-test that leaves a mark has broken the
invariant it was testing.

Also report the final `ge-rust` image size, read from `docker images`, not estimated.

## Ledger delta

Empty → empty. No capability is registered in Phase 0. `capabilities --check` exits 0 by virtue of
having nothing to check, and the report must **say that explicitly** rather than presenting a green
check as evidence of coverage (rule 0.5, and `quality-bar.md`: skipped ≠ passed).

## Ponytail requirements

- The D1 probe is a **sampler**: it tests a chosen sweep, not the whole f64 domain. Its marker must name
  that, and name the direction — a sweep that misses a divergent input **under-reports**, which is the
  dangerous direction.
- The binary format's NaN assertion is exact, not heuristic. **Do not** put a marker on it
  (`ponytail.md`: a marker on deterministic code trains readers to skip markers).
- **`guard-osionos.sh` is a sampler over a file set**, so it owes a marker naming what it cannot see:
  paths excluded by osionos's `.gitignore` (`node_modules/`, `build/`, `.env`) are outside the hashed set,
  so a write there passes the guard. Direction: **under-reporting — the dangerous one.** Escape hatch: a
  `--include-ignored` mode, slow enough that it is not the default.

## Stop-and-ask

- rustup does not offer the version you pinned → ask, do not silently pick another.
- `graph-core` will not compile for `wasm32-unknown-unknown` without a dependency not on the closed
  allow-list → **stop**. Do not add to the allow-list.
- The negative control passes (exits 0) when it should fail → **stop and report it**. That is the most
  important finding this phase can produce, and it invalidates every later phase until fixed.
