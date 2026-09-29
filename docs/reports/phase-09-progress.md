# Phase 9 progress — slice 1: the native scale/bench harness

**Status:** partial. Branch `p9`, based on develop (p3 + p5 + p6e + p6f + p7). **p4 is not merged**, so
everything that needs the WASM ABI or the JS SDK is blocked and is listed below rather than faked.

**Machine class:** whatever the `ge-rust` container ran on, this session. Not portable; the harness is a
sampler of one machine, one seed, one `reference_degree`.

## What landed (TDD, RED before GREEN)

| Step | RED | GREEN |
|---|---|---|
| `bench/campaign.rs` stubs (`median`, `settle_ms`, `largest_fitting`) | 3 tests panicked `not implemented` (6 passed, 3 failed) | 9 passed in `bench::tests` |
| `Plan` gains `repeat`/`out`/`crossover`/`budget_ms`; `main.rs` flags | — | builds; `cargo test -p graph-cli bench` green |

Created: `crates/graph-cli/src/bench/campaign.rs` (a **deviation** from the envelope, which lists
`crates/graph-cli/src/bench.rs`: `bench.rs` is already 192 lines and the 300-line house limit leaves no
room for the campaign). Modified: `crates/graph-cli/src/bench.rs`, `crates/graph-cli/src/bench/tests.rs`,
`crates/graph-cli/src/main.rs`. Created: `docs/measurements/phase09-bench.md`,
`docs/measurements/phase09-crossover.md`.

## What the slice measures (and does not)

Measured natively, medians over `--repeat`: build ms, tick ms, settle ms (tick × 112), **columns and
arena separately**, snapshot bytes in both faces, and the largest N fitting 16.67 ms.

**Not measured, and why** — the crossover doc prints these as *not measured*, never as zero:

- **wasm32 arm** — needs p4's real ABI (`harness/wasm-run.mjs` driving `graph-wasm`'s exports, not the
  Phase-0 shim) and `harness/sdk-smoke.mjs`.
- **TypeScript oracle arm** — `harness/oracle-tick-bench.mjs` is not in this slice. Next step, first
  thing, and it does not need p4: it loads `src/core/layout/forceLayout.ts:163` `tick()` under
  `node --experimental-strip-types --experimental-loader ./tests/ts-extension-loader.mjs` the way
  `harness/oracle-layouts.mjs` loads d3, and times `repeat` ticks per N. (Note: the container mount must
  include `node_modules/` for `d3-force@3.0.0` to resolve.)
- **N = 1 000 000** — `bench --n` is capped by `snapshot_cmd::MAX_NODES` (100 000) and that constant
  lives in `snapshot_cmd.rs`, outside this phase's envelope. Raising it, or adding a campaign-specific
  bound, is a decision for the orchestrator.
- `fixtures/scale/*` generators, `scale/{lod,simplify,adaptive}.rs`, `capabilities --ceilings-measured`,
  `BENCHMARKS.md`, and the Amdahl / autovectorisation split of §6b (it needs the wasm arm to answer the
  `f32x4` half).

## Next step

1. `harness/oracle-tick-bench.mjs` — the third arm, N = 220 first.
2. The `f32x4` / Amdahl split against `layout.force.barnes_hut`, native side only, so Phase 11 has the
   scalar baseline even before p4 lands.
3. `scale/lod.rs` (heuristic, Ponytail-marked: it can hide low-degree important nodes — the dangerous
   direction) with the `grid_index` reuse the prompt demands.
