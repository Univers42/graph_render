# Layout parameters on the wire

Status: accepted and implemented (job `ux-params-abi`, 2026-10-03), after the `devil`
ruling recorded at the end: PROCEED-WITH-CONDITIONS, all twelve conditions met.

## The problem

`Motor#layouts()` answers *what can this module draw* and nothing else: `gm_layout_id`
publishes a capability id and `gm_run`'s `run: fn(&Topology)` takes no arguments, so
every drawing in the studio runs at whatever the layout's own `Default` says. The
parameters exist — `FrParams`, `KkParams`, `DrlParams`, `LglParams`, `GraphoptParams`,
`DhParams`, `ForceParams`, `Fa2Params`, `SpringParams`, `GridParams`, `SugiyamaParams`,
`CirclePackingParams` — and every one of them is read by the layout that owns it, but
none of them is reachable from outside the crate. So the studio cannot ask for a wider
spacing, a longer run or a slower cooling schedule, and a caller that guesses a value has
nowhere to discover the range, the default or what the knob is called.

This is the ABI half of the work. The dock that draws the controls (`ux-params-dock`) is
the next job.

## The schema

One parameter is one `ParamSpec`, in `graph-contract::params`:

| field | type | meaning |
| --- | --- | --- |
| `name` | `&'static str` | the key a caller sends; the struct field's own name |
| `kind` | `ParamKind` | `Int`, `Float` or `Bool`, as `0`/`1`/`2` on the wire and `"int"`/`"float"`/`"bool"` in JSON |
| `min`, `max` | `f64` | inclusive; a value outside is **refused**, never clamped |
| `default` | `f64` | equal, bit for bit, to the field's `Default` |
| `step` | `f64` | the increment a control should offer; never applied to a value |
| `doc` | `&'static str` | one line, for the label and the tooltip |

An `Int` value must be integral and a `Bool` must be `0.0` or `1.0`; both are carried as
the same `f64` as everything else, so the buffer is one type rather than three. Every
number is a `f64` because the parameters are `f64` at heart — a `u32` iteration count is
exactly representable, so nothing is lost by carrying it in the wider type.

The list is a `&'static [ParamSpec]` in **schema order**, and that order *is* the wire
order. There is no name lookup on the wire: a caller resolves a name to an index once,
through the schema, and sends a positional buffer. Two layouts can therefore never
disagree about what index 3 means, because index 3 means "the fourth parameter this
layout publishes".

### What is advertised, and what is not

Thirteen of the thirty-nine registered layouts publish parameters; the other twenty-six
publish an empty list (`LayoutParams::NONE`, which carries the `Ponytail:` line saying
exactly that). The rule is **advertise only what the layout really reads, and never
invent one**:

| layout | struct | published |
| --- | --- | --- |
| `layout.grid` | `GridParams` | `spacing` |
| `layout.dag.sugiyama` | `SugiyamaParams` | `layer_spacing` |
| `layout.packing.circle` | `CirclePackingParams` | `iterations`, `scale` |
| `layout.force.spring` | `SpringParams` | `iterations`, `threshold`, `scale` |
| `layout.force.spring3d` | `SpringParams` | the same three |
| `layout.forceatlas2` | `Fa2Params` | `max_iter`, `jitter_tolerance`, `scaling_ratio`, `gravity`, `seed` |
| `layout.forceatlas2.barnes_hut` | `Fa2Params` | the same five |
| `layout.force.fruchterman_reingold` | `FrParams` | `niter`, `seed` |
| `layout.force.kamada_kawai` | `KkParams` | `epsilon` |
| — | | |
| `layout.force.graphopt` | `GraphoptParams` | `niter`, `node_charge`, `node_mass`, `spring_length`, `spring_constant`, `max_sa_movement`, `seed` |
| `layout.force.davidson_harel` | `DhParams` | `maxiter`, `fineiter`, `cool_fact`, `seed` |
| `layout.force.lgl` | `LglParams` | `maxit`, `coolexp`, `seed` |
| `layout.force.drl` | `DrlParams` | `edge_cut`, `seed` |

Four things are deliberately **not** in the schema, and each is a limitation rather than
an oversight:

- **`Option` fields** (`FrParams::start_temp`, `KkParams::maxiter`, `KkParams::kkconst`,
  `LglParams::{maxdelta, area, repulserad, cellsize, root}`). Their `Default` is `None`,
  which means "derived from `n`" — there is no number to publish as `default`, and a
  schema row claiming one would be a claim about a value the layout never sees.
  `layout.force.kamada_kawai` is the visible cost: `epsilon` is all it publishes, and its
  `maxiter` and `kkconst` stay unreachable.
  Ponytail: a caller that wants `kkconst = n` must run the layout natively; a published
  `None` would freeze the derived value into an explicit one and change the drawing.
- **Nested structs** (`DhParams::weights`, `DrlParams::phases`). They are one row each in
  the schema, or a tree the buffer cannot express. One row would need the flattened name
  `weights.node_dist` and a struct the buffer does not have.
  Ponytail: `DhParams::weights` and `DrlParams::phases` are unreachable from a run.
- **`ForceParams`** (`layout.force.barnes_hut`, `layout.force.yifan_hu`,
  `layout.force.particle_mesh`). It has twelve fields and each of those three layouts
  reads a different subset; publishing all twelve for all three would advertise knobs that
  do nothing on two of them, which is the "never invent one" rule broken from the other
  side. Publishing the intersection needs a per-layout subset and three spec tables.
  Ponytail: those three layouts publish nothing; their spacing is fixed at
  `ForceParams::default()`. The live force session (`docs/decisions/live-force-session.md`)
  already takes all twelve over its own ABI, and that is the surface to tune them through.
- **`f64`-ness.** `CirclePackingParams::scale`, `GridParams::spacing` and
  `SugiyamaParams::layer_spacing` are `f32` in Rust and `f64` on the wire. A value the two
  cannot both hold is rounded to the nearest `f32` on the way in.
  Ponytail: `spacing = 0.3` runs as `0.30000001192092896`; the schema's number is what a
  caller sent, not what the layout multiplied by. The escape hatch is one line — widen the
  three fields to `f64` — and it is not taken here because it would move hashed bytes.

## The wire face

`graph-contract::params` holds the types and the format; it is the crate every other one
depends on, so neither graph-core nor the wasm module has to restate the encoding.

`codegen` emits two more files from those types, alongside the four it already emits:

- `crates/graph-contract/generated/layout-params.schema.json` — the JSON Schema
  (draft 2020-12) of `LayoutParamsSchema`, derived from the serde/`schemars` mirror types
  added to `canonical_json/schema.rs`;
- `crates/graph-contract/generated/layout-params.d.ts` — the TypeScript declarations for
  the same types, zero runtime bytes, generated by the same `declaration`/`ts_type` path
  the header's `.d.ts` uses.

`codegen --check` compares both against what the types generate now, so the committed
files cannot rot, exactly as for the snapshot header.

The generated schema describes the **shape**, not the values: the specs themselves live in
graph-core's registry, where the layouts are, and reach a caller through the ABI below.
That split is the same one the header schema already makes — it describes
`SnapshotHeader`, not any particular snapshot.

## The ABI

Two exports, and only one of them is new:

### `gm_layout_params(layout_index: u32) -> u32` (new)

Publishes one layout's schema as a framed buffer (`[len: u32 LE][len bytes]`, the framing
every other buffer in this ABI uses), or `0` with `Code::IndexOutOfRange`. The body is
little-endian throughout:

```
u32  param_count
param_count times:
  u32  name_len   ; name_len bytes, UTF-8
  u8   kind       ; 0 int, 1 float, 2 bool
  f64  min, max, default, step
  u32  doc_len    ; doc_len bytes, UTF-8
```

`layout_index` is the same index `gm_run` takes, so a caller resolves the id to an index
once (`gm_layout_count`/`gm_layout_id`) and uses it for both. A layout that publishes
nothing answers with `param_count = 0` and a four-byte body — not a refusal, because
"this layout takes no parameters" is an answer.

### `gm_run(handle, layout_id, params_ptr, params_len) -> u32` (changed)

The signature is unchanged and `params_ptr`/`params_len` are no longer refused: they now
carry the run's parameters. `params_len == 0` keeps meaning "the layout's own defaults",
which is what every existing caller sends, so this is additive at the ABI level and every
hash, every smoke and every recorded gate keeps its bytes.

A non-empty buffer is **one little-endian `f64` per published parameter, in schema
order**: `specs.len() * 8` bytes, nothing else. There is no header, no name table and no
version byte inside it, because the schema it is read against *is* the version: a motor
that publishes a different list has a different `gm_layout_params` answer, and a buffer
built for the old list is the wrong length and is refused rather than misread.

