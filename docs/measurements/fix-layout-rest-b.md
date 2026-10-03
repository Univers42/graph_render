# fix-layout-rest-b — repair log for `docs/reviews/review-layout-rest.md` ids LR-02 … LR-12, LR-20 … LR-23, LR-31 … LR-33

Scope: `crates/graph-core/src/layout/{basic_3d.rs,basic_3d/**,grid.rs,grid/**,random.rs,random/**}`,
`crates/graph-core/src/registry/three_d.rs` (LR-04 line only), `registry/hierarchy.rs` (LR-12 line
only). Paths below are relative to `crates/graph-core/src/` unless they start with `docs/`.

The three prerequisite jobs named in the job body (`sg-spiral3d`, `sg-mt19937`,
`sg-grid-minors`) are on this tree and had rewritten half the cited lines, so every cited
`file:line` was re-opened before a verdict. Four ids their changes had already closed are
recorded `fixed-by`, with the measurement file that closed them as evidence.

## Findings

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| LR-02 | MAJOR | fixed-by sg-grid-scale | `a_scale_that_does_not_fit_the_f32_range_is_refused` (pre-existing) | `layout/grid/scaled.rs:57-88` → `layout/grid/scaled/tests.rs:88` |
| LR-03 | MAJOR | fixed | `a_scale_other_than_five_is_reachable`, `a_scale_that_is_not_finite_and_positive_is_refused` | `layout/basic_3d.rs:50-72` (the false claim), `:78-88` (`checked`), `:139` / `:189` (the five `*_scaled`) |
| LR-04 | MAJOR | fixed (visibility + doc-only) | `the_eight_corners_are_the_reference_literal_in_its_order` (pre-existing, unchanged) | `layout/basic_3d/cube.rs:71` (`pub`), `layout/basic_3d.rs:48` (`pub use cube::CORNERS`) |
| LR-05 | MAJOR | fixed | `the_seed_moves_only_the_interior` | `layout/basic_3d/cube/tests.rs:189`, `layout/basic_3d/cube.rs:137` (`columns_scaled`) |
| LR-06 | MINOR | doc-only | — | `layout/basic_3d/sphere.rs:19-24`, `:38-41` |
| LR-07 | MINOR | doc-only | — | `layout/basic_3d/helix.rs:97-103` |
| LR-08 | MINOR | fixed-by sg-mt19937 | — | `layout/basic_3d/cube.rs:168-183` |
| LR-09 | MINOR | doc-only | — | `layout/basic_3d/cube.rs:3`, `:94-102`, `layout/basic_3d/cube/tests.rs:119-131` |
| LR-10 | MINOR | false | `an_empty_graph_is_still_a_3d_geometry_for_all_four` (pre-existing, unchanged) | `layout/basic_3d/tests.rs:118-131` |
| LR-11 | MINOR | fixed | negative control run, see below (a `debug_assert`, unreachable from any input) | `layout/basic_3d.rs:104-118` |
| LR-12 | MINOR | doc-only (registry half) | — | `registry/hierarchy.rs:119-121` |
| LR-20 | MINOR | fixed | `a_spacing_whose_extreme_cell_is_not_finite_is_refused` | `layout/grid.rs:135-166`, `layout/grid/tests.rs:104-140` |
| LR-21 | MINOR | doc-only | — | `layout/grid/scaled.rs:15-21` |
| LR-22 | MINOR | fixed | `every_node_matches_the_reference_formula` (helper made independent; negative control below) | `layout/grid/scaled/tests.rs:139-161` |
| LR-23 | MINOR | fixed | — (house file limit) | `layout/grid.rs:211`, `layout/grid/scaled.rs:130` |
| LR-31 | MINOR | doc-only | — | `layout/random.rs:15-16` |
| LR-32 | MINOR | doc-only | — | `layout/random/tests.rs:94-101`, `:121-126` |
| LR-33 | MINOR | fixed-by sg-mt19937 | — | `layout/random.rs:59-73` (`run_seeded`) |

Nothing is `deferred`.

## LR-03 — the scale is a parameter now, not a constant

The finding's evidence holds on this tree: the claim at `layout/basic_3d.rs` that "no caller in
SciGraphs ever passes anything else" was false, `layout_scale` is a `FloatProperty(min=0.1,
max=100.0)` (`SciGraphs/SciGraphs/properties/scene_properties.py:478-483`) handed to
`apply_graph_layout` (`ui/operators/scigraphs/layout_operators.py:315`) and from there to
`_sphere_layout` & co. (`layouts/dispatcher.py:101-110`).

**The job body preferred the entry point over the corrected claim, and it fits the house limits,
so the entry point is what landed.** One additive `*_scaled` wrapper per id — `sphere_scaled`,
`helix_scaled`, `cube_scaled`, `spiral_scaled`, `bipartite_3d_scaled` — each over one
`basic_3d::checked` (`finite and above 0`, the shape `grid/scaled.rs:69-74` already uses), and
one `run_scaled`/`columns_scaled` per submodule. The registered default does not move: each plain
wrapper still calls its submodule's `run(n)`, which still multiplies by the `SCALE` constant, so
no product sequence changed and no hashed snapshot can move.

Line counts after the change: `basic_3d.rs` 194, the five submodules 109–239, `cube.rs` 202.
Widest new signature: 3 parameters (`cube::columns_scaled(n, scale, seed)`).

Ponytail: the five `*_scaled` wrappers are five near-identical three-line functions. A macro or a
`dyn` table would fold them into one and cost the named entry points a caller can point at — which
is the whole of what they are for. Failing input: none; they are the reference's own signature.
Direction: none. Escape hatch: `SCALE` is still the registered default, so deleting all five and
the `run_scaled` half of each submodule restores the previous surface exactly.

## LR-05 — the seed is a parameter, so the test can vary it

`SEED` is now a parameter of `cube::columns_scaled(n, scale, seed)`, with `columns(n)` =
`columns_scaled(n, SCALE, SEED)`. `the_seed_moves_only_the_interior` now asks for two seeds:
one seed twice is byte-identical, `SEED + 1` moves every interior coordinate and none of the
eight corners.

**Negative control** (a seed-ignoring kernel — `interior(..., SEED)` instead of the argument):

```
$ scripts/orch/gr cargo test -p graph-core --lib layout::basic_3d::cube
test layout::basic_3d::cube::tests::interior::a_neighbouring_seed_moves_the_interior_and_no_corner ... ok
test layout::basic_3d::cube::tests::the_interior_is_the_same_twice_over ... ok
test layout::basic_3d::cube::tests::the_seed_moves_only_the_interior ... FAILED
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 1277 filtered out

