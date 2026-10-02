# Perf P3-sqrt — `f64::sqrt` instead of `libm::sqrt` in graph-core

Measured 2026-10-02 on branch `perf-p3-sqrt` at `bd75ed5`, against `63a9554`. Host dlesieur42,
20 cores, load average 25–30.

## Why it is allowed

`prompt.md` §6: WASM mandates IEEE-754 for `+ - * / sqrt`, and native x86-64 SSE2 matches it.
D1 asks for libm on every *transcendental*; a square root is not one. `f64::sqrt` compiles to
`sqrtsd` natively and to `f64.sqrt` on wasm32, both correctly rounded, so both give the one
correctly rounded result. libm 0.2's `sqrt` is a software routine on wasm32: the same result,
paid for in instructions.

## Change

Every `libm::sqrt` under `crates/graph-core/src` becomes `f64::sqrt` (48 files).
`scripts/forbidden-constructs.sh` now forbids `libm::sqrt` in graph-core, and its self-test plants
one (`slow.rs`) and requires the scan to find it. The one `libm::sqrt` left in the tree is in
`crates/graph-cli/src/stress/metric.rs`, a test instrument.

## Identity

`graph-cli snapshot --seed S --nodes N --layout L --out-bin …` on `bd75ed5`; SHA-256, first 8 hex
digits, equal to the ones `perf-p3-link-once.md` records for the base:

| seed | n | `force.particle_mesh` | `force.barnes_hut` |
|---:|---:|---|---|
| 1 | 100 000 | `3d47b7d1` | `47296918` |
| 3 | 20 000 | `6c50077d` | `02da6ea5` |
| 4 | 1 000 | `baeff1b6` | `3d184107` |
| 5 | 5 000 | `453c138c` | `6ac01f26` |

The merge floor on `bd75ed5`, each through `scripts/orch/gr`: `cargo fmt --all --check` 0,
`cargo clippy --workspace --all-targets -- -D warnings` 0, `cargo test --workspace
--no-fail-fast` 0, `cargo build -p graph-core --target wasm32-unknown-unknown` 0, `hashgate
--seeds 8` 0 (four arms equal on every stage).

## Negative controls, and what the hash gate can see

Both perturb `barnes_hut/link.rs::force` on wasm32 only, right after its square root, then run
`hashgate --seeds 8`:

| perturbation of `l` on wasm32 | hashgate exit | |
|---|---:|---|
| `f64::from_bits(l.to_bits() + 1)`, one ulp | **0** | not detected |
| `l * (1.0 + 1e-6)` | 1 | `FAIL: 8 of 8 seeds diverge`, first at `layout.force.barnes_hut` |

The gate hashes the snapshot's binary face, whose coordinates are `f32`
(`docs/contract/binary-layout.md`). A one-ulp `f64` difference that the layout damps instead of
amplifying stays below `f32` resolution and is invisible to it. So the gate does not prove this
slice's bit-identity; the IEEE-754 argument above does, and the gate confirms no difference grew
to `f32` size on its 8 seeds.

Caveat: the hash gate cannot see an `f64` divergence that stays below one `f32` ulp of the output.
A gate on the `f64` positions (a hash over `Sim::x`/`y` per arm) would; it does not exist yet.

## Wall clock, wasm32 serial

`node harness/wasm-tick-bench.mjs --layout L --n N --repeat R --wasm …`, the release
`graph_wasm.wasm` of each commit, two rounds with the arm order alternating
(`target/sq/bench.sh`). `tick_ms`, the settle time over its 112 ticks:

| layout | n | repeat | base, rounds | new, rounds | change |
|---|---:|---:|---|---|---:|
| `force.particle_mesh` | 100 000 | 1 | 91.26, 90.94 | 52.71, 52.85 | −42 % |
| `force.barnes_hut` | 20 000 | 3 | 40.11, 40.34 | 26.99, 27.46 | −32 % |
| `force.particle_mesh` | 20 000 | 3 | 13.72, 11.62 | 9.03, 8.50 | −31 % |

Native is not measured here: `libm::sqrt` already lowers to `sqrtsd` on x86-64, so the change is
expected to be wasm32-only, and that expectation is unmeasured.

## Gates

The merge floor above ran on this branch. Hashgate `--seeds 1000`, the Python oracles and mutants
run once on develop: **not run** here.

## What it does not do

- **No other libm function changes.** `exp2`, `log2`, `floor`, `ceil` and the transcendentals stay
  on libm (D1); `frame.rs` still calls `libm::floor` in `place`.
- **graph-cli's stress metric keeps `libm::sqrt`**: it is outside graph-core and off the hot path.