## The SDK

```ts
motor.layoutParams("layout.force.graphopt"): LayoutParamSpec[]
motor.run(handle, layoutId, { params?: Record<string, number | boolean> }): RunResult
```

The spec types are named `LayoutParamSpec` and `LayoutParamKind` rather than `ParamSpec`
and `ParamKind`, which the studio already exports for its own knobs with different
meanings (`packages/graph-studio/src/actions/registry.ts`) — one name for two shapes is
the confusion this repo's own rules keep warning about, so the generated `.d.ts` carries
the same names.

`layoutParams` is `gm_layout_params` over the index the id already resolves to, decoded
into the generated `LayoutParamSpec` shape and cached per motor. `run` is `gm_run` with an optional named
`params`: the SDK looks each name up in the schema it just asked the motor for, refuses a
name the schema does not publish (a `RangeError`, not a silent drop), refuses a value
that is not a number or a boolean, and writes the values positionally in schema order.
`Motor#layout(handle, layoutId)` stays as the two-argument form and is now `run` with no
options, so no existing caller changes. `layoutParams` reads the schema at most once per
motor: a live module's schema cannot change, and re-reading it per drawing would be a
round trip per drawing.

**The SDK does not range-check a value.** It checks the two things it can without a round
trip — a key the schema does not publish, and a value that is not a finite number — and
sends everything else as written. A value out of range comes back as `ParamOutOfRange`
from the motor, because a second range check here would be a second rule to keep in step
with the schema, and two rules that agree today are two that can stop agreeing.

The buffer is staged through `gm_alloc` and freed by the SDK, like every other buffer
(C7): the caller never sees a pointer.

## Refusal

A value out of range is refused with a code. It is never clamped, never rounded into
range and never quietly replaced by the default — a drawing that is not the one asked for
is worse than no drawing.

| condition | `Code` | value |
| --- | --- | --- |
| the layout publishes nothing and the buffer is not empty | `ParamsNotAccepted` | 22 |
| the buffer is not `specs.len() * 8` bytes, or `(ptr, len)` is not a live `gm_alloc` | `ParamsMalformed` | 21 |
| a value is not finite, not integral (an `Int`), not `0`/`1` (a `Bool`), or outside `[min, max]` | `ParamOutOfRange` | 20 |
| otherwise the run itself fails | `LayoutFailed` | 8 |

The three buffer codes are checked in that order, and **the layout is resolved before the
buffer**: a caller that named a layout that does not exist is told `UnknownLayoutId` (5),
not that its parameters were wrong. `ParamsNotAccepted` is checked before the length,
because for a layout that publishes nothing every non-empty length *is* wrong, and the
specific code is the one a caller can act on.

`ParamsMustBeEmpty` (6) is **not produced any more**. Its number stays reserved so an older
SDK reading a newer motor's code fails loud on an unknown value rather than reinterpreting
it; the SDK's `CODE_NAMES` keeps the name at index 6 and appends the three new ones, so
every code number that meant something before still means it. `20`, `21`, `22` are the next
free values (`19` is `IngestTooLarge`, which reached develop first) because `errors::mirrors::every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order`
holds `Code` to be contiguous from `0` — a code inserted in the middle would renumber every
one above it.

**A name the schema does not publish is refused by the SDK, not the motor.** Names never
cross the wire — the buffer is positional — so this half of the rule can only live in
`layout-params.ts`, as a `RangeError` rather than a silent drop. The residual is stated
rather than papered over: a raw host that skips the SDK and assembles a buffer positionally
from a stale schema gets a length refusal for a changed *count* and **no motor-side refusal
at all** for a changed name, order or range. `gm_abi_version` is what catches that case, and
it was raised to `2` for it (`docs/contract/wasm-abi.md` "Exports").

Natively, the same three refusals arrive as `StageError::Param { name, rule }` — the
parameter's own name, and the static rule "outside the range the schema publishes". The
precise bounds are in the schema, which is the one place they are stated once.

## Determinism

- **Little-endian `f64`, always.** The run buffer and the four numbers in a published
  spec are the same width and the same order on both targets, so a native run and a wasm
  run of the same buffer are the same computation. No `usize`, no host word order, no
  padding.
- **Schema order is the wire order**, and the specs are a `&'static` array, so a
  capability's parameter list is fixed at compile time. Nothing iterates a hash map to
  produce one.
