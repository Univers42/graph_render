# A horizontal option for the layered layouts

Status: **PROCEED-WITH-CONDITIONS** — the shape is right and the transpose is exact, but the
build job must meet the conditions below before the parameter ships. `devil`, 2026-10-06,
on the proposal in `prompts/jobs/dag-horizontal-verdict.md`.

## Risk (`.claude/rules/devil/risk.md`)

| axis | score | why |
| --- | --- | --- |
| Blast radius | 2 | At `horizontal = false` nothing moves; a wrong transpose is one layout's drawing wrong when the flag is on. The graph and every default hash stay intact. |
| Reversibility | 5 | Re-run with `horizontal = false` (or an empty buffer) is today's drawing, bit for bit. No migration, no publish, no delete. |
| Cost on failure | 2 | A plausible wrong picture — a *silent* wrong answer, not a red test — if the transpose is wrong and no test catches it. The transposition-equality test is the guard. |
| Confidence | 2 | Every citation in the proposal was re-read and holds. What is unverified is the build job's execution: the wasm32 half of the transpose test, the struct-literal fixes, and the first-Bool switch path. |

Worst: **blast radius** — a wrong transpose is a silent wrong drawing, and the parameter is a
public surface.

## The proposal

Append one published `Bool` parameter, `horizontal` (default `false`), as the **last** field of
`SugiyamaParams` (`crates/graph-core/src/layout/sugiyama/mod.rs:64-67`) and of `LanesParams`
(`crates/graph-core/src/layout/lanes.rs:33-38`). `false` is today's drawing; `true` is the same
drawing transposed — every node's `(x, y)` becomes `(y, x)`, and every edge interior point too.
The transpose is a swap of the two node columns and of each `(pts[2p], pts[2p + 1])` pair
(`crates/graph-contract/src/geometry.rs:155-164`). No arithmetic, so it is exact and the same on
native and wasm32.

## Rulings

### 1. The shape — `Bool horizontal` appended last, against alternatives 1–3

