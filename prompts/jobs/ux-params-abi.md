# Job ux-params-abi (agent build: each layout publishes its parameters, and a run can set them)

Why (user, 2026-10-03): the user wants to tune every drawing, not only the live force: spacing,
iterations, distances, per layout. Today the studio cannot, because the motor publishes layout ids
only. `Motor#layouts()` returns ids (`crates/graph-sdk-js/README.md:144`), and a run takes no
parameters.

Facts (confirm before editing):
- `crates/graph-core/src/registry.rs`: `LAYOUTS` and `Metadata`. No field is an `Option`.
- Parameter structs already exist, for example:
  - `FrParams` (`layout/force/fruchterman_reingold.rs:19`)
  - `KkParams`, `DrlParams`, `LglParams`, `GraphoptParams`, `DhParams`
  - `ForceParams` (`layout/force/params.rs:27`)
  - `CirclePackingParams` (`layout/circle_packing.rs:98`)
  - the ring's `run_with` (`layout/circular/ring.rs:42`)

  Read how each layout gets its parameters today before choosing a shape.

Do, in order:
1. Write `docs/decisions/layout-params.md`. It covers:
   - the schema: per parameter, a name, a kind (`int|float|bool`), min, max, default, step, and a
     one-line doc;
   - its wire face, in the contract crate, emitted by codegen;
   - the ABI: one export for a layout's schema, and one run variant that takes a parameter buffer;
   - the SDK: `motor.layoutParams(id)` and `motor.run(id, { params })`;
   - refusal: a value out of range is refused with a code, never clamped;
   - determinism: little-endian `f64`, and defaults reproduce today's bytes exactly.
2. The ABI is a public surface, so route this decision through the `devil` agent. Record its verdict
   in the doc. On BLOCK, stop and return.
3. Implement it additively. `Metadata` gains the parameter list; a layout without parameters has an
   empty list, with a `Ponytail:` line saying so. Advertise only parameters that the layout really
   reads. Never invent one.
4. Tests:
   - every advertised default equals the struct's `Default`, in one test over `LAYOUTS`;
   - the schema round-trips, native and wasm;
   - a value out of range is refused;
   - changing one parameter changes the output, with one test per layout family;
   - hashgate 8 hashes do not change, because the defaults still run.
5. A `GM_MUTATE_*` knob makes one advertised default differ from its struct, and must turn a row red.
   Add it to the knob list in `crates/graph-cli/tests/common/mod.rs`.

Paths:
- `crates/graph-core/src/{registry.rs,registry/**}` and the parameter structs
- `crates/graph-wasm/src/**`
- `crates/graph-contract/**`
- `crates/graph-sdk-js/src/**`
- `crates/graph-cli/**` (codegen, capabilities, knob)
- `docs/decisions/layout-params.md` and `docs/contract/`

Out of bounds: `packages/` and `app/` (`ux-params-dock` comes next). Edit `registry.rs`,
`capabilities.rs` and `canonical_json/schema.rs` additively only. No new dependency.

Done when:
- `scripts/orch/rows/ux-abi.rows` is green: the merge floor, hashgate 8 and its negative control,
  `capabilities --check`, `codegen --check`, `sdk:typecheck`, the release wasm build, and `sdk:smoke`.
- The decision doc carries the verdict.