- **Defaults reproduce today's bytes exactly**, and not by argument. Every published
  `default` equals its field's `Default` bit for bit
  (`every_advertised_default_is_its_struct_default`), and **an empty buffer is the
  layout's own `Default`** — `Capability::params_values` returns the published defaults for
  it rather than reading a zero-length buffer as a malformed one, so a run with no buffer
  is the run the hash gate has always called. `a_run_at_the_published_defaults_is_the_registered_run`
  compares the two byte for byte across all thirty-nine rows, which is the evidence;
  `hashgate --seeds 8` not moving is the same claim at the gate.
- **A value that cannot be both `f64` on the wire and `f32` in the struct** is rounded to
  the nearest `f32` by `as`, which is IEEE and identical on both targets. Every *bound* of
  an `f32`-backed layout is a power of two or a whole number precisely so the rounding can
  never push a value past a bound the schema published
  (`every_f32_backed_bound_survives_the_f32_round_trip`).

## Where the list lives, and one deviation

The parameter list is a field of `graph_core::registry::Capability`
(`params: &'static LayoutParams`), not of `Metadata`, for two reasons and one of them is
the real one:

1. **`Metadata` derives `Eq`** (`registry/capability.rs`), and a list of specs carrying
   `f64` bounds cannot be `Eq`. A `params` field there would drop `Eq` from every layout
   row and from `crate::post::styles::ledger::meta`, which builds a `Metadata` for the four
   POST styles — a file outside this job's paths, and one whose factory would need editing
   for four rows that are not layouts.
2. `Capability` is constructed only where the layouts are, so the published list and the
   run that honours it sit in one literal and a layout cannot declare parameters the
   dispatcher has no path for.

Everything the body asked `Metadata` to carry is carried, one level up: the ledger row is
built from the `Capability`, and `motor.layoutParams(id)` reads the same field.

`LAYOUTS` moved out of `registry.rs` into `registry/layouts.rs` to pay for the fourth
field: thirty-nine six-line literals do not fit beside `find`, the ceiling re-exports and
the tests under the house's 300-line cap. Nothing about the array's order changed, which is
the one thing that must not (see the append-only comment at the top of that file).

The capabilities ledger JSON (`graph-cli capabilities`) does **not** gain a `params`
column in this phase. The schema is the published face, the ledger row would have grown a
field on all thirty-nine rows to repeat it, and `capabilities --check` is a gate row that
has no reason to move. `ux-params-dock` reads `motor.layoutParams`, not the ledger.

## Tests

| claim | where |
| --- | --- |
| every advertised default equals its struct's `Default`, all eleven structs | `graph-core`, `registry/params/tests.rs` |
| each published index reaches its own field, per struct | same file |
| a run at the published defaults is the registered run, byte for byte, all 39 | same file |
| one parameter moved changes the output, one row per advertising layout (13) | same file |
| a value out of range is refused by name, never clamped | same file, both directions |
| every `f32`-backed bound survives the `f32` round trip; every `Int` bound is a whole `f64` | same file |
| the schema encodes to the length it claims; an empty list is a bare count | same file |
| the export publishes exactly what `graph-core` publishes, all 39 | `graph-wasm`, `exports/build/tests.rs` |
| the refusal order, and that a refusal leaves no geometry behind | same file |
| the format itself, decoded by a reader written against the doc | `graph-contract`, `params/tests.rs` |
| `hashgate --seeds 8` is unchanged | the existing gate row |

The wasm half of "round-trips natively and through the export" is bounded by the host, and
the bound is worth stating: on the native 64-bit test target every heap address is past
`u32`, so `gm_alloc` refuses to return one and `is_live` can never admit a buffer. The two
paths that need a real pointer therefore cannot run natively. `gm_layout_params`'s framed
body is still checked natively — through `wire::last_frame`, because the body is what a
caller decodes and the address it would come back at is not part of it — and the refusal
walk covers everything up to liveness. The pointer itself is covered by `sdk:smoke` and by
the wasm32 release build.

`GM_MUTATE_LAYOUT_PARAM_DEFAULT` is the negative control, and it is a control over the ABI
rather than over an algorithm: the hash gate's native arm runs
`layout.force.fruchterman_reingold` through `Capability::run_params` at a buffer whose value
at one index is one more than the default the registry publishes — a drawing that is not the
hashed one, which turns the row red. Measured: `hashgate --seeds 2` exits `0` unset, `1` at
`=0` or `=1`, and `2` (refused, not clamped) at `=7`, `=one`, `=-1`.

