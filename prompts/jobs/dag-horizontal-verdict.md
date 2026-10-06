# Job dag-horizontal-verdict (agent devil, docs only: the public-surface ruling on a horizontal option for the layered layouts)

Why: the user, 2026-10-06, about a commit history loaded into the studio: "the dag of sugiyama
should be horizontal not vertical". Every layered layout in the motor draws top to bottom, and none
can draw another way:
- `layout.dag.sugiyama`: `SugiyamaParams` has one field, `layer_spacing`, the "Y distance between
  adjacent layers" (`crates/graph-core/src/layout/sugiyama/mod.rs:64-67`). It is published through
  `tunable!` (`crates/graph-core/src/registry/tunable.rs:197-200`).
- `layout.dag.lanes`: `LanesParams` has `lane_spacing` (x) and `row_spacing` (y)
  (`crates/graph-core/src/layout/lanes.rs:33-38`, `tunable.rs:202-207`).
- `layout.dag.dot` publishes no parameters by decision (`crates/graph-core/src/layout/graphviz/dot.rs:169-174`).

The engine must not know where a graph comes from. The option is about geometry only: "lay the
layers out along x instead of y". It is not about commits.

**Proposal.** Append one published `Bool` parameter, `horizontal` (default `false`), as the **last**
field of `SugiyamaParams` and of `LanesParams`.
- `false` is today's drawing, bit for bit.
- `true` is the same drawing transposed: every node's `(x, y)` becomes `(y, x)`, and every edge
  interior point too. Layer (or row) `k` sits at `x = k · spacing`, read left to right; the order
  inside a layer runs top to bottom.
- Both layouts already emit `NodeGeometry::Point { x, y }` and `EdgeGeometry::Polyline(paths)`
  (`sugiyama/mod.rs:99-108`, `lanes.rs:61-72`). The transpose is a swap of the two node columns
  and of each `(pts[2p], pts[2p + 1])` pair (`crates/graph-contract/src/geometry.rs:155-164`). There
  is no arithmetic, so it is exact and the same on native and wasm32.
- It lives once, as a `graph-core` function both layouts call. The doc of `layer_spacing` and
  `row_spacing` changes from "y distance" to "distance between adjacent layers (rows), along y, or
  along x when `horizontal`".
- The studio needs no code: `LayoutParamsPanel` draws a `bool` as a switch
  (`packages/graph-studio/src/ui/paramSpecs.ts:7-16`, `ui/LayoutParamsPanel.tsx:8`), and the
  schema is read at run time through `gm_layout_params` (`crates/graph-wasm/src/exports/build/params.rs:15-42`).
- `git grep` finds no caller outside `graph-core` that names `layer_spacing`, `lane_spacing` or
  `row_spacing` (server, harness, packages, app, SDK). Re-check this.

**Alternatives** the ruling must weigh, one line each on why not (or why instead):
1. An `Int` `rank_direction` 0..3 (top-bottom, left-right, bottom-top, right-left), Graphviz's
   `rankdir`. More choices, and two of them are reflections that no caller has asked for.
2. A layout-agnostic post stage (`post.transpose`): a new registry entry, no change to any existing
   schema, but a step away from the layout the user is choosing.
3. A view rotation in `packages/graph-render`. The motor's geometry stays vertical, so fit, picking,
   labels and exports would each have to know about it. `packages/graph-render` belongs to another
   session.

Read, in full:
- `docs/decisions/layout-params.md`;
- `crates/graph-contract/src/params.rs`, `crates/graph-core/src/registry/params.rs`,
  `registry/tunable.rs`, `registry/params/tests/schema.rs`;
- `crates/graph-wasm/src/exports/build/params.rs`, and `crates/graph-sdk-js/src/params.ts`;
- `crates/graph-core/src/layout/sugiyama/mod.rs`, `crates/graph-core/src/layout/lanes.rs` and
  `lanes/geometry.rs`;
- `packages/graph-studio/src/state/paramValues.ts`, `state/settings.ts`, `ui/LayoutParamsPanel.tsx`;
- `.claude/rules/devil/risk.md`.

Rule on each of these. For each, one line: OK, or a condition the build job must meet.
1. **The shape.** `Bool horizontal` appended last, against alternatives 1-3.
2. **Backward compatibility of the buffer.** `read_params` refuses a buffer whose length is not the
   schema's (`params.rs:46-60`). A caller that cached the old one-value schema for Sugiyama now
   gets `ParamsMalformed`. Is "the schema is read at run time" enough? Or must a shorter buffer be
   accepted as a prefix, with the missing fields at their defaults? Name every caller you find that
   builds a buffer without reading the schema.
3. **The hash gate.** At the defaults nothing moves, so the hash gate is unchanged. Does
   `horizontal = true` need a hash-gate arm, or a mutate knob with a negative control? Or is a
   `graph-core` test enough? That test would run the same topology with `horizontal = true`, and
   require bit-for-bit equality with the transposed default output, on native and on the wasm32
   build.
4. **Codegen, capabilities and the service.** Does a published parameter appear in
   `graph-cli codegen` output, `graph-cli capabilities`, `docs/measurements/service-caps.tsv` or the
   service digests (`server/graph-server/tests/digest/manifest.json`)? Say which files the build
   job must regenerate, and with which command. `server/` belongs to another session.
5. **The studio.** A user's saved settings may hold the old one-value parameter list for Sugiyama
   (`state/paramValues.ts`, `state/settings.ts`). Does the studio still load it after the schema
   grows: by name, or by position?
6. **Tests the build job must add**, by name:
   - transposition equality on both layouts;
   - `false` is today's bytes;
   - the schema lists `horizontal` last, as `Bool`, default 0;
   - each layout's own invariant tests (Sugiyama's routing checker, lanes' `nothing_sits_on_an_edge`)
     still hold in the horizontal frame, or run on the vertical drawing only, and why.

Write `docs/decisions/dag-horizontal.md`: a status line (PROCEED, PROCEED-WITH-CONDITIONS or BLOCK),
the four risk scores from `risk.md`, the rulings 1-6, and numbered conditions. Cite `file:line` for
every claim.

Rules (beyond `scripts/orch/common.md`):
- Docs only. The single file you write is `docs/decisions/dag-horizontal.md`. No code, no build.
- Re-read every citation above on your branch before relying on it; a wrong one is a finding.

Done when `scripts/orch/gate.sh target/rows-dag-horizontal-verdict scripts/orch/rows/docs.rows`
writes a `summary.txt` with every row PASS.

Return:
- the branch tip;
- the status line;
- the conditions;
- every deviation.