**OK.** A `Bool` is the smallest change that gives the user what they asked for ("lay the layers
out along x instead of y"), it is exact (a transpose, no arithmetic), and the `tunable!` macro
already supports a `bool` field (`FromParam for bool`, `crates/graph-core/src/registry/tunable.rs:77-81`),
so the spec table and the applier are generated from one list and cannot drift
(`tunable.rs:88-124`). Against the alternatives:

1. An `Int rank_direction` 0..3 (Graphviz's `rankdir`) — more choices, and two of them are
   reflections no caller has asked for; it also imports a Graphviz-ism the motor does not
   otherwise follow (`crates/graph-core/src/layout/graphviz/dot.rs:169-174` records the decision
   that the motor does not grow a knob the reference's own ledger does not set).
2. A layout-agnostic post stage (`post.transpose`) — a new registry entry with its own schema,
   but a step away from the layout the user is choosing: the user picks `layout.dag.sugiyama`,
   not "sugiyama, then transpose". It would also need its own params and its own gate row.
3. A view rotation in `packages/graph-render` — the motor's geometry stays vertical, so fit,
   picking, labels and exports would each have to know about it, and `packages/graph-render`
   belongs to another session.

### 2. Backward compatibility of the buffer

**OK — "the schema is read at run time" is enough; a shorter buffer must *not* be prefix-accepted.**
`read_params` refuses a buffer whose length is not the schema's (`crates/graph-wasm/src/exports/build/params.rs:58-61`),
and `validate` is the one refusal rule (`crates/graph-contract/src/params.rs:154-171`). The
design is explicit: "a buffer built for the old list is the wrong length and is refused rather
than misread" (`docs/decisions/layout-params.md:152-156`). An empty buffer already means
"the layout's own defaults" (`crates/graph-core/src/registry/params.rs:171-173`), so a caller
that wants defaults sends nothing — there is no need for a prefix path, and adding one would
create a silent route for a stale caller to get a drawing it did not ask for.

**Every caller that builds a buffer reads the schema first** — this was checked, not assumed:

| caller | where it gets the schema |
| --- | --- |
| SDK `encodeLayoutParams` | `crates/graph-sdk-js/src/layout-params.ts:82-104`, specs from `gm_layout_params` |
| SDK schema cache (per motor) | `crates/graph-sdk-js/src/params.ts:46-56` |
| Studio motor | `packages/graph-studio/src/motor/session.ts:240` → `motor.layoutParams` |
| Hash gate `layout_bytes` | `crates/graph-cli/src/hashgate/stages.rs:265-271`, reads `layout.params()` |
| Hash gate native arm | `crates/graph-cli/src/hashgate/stages.rs:145`, passes `setting.sugiyama` directly (no buffer) |

A live module's schema is `&'static` and fixed at compile time
(`crates/graph-core/src/registry/params.rs:235-237`), so a per-motor cache cannot go stale
within a session. The one residual — a raw host that skips the SDK and assembles a buffer
positionally from a stale schema — is already named and is caught *loudly* by the length check
(`docs/decisions/layout-params.md:219-223`). `ABI_VERSION` is explicitly never bumped for a
registry entry (`crates/graph-wasm/src/lib.rs:132-135`), so no version bump is needed.

### 3. The hash gate

**OK — no new arm is needed; a `graph-core` test is enough, and it must run on native *and*
wasm32.** At the defaults nothing moves: the native arm runs sugiyama at `setting.sugiyama`,
which is `SugiyamaParams::default()` at `Setting::compiled_in()`
(`crates/graph-cli/src/hashgate/stages.rs:145`, `knob/setting/honest.rs:25`), and the wasm arm
sends `params_len == 0` (`harness/wasm-run/abi.mjs:98`). Both are `horizontal = false`, so every
hash is unchanged. The gate pins the *registered* run (defaults); `horizontal = true` is not the
registered run, so the gate has nothing new to pin.

A `graph-core` test is the right home for the transpose: run the same topology with
`horizontal = true` and require bit-for-bit equality with the transposed default output. The
wasm32 half is the load-bearing one (D10: bit-identical native vs wasm32), and it needs a driver
that sends a params buffer — the existing wasm arm sends none (`harness/wasm-run/abi.mjs:98`).
The build job should drive the wasm module the way `scripts/orch/svc-digest-wasm.sh` does
(`cargo build -p graph-wasm --release --target wasm32-unknown-unknown`, then a node driver that
calls `gm_run` with a two-value buffer), or add a wasm32-targeted test. If the build job
instead adds a hash-gate knob for `horizontal`, it must be native-arm-only (like
`Knob::SugiyamaLayerSpacing`, `crates/graph-cli/src/hashgate/knob/kind.rs:57-58`) with a negative
control in the pattern of `the_layer_spacing_knob_moves_only_the_layered_drawing_and_zero_is_refused`
(`crates/graph-cli/src/hashgate/tests/pipeline.rs:114-134`) — the house rule that every gate row
has a negative control that must fail. The existing `Knob::LayoutParamDefault` perturbs
Fruchterman–Reingold's defaults (`crates/graph-cli/src/hashgate/knob/setting/params.rs:13`), not
the layered layouts', so it does not cover the new parameter.

### 4. Codegen, capabilities and the service

**OK — a published parameter appears in none of them, so nothing is regenerated.**

- `graph-cli codegen` generates from the *contract's* types (`crates/graph-cli/src/codegen.rs:39`,
  `graph_contract::codegen::outputs()`); the generated schema describes the **shape**, not the
  values (`docs/decisions/layout-params.md:116-119`). Adding a field to a graph-core struct does
  not change the contract's generated schema.
- `graph-cli capabilities` — the ledger has no `params` column
  (`crates/graph-cli/src/capabilities.rs:58-95`, `docs/decisions/layout-params.md:275-278`).
- `docs/measurements/service-caps.tsv` — scale ceilings only (`cap_n`, `cap_m`;
  `docs/measurements/service-caps.tsv:11,52`).
- Service digests (`server/graph-server/tests/digest/manifest.json:12,52`) — default-run hashes,
  unchanged at `horizontal = false`. `server/` belongs to another session.

The files the build job **must** touch are not regenerations but fixes:

- Struct literals that a new field breaks (compile errors): `crates/graph-cli/src/hashgate/tests/pipeline.rs:117,124`,
  `crates/graph-core/src/layout/sugiyama/tests.rs:104,110`,
  `crates/graph-core/src/layout/lanes/tests.rs:183,195,205`.
- `deploy/nav/paramsrows.py:205` asserts `shown["labels"] == [SPACING]` — the panel now shows
  `["layer_spacing", "horizontal"]`, so this deploy-time row goes red. `deploy/` belongs to
  another session; the build job must update it (or hand it to that session) or the deploy gate
  fails.

### 5. The studio

**OK — the studio loads by name, not by position, and needs no code.** Saved settings are a
name→value map (`packages/graph-studio/src/state/paramValues.ts:40-45`); `valuesOf` maps over
the motor's specs and falls back to `spec.default` for an absent key
(`packages/graph-studio/src/ui/paramSpecs.ts:20-23`); `encodeLayoutParams` builds the buffer
positionally from the specs, with values keyed by name
(`crates/graph-sdk-js/src/layout-params.ts:91-93`). So a saved
`{"layout.dag.sugiyama": {"layer_spacing": 2.0}}` still loads: `layer_spacing` is found by name,
and `horizontal` is absent, so it takes the published default `false`. A `bool` renders as a
switch through existing code (`paramSpecs.ts:15-17` maps `bool`→`flag`,
`packages/graph-studio/src/ui/controlOf.tsx:5` maps `flag`→`toggle`,
`packages/graph-studio/src/ui/controls/ToggleControl.tsx:15`).

**Condition:** `horizontal` is the *first* published `Bool` — every parameter today is an `Int`
or a `Float` (`crates/graph-core/src/registry/tunable.rs:131-236`) — so the studio's bool→switch
path has never been exercised by a real published parameter. The build job should verify the
switch renders and commits (a studio test, or the deploy `params-panel` row). The `ToggleControl`
handles a numeric `0` default (`checked={value === true}`), so the default renders correctly.

### 6. Tests the build job must add

- **Transposition equality on both layouts** — run the same topology with `horizontal = true`
  and require bit-for-bit equality with the transposed default output, on native and on the
  wasm32 build (condition 2).
- **`false` is today's bytes** — the existing
  `a_run_at_the_published_defaults_is_the_registered_run`
  (`crates/graph-core/src/registry/params/tests/drawing.rs:13-35`) covers the default path
  across all layouts; add an explicit test that `horizontal = false` output equals the default
  output, bit for bit, so the default path is pinned by name and not only by a table.
- **The schema lists `horizontal` last, as `Bool`, default 0** — a new test in
  `crates/graph-core/src/registry/params/tests/schema.rs`. The existing
  `every_advertised_default_is_its_struct_default` (`schema.rs:44-59`) and
  `each_published_index_reaches_its_own_field` (`schema.rs:66-98`) automatically cover the new
  field, but an explicit "last, `Bool`, default `0`" assertion is the named test.
- **Each layout's own invariant tests** — run them on the **vertical drawing only**. Sugiyama's
  `assert_invariants` checks `y[tail] < y[head]` and that every route hop steps exactly one
  `LAYER_SPACING` in Y (`crates/graph-core/src/layout/sugiyama/routing/tests.rs:179-229`); lanes'
  `assert_nothing_sits_on_an_edge` checks columns at lanes
  (`crates/graph-core/src/layout/lanes/tests/history.rs:55-75`). Both are frame-specific. The
  transposition-equality test covers the horizontal frame, because every invariant is preserved
  under an exact axis swap: a vertex on an edge in the horizontal frame is a vertex on an edge
  in the vertical frame. Running the invariant suites on the horizontal frame without
  re-expressing them would fail — the frame changed — and re-expressing them would test the
  transpose a second time.

## Conditions

1. The transpose is a pure column swap: the two `NodeGeometry::Point` columns and each
   `(pts[2p], pts[2p + 1])` pair of `EdgeGeometry::Polyline`'s `pts`; the CSR `offsets` are
   preserved. No arithmetic, so it is exact and identical on native and wasm32.
2. The transposition-equality test runs on native **and** on the wasm32 build (a driver in the
   `scripts/orch/svc-digest-wasm.sh` pattern, or a wasm32-targeted test). The native half alone
   does not hold D10.
3. The struct literals in `crates/graph-cli/src/hashgate/tests/pipeline.rs:117,124`,
   `crates/graph-core/src/layout/sugiyama/tests.rs:104,110`, and
   `crates/graph-core/src/layout/lanes/tests.rs:183,195,205` are updated to the new field count.
4. `deploy/nav/paramsrows.py:205` is updated to expect `["layer_spacing", "horizontal"]` (or
   handed to the deploy session). No codegen, capabilities, `service-caps.tsv`, or digest
   regeneration is needed.
5. The schema test asserts `horizontal` is the last spec of both layouts, of kind `Bool`, with
   default `0`.
6. The invariant tests (sugiyama's routing checker, lanes' `nothing_sits_on_an_edge`) run on
   the vertical drawing only; the transposition-equality test is what covers the horizontal
   frame.
7. The build job adds no caller that hardcodes a parameter buffer; every caller reads the schema
   at runtime (condition: the residual in `docs/decisions/layout-params.md:219-223` stays a
   loud length refusal, never a silent prefix-acceptance).
8. The studio's bool→switch path is verified with the first published `Bool` (a studio test or
   the deploy `params-panel` row).

## The failure nobody mentioned

**The transpose is exact only if it swaps the right things.** The node geometry is two `Vec<f32>`
columns and the edge geometry is a CSR `paths` (`crates/graph-contract/src/geometry.rs:155-164`):
swapping the two node columns and each `pts` pair is exact, but swapping `offsets` too — or
swapping the wrong pair, or normalising after the swap — is a silent wrong drawing that every
default hash would still pass (the defaults never exercise the flag). The transposition-equality
test is the only thing that catches it, which is why condition 2 (run it on wasm32, not just
native) is the one not to skip. The second one: `horizontal` is the first published `Bool`, so
the studio's switch path is unexercised — a green build is not evidence the control works.

Had the verdict been BLOCK, this section would say so instead.