thread 'the_seed_moves_only_the_interior' panicked at cube/tests.rs:210:
assertion `left != right` failed: the interior did not move with the seed, so this asserts nothing
  left: 3.292362674000624
 right: 3.292362674000624
```

The kernel was restored immediately afterwards; `cargo test -p graph-core --lib
layout::basic_3d::cube` is 16 passed / 0 failed on the tree as it stands.

Note the second row of that control: `a_neighbouring_seed_moves_the_interior_and_no_corner`
(sg-mt19937) also stayed green under the mutant, because it compares the layout against a fresh
`Mt19937::new(SEED + 1)` rather than against the kernel handed a second seed. It is a control on
the *arithmetic*, not on the seed reaching the kernel; LR-05's test is now the one that covers
that, and this is why both are kept.

## LR-11 — one place owns the 3D shape

`basic_3d::in_space` now asserts, in every debug build and for all five layouts plus
`layout::random::run_seeded`, that the three columns are the same length. `Geometry::in_space`
deliberately does not length-check `z` (`layout/mod.rs`: the rule lives at `snapshot`), so the
check belongs in the builder. The two per-wrapper `debug_assert_eq!(geometry.dim(), Dim::D3)`
were removed — `in_space` always sets `Some(z)`, so they were asserting what the line below them
already guaranteed.

**Negative control** (`in_space` dropping its last z value):

```
thread 'layout::basic_3d::cube::tests::node_i_takes_corner_i_at_the_scale' panicked at basic_3d.rs:82:
assertion `left == right` failed: the z column must be one value per node, or snapshot refuses it
under node.z
```

A `debug_assert` and not a refusal, on purpose: no input reaches it today, and a release check
would put a branch in five hot paths against an edit that has not happened. Ponytail: it is off in
release, so a release-only caller can still hash a short z column and be refused at `snapshot`
under `node.z`. Direction: a refusal naming the column, never a wrong drawing. Escape hatch: the
rule is one `debug_assert_eq!` in one function.

## LR-22 — the expectation is no longer a function of the code

`reference()` in `layout/grid/scaled/tests.rs` now counts `cols = ceil(sqrt(n))` up one step at a
time (`columns_by_hand`, in `u64` because `65536 * 65536` does not fit a `u32`) instead of asking
the production `dimensions`.

**Negative control** (`dimensions` regressed to `cols = isqrt(n)`, i.e. floor):

```
with the hand-counted cols:  every_node_matches_the_reference_formula ... FAILED   (n = 2)
with the old helper:         every_node_matches_the_reference_formula ... ok
```

The second line is the defect: the old expectation reproduced the regression on both sides.

## LR-20 — a finite spacing whose cell centre is not finite is refused

`cells` gained the second rule `grid/scaled.rs` already had — the widest cell centre
`((max(cols, rows) - 1) / 2) * spacing` must be inside the `f32` range — with the error
`StageError::Param { name: "spacing", rule: "widest cell centre inside the f32 range" }`. RED
before the change, on the current code:

```
test layout::grid::tests::a_spacing_whose_extreme_cell_is_not_finite_is_refused ... FAILED
thread '…' panicked at crates/graph-core/src/layout/grid/tests.rs:119:36:
refused: Lattice { count: 16, cols: 4, rows: 4, spacing: 3.4028235e38 }
```

`f32::MAX` at `n = 16` gives `cols = rows = 4`, so the offset is `1.5 * f32::MAX` and `f32`
overflows to `inf` (measured, not assumed — the cast does not saturate on this toolchain). `n = 4`
is asserted to stay legal, so the rule is a bound and not a veto. No hash moves: the default
spacing is `1.0`, and the gate's own control is `GM_MUTATE_GRID_SPACING=2`.

## LR-04 — the escape hatch now exists at the path the ledger names

`registry/three_d.rs` no longer holds the `CUBE` row: the file was split into
`registry/three_d/{basic,graph,spiral3d,bipartite_3d}.rs` by an earlier job, and the escape-hatch
sentence cited by LR-04 is at `registry/three_d/basic.rs:108-109` — outside this job's paths. The
half that *was* in scope is fixed here, and it is the half that makes the sentence true rather
than cosmetic: `CORNERS` is now `pub` and re-exported as `layout::basic_3d::CORNERS`, so the
exact path the ledger writes, `crate::graph_core::layout::basic_3d::CORNERS`, resolves.
`pub use` is additive and moves nothing.

## Commands

```
scripts/orch/gr cargo test -p graph-core --lib layout::grid              -> 0  (13 passed)
scripts/orch/gr cargo test -p graph-core --lib layout::basic_3d          -> 0  (68 passed)
scripts/orch/gr cargo test -p graph-core --lib layout                    -> 0  (893 passed, 5 ignored)
scripts/orch/gr cargo fmt --all --check                                  -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings    -> 0
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                    -> see below
```