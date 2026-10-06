# Job dag-horizontal (agent build: a published `horizontal` switch for `layout.dag.sugiyama` and `layout.dag.lanes`)

Why: the user, 2026-10-06: "the dag of sugiyama should be horizontal not vertical". Every layered
layout draws its layers top to bottom, and none can draw another way. The ruling is
`docs/decisions/dag-horizontal.md` (PROCEED-WITH-CONDITIONS). **Read it in full first.** Its eight
conditions are this job's acceptance criteria.

The peer session graph-render-0e asked for one thing: the field is appended last and defaults to
`false`, so its hub relay roundtrip fixtures stay bit-identical.

The engine must not know where a graph comes from. Nothing you write names commits, git or history.

Facts (develop 7ad507e1; re-check each on your branch, and stop if one no longer holds):
- `SugiyamaParams` is at `crates/graph-core/src/layout/sugiyama/mod.rs:64-75`, one field
  `layer_spacing`. Its `Stage::run` (`:77-84`) calls `run(topology, params.layer_spacing)` (`:89`).
- `LanesParams` is at `crates/graph-core/src/layout/lanes.rs:33-48`. `lanes::run` (`:61-72`) ends
  in `Geometry::planar(NodeGeometry::Point { x, y }, EdgeGeometry::Polyline(paths), ...)`.
- Both are published through `tunable!` in `crates/graph-core/src/registry/tunable.rs:197-207`.
  The macro takes `field: RustTy, Kind, min, max, default, step, "doc";` per field, and `bool` is
  supported (`FromParam for bool`, `:77-81`). No published parameter is a `Bool` yet.
- `Geometry` (`crates/graph-core/src/layout/mod.rs:53-65`) has `nodes`, `edges`, `notes` and `z`,
  and is never built as a struct literal (`:50-51`). `NodeGeometry` (`Point{x,y}`,
  `Circle{x,y,r}`, `Box{x,y,w,h}`), `EdgeGeometry` (`Line`, `Polyline(Paths)`,
  `Curve{degree, paths}`) and `Paths{offsets, pts}` are in `crates/graph-contract/src/geometry.rs:105-164`.
- The struct literals the new field breaks are listed in the ruling's condition 3. The compiler
  names any others.
- `deploy/nav/paramsrows.py:205` asserts `shown["labels"] == [SPACING]` on `layout.dag.sugiyama`.
  The gate is `scripts/studio-params.sh`, and `STUDIO_PARAMS_BREAK=1` is its negative control.

Steps (TDD: each test is written and seen RED before the code that turns it green):
1. **The transpose, once.** Create `crates/graph-core/src/layout/transpose.rs` and declare it as
   `mod transpose;` in `layout/mod.rs`. Write exactly this, plus a module doc that cites the ruling:
   ```rust
   use super::Geometry;
   use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

   impl Geometry {
       /// This geometry with x and y exchanged: node centres, a box's width and height, and
       /// every edge point. Offsets, radii, notes and z are kept as they are.
       pub(crate) fn transposed(mut self) -> Self {
           self.nodes = transposed_nodes(self.nodes);
           self.edges = transposed_edges(self.edges);
           self
       }
   }

   fn transposed_nodes(nodes: NodeGeometry) -> NodeGeometry {
       match nodes {
           NodeGeometry::Point { x, y } => NodeGeometry::Point { x: y, y: x },
           NodeGeometry::Circle { x, y, r } => NodeGeometry::Circle { x: y, y: x, r },
           NodeGeometry::Box { x, y, w, h } => NodeGeometry::Box { x: y, y: x, w: h, h: w },
       }
   }

   fn transposed_edges(edges: EdgeGeometry) -> EdgeGeometry {
       match edges {
           EdgeGeometry::Line => EdgeGeometry::Line,
           EdgeGeometry::Polyline(paths) => EdgeGeometry::Polyline(swapped(paths)),
           EdgeGeometry::Curve { degree, paths } => EdgeGeometry::Curve { degree, paths: swapped(paths) },
       }
   }

   fn swapped(mut paths: Paths) -> Paths {
       for point in paths.pts.chunks_exact_mut(2) {
           point.swap(0, 1);
       }
       paths
   }
   ```
   Tests, in a child module `layout/transpose/tests.rs`:
   - `a_transpose_swaps_centres_sizes_and_every_path_point`: a `Box` geometry with distinct
     x, y, w, h, and a `Polyline` with offsets `[0, 1, 3]` and three distinct points. Assert every
     column exactly (`to_bits`), and that `offsets` is unchanged.
   - `two_transposes_give_back_the_same_geometry`: `assert_eq!` after `.transposed().transposed()`.
2. **The parameter.** Append `pub horizontal: bool` as the **last** field of `SugiyamaParams` and
   of `LanesParams`. Its doc: "Lays the layers (rows) out along x, left to right, instead of along
   y. `false` is the vertical drawing, bit for bit." The default is `false`.
   - Append to each `tunable!` entry:
     `horizontal: bool, Bool, 0.0, 1.0, 0.0, 1.0, "layers along x, left to right, instead of along y";`
   - Reword the doc of `layer_spacing` and `row_spacing`, both in the struct and in `tunable!`,
     to say "along y, or along x when `horizontal`".
   - Sugiyama: in `Stage::run`, `let drawing = run(topology, params.layer_spacing)?;` then return
     `drawing.transposed()` when `params.horizontal`, else `drawing`. The public
     `sugiyama::run(topology, layer_spacing)` is unchanged.
   - Lanes: at the end of `lanes::run`, the same choice on the built geometry.
   - Fix every struct literal the compiler names. Add `horizontal: false`, or
     `..Default::default()` where the file already uses it.