It is in `crates/graph-cli/tests/common/mod.rs`'s knob list like every other, so an honest
integration run cannot inherit it, and in `hashgate::Knob::ALL` with its own record so the
gate's evidence names it.

## Verdict

`devil`, on this document and the ABI it describes, before implementation:
**PROCEED-WITH-CONDITIONS**. Blast radius 2 of 5 (one handle's drawing goes wrong; the
graph and every hash stay intact), reversibility 5 of 5 (re-run with `params_len == 0`),
cost on failure 2 of 5 (a plausible wrong picture — a *silent* wrong answer, not a red
test). Its summary: the ABI really is additive, the refusal-not-clamp rule is the right
shape, and nothing in it is irreversible.

Its two findings that changed the implementation, rather than its checklist:

- **Q5, the failure it called most likely: a same-length field/index desynchronisation in
  one of the thirteen spec tables.** A hand-written `ParamSpec` list beside a hand-written
  setter, coupled only by being positional — swap two rows and every value stays in range,
  every gate stays green, every hash is unchanged for `params_len == 0`, and the picture is
  simply wrong. The doc's own "one parameter moved changes the output" test cannot see it,
  because *some* parameter moving does change the output.
  **Answered by construction, not by a test.** One `tunable!` invocation per struct
  generates the spec table *and* the applier from one `field, kind, min, max, default,
  step, doc` list (`registry/tunable.rs`), so index `i` of the wire is field `i` of the
  struct and there is no second list to drift. The per-index test
  (`each_published_index_reaches_its_own_field`) is there to say so out loud.
- **Condition 8, the code numbers were wrong.** `errors::mirrors` holds `Code` contiguous
  from `0`, so the next free value is `19`, not `20`. Fixed, and the reason is now stated
  in the Errors table above.

The twelve conditions, and where each landed:

| # | condition | where |
|---|---|---|
| 1 | bump `ABI_VERSION` to `2` | `crates/graph-wasm/src/lib.rs`, `wasm.ts`, `wasm-abi.md`; `errors::mirrors` holds all three |
| 2 | `gm_layout_params` in `RawExports`/`EXPORT_NAMES`, and its load consequence recorded | `wasm.ts`; recorded above and in the export's own note |
| 3 | correct the version argument — a length check catches a changed *count* only | "The wire face" and "Determinism" above |
| 4 | per-index alignment test | answered by construction; tested anyway |
| 5 | `is_live` before any read, `len == 0` never read | `exports/build.rs::read_params`, its SAFETY note, and a native test that a dead pointer is refused rather than trapped on |
| 6 | read the buffer bytewise, never `&[f64]` | `ParamsView::value` copies eight bytes and `from_le_bytes`es them; the `ALIGN = 4` reason is in its doc |
| 7 | SDK writes `f64`s through `DataView` | `layout-params.ts::encodeLayoutParams`, with the `RangeError` reason quoted |
| 8 | codes `20`, `21`, `22` | `errors.rs`, `CODE_NAMES`, the Errors table |
| 9 | all five mirrors updated | `mirrors.rs`, the Errors table, the `gm_run` row, `CODE_NAMES`, the coverage table, the now-false `params_ptr` SAFETY comment, the `Code::ParamsMustBeEmpty` doc |
| 10 | the refusal precedence stated and tested | the table above; `a_dead_handle_is_refused_before_anything_else_is_read` and `an_index_past_the_registry_is_refused_by_both_layout_exports` |
| 11 | `f32` bounds exact, `Int` bounds whole | `every_f32_backed_bound_survives_the_f32_round_trip`, `every_published_integer_is_exactly_representable`; every `f32` bound is a power of two or a whole number |
| 12 | the doc's own status and claims corrected, and the SDK's type names | this document; `LayoutParamSpec`/`LayoutParamKind`, because the studio already exports `ParamSpec`/`ParamKind` for its own knobs with different meanings |

Condition 12 also caught two name collisions worth writing down: `graph-core::post::Metadata`
is a differently-shaped type with the same name as the registry's (pre-existing), and the
studio's `packages/graph-studio/src/actions/registry.ts` already exports `ParamKind` and
`ParamSpec` for its own knobs — which is why the generated `.d.ts` and the SDK export
`LayoutParamSpec` and `LayoutParamKind`.

Had the verdict been BLOCK, this section would say so instead.