3. **Tests the ruling names** (condition 5, its ruling 6):
   - `crates/graph-core/src/layout/sugiyama/tests.rs`:
     - `horizontal_draws_the_layers_along_x_bit_for_bit`: one topology with at least one edge
       that spans two layers, so its path has interior points. Run with `horizontal: false` and
       with `true`. Assert `x_h == y_v` and `y_h == x_v` by `to_bits`, `offsets` equal, and every
       `pts` pair swapped.
     - `horizontal_false_is_the_default_drawing`: `Stage::run` at `SugiyamaParams::default()`
       equals `Stage::run` at `horizontal: false`, by `assert_eq!`.
   - `crates/graph-core/src/layout/lanes/tests.rs`: `horizontal_draws_the_rows_along_x_bit_for_bit`
     and `horizontal_false_is_the_default_drawing`, the same way, on a graph with a merge so a
     path bends.
   - `crates/graph-core/src/registry/params/tests/schema.rs`:
     `horizontal_is_published_last_as_a_bool_defaulting_to_false`, for both structs: the last spec
     is named `horizontal`, kind `ParamKind::Bool`, default `0.0`.
   - Keep the invariant suites (Sugiyama's routing checker, lanes' `nothing_sits_on_an_edge`) on
     the vertical drawing only (condition 6).
4. **The wasm32 half** (condition 2). Create `harness/horizontal-wasm.mjs`, at most 150 lines, in
   the style of `harness/sdk-smoke.mjs`: the SDK's public surface only (`createMotor` from
   `crates/graph-sdk-js/src/index.ts`), and exit 0 pass, 1 fail, 2 could not run.
   - Usage: `node --experimental-strip-types harness/horizontal-wasm.mjs <graph_wasm.wasm> [--break]`.
   - For each fixture `fixtures/dag/multi-span.json` and `fixtures/dag/wide-layer.json`, and each
     layout `layout.dag.sugiyama` and `layout.dag.lanes`:
     - build the graph the way `harness/sdk-smoke/build.mjs` builds one;
     - `motor.run(handle, id, { params: { horizontal: false } })` and then `{ horizontal: true }`;
     - read the node x/y columns and the edge path columns;
     - compare as `Uint32Array` views of the `Float32Array`s: the true run's x equals the false
       run's y, its y equals the false run's x, and every path point is swapped; the offsets are
       equal.
   - Print one line per case: `PASS <fixture> <layout>` or `FAIL <fixture> <layout> <first
     mismatch>`.
   - `--break` sends `horizontal: false` for the second run too. The comparison must then fail
     (exit 1), so the control runs the real comparison.
5. **The studio row** (conditions 4 and 8).
   - In `deploy/nav/paramsrows.py`: `row_layered` expects `shown["labels"] == [SPACING, "horizontal"]`.
   - Add `row_horizontal(studio, broken)`, named "params-horizontal", to `run_rows`, on
     `layout.dag.sugiyama`. It finds the switch labelled `horizontal` in the panel, clicks it with a
     real click, and waits for `settled`. It then checks four things:
     - `state()["params"]` holds `horizontal` true;
     - the digest changed;
     - the drawn width and height swapped, within 1e-3 relative;
     - the pixels changed.
   - Under `broken`, it clicks nothing and must FAIL.
   - Keep `paramsrows.py` and `paramspage.py` at most 300 lines each, with every function at most
     40 lines. If they would not fit, put the new row in a new `deploy/nav/paramsrows2.py` the
     same way `displayrows2.py` exists.
   - Add "a published bool is a switch and turns the layered drawing on its side" to the
     `Rows:` line in the header of `scripts/studio-params.sh`.
6. **Green.** Run `scripts/orch/gate.sh target/rows-dag-horizontal scripts/orch/rows/dag-horizontal.rows`
   until every row is PASS.

Rules (beyond `scripts/orch/common.md`):
- Paths you may touch:
  - `crates/graph-core/src/` (only the files above, plus the struct literals the compiler names);
  - `crates/graph-cli/src/` (struct literals only);
  - `harness/horizontal-wasm.mjs`;
  - `deploy/nav/paramsrows.py`, `deploy/nav/paramspage.py`, a new `deploy/nav/paramsrows2.py`;
  - the header of `scripts/studio-params.sh`.
- Do not touch `crates/graph-contract`, `crates/graph-wasm`, `crates/graph-sdk-js`, `server/`,
  `packages/` or `app/`. The ruling needs no regeneration: `codegen --check` and
  `capabilities --check` must stay green as they are.
- graph-core takes no new dependency and no `unsafe`. Each function is at most 40 lines, each file
  at most 300, nesting at most 3.
- No hash may move: the hash gate and the service digests run at the defaults.
- Run `scripts/orch/node-slim.sh npm ci --ignore-scripts` once before `cargo test`.
- If an `edit` fails twice on the same file, read the whole file, then `write` it once. Never repeat
  an identical edit.

Done when `scripts/orch/gate.sh target/rows-dag-horizontal scripts/orch/rows/dag-horizontal.rows`
writes a `summary.txt` with every row PASS.

Return:
- `status: done` or `status: blocked` and the reason;
- the branch tip;
- the files changed, with line counts;
- the RED run of each new test;
- each row's result;
- every deviation.
