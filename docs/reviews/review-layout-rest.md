# Review: `layout/{basic_3d,bipartite,circular,coords,planarity,random,spiral}/**`, `grid`, `adjacency`, `mod`

Scope: `crates/graph-core/src/layout/{basic_3d,bipartite,circular,coords,planarity,random,spiral}/**`
and their `.rs` parents, plus `layout/grid.rs`, `layout/grid/scaled.rs`, `layout/adjacency.rs`,
`layout/mod.rs`, and their rows in `crates/graph-core/src/registry*`. Reviewed as `origin/develop`
stood at the start of this job (`9af0ccb`). **Review only: no code was changed.**

Method: twelve `explore` subagents, one per module, fanned out in a single message; every row below
was then re-derived by me against the source, and every BLOCKER/MAJOR either carries a command I ran
in this job or a `file:line` in the pinned reference it disagrees with. **No BLOCKER survived
verification** — the four the subagents proposed are recorded, with why each is refuted, under
"Refuted" so the next reviewer does not re-derive them. `Graphviz` (`layout/graphviz/**`) is out of
scope. Where the conformance matrix already names a divergence, the row cites that matrix row
instead of re-reporting it.

References: networkx 3.6 at `$GM_SCRATCH/refs/networkx-3.6/networkx-3.6/`, SciGraphs at the
`SciGraphs/` submodule. Both were read for the rows that cite them.

---

## Findings

| id | severity | file:line | defect | failing input | evidence | proposed fix |
|---|---|---|---|---|---|---|
| LR-01 | MAJOR | `layout/spiral.rs:109-121` (contract at `:43`) | `SpiralParams::resolution` is documented "Finite and above 0" and is **never validated**: the equidistant branch divides by `STEP * theta` with `theta = resolution`, so `resolution = 0.0` gives `theta += 1/(0.5*0) = inf`, every `radius * libm::cos(inf)` is NaN, and `run_with` returns `Ok(Geometry)` holding all-NaN coordinates | `run_with(&graph(4, &[]), &SpiralParams { resolution: 0.0, equidistant: true }, &Serial, 1)`; also `resolution: f64::NAN`, and the archimedean branch at `resolution: NAN` | measured (scratch test, deleted): `equid res=0 n=2 -> [(NaN, NaN), (NaN, NaN)]`, `arch res=NaN n=2 -> [(NaN, NaN), (NaN, NaN)]`; `spiral.rs:113` `theta += CHORD / (STEP * theta)`; `spiral.rs:43` "Finite and above 0"; the reference raises instead — `refs/networkx-3.6/.../drawing/layout.py:1331-1335` divides a python float by zero; the sibling `grid` **does** enforce the identical rule, `grid.rs:136-142` `StageError::Param { name: "spacing", rule: "finite and above 0" }` | reject at `run_under` with `StageError::Param { name: "resolution", rule: "finite and above 0" }`, the shape `grid.rs:136-142` already uses. Nothing corrupt ships today — `snapshot` refuses it as `SnapshotError::NonFinite { column: "node.x", index: 0 }` (measured) — so this is an unenforced stated contract and a refusal that names the column instead of the parameter, not a wrong drawing |
| LR-02 | MAJOR | `layout/grid/scaled.rs:59-60`, `:106-108` | the guard's own doc says "`run_scaled` refuses what `run` refuses instead of writing a non-finite coordinate", but the guard only tests `scale.is_finite() && scale > 0.0`; nothing bounds `cell * scale` against the `f32` narrowing, so a **legal** scale writes `inf` | `Grid::run_scaled(&graph(4, &[]), 1e300, &Serial, 1)`, and `f64::MAX`; `scale = 1e300 * 4` also overflows `f64` inside the multiply | measured (scratch test, deleted): `scale=1e300 -> x=[0.0, inf, 0.0, inf] y=[0.0, 0.0, inf, inf]`; `scaled.rs:62` `if !(scale.is_finite() && scale > 0.0)`; `scaled.rs:107` `(cell as f64 * self.scale / self.cols as f64) as f32`; the doc claim at `scaled.rs:59-60` | extend the guard to reject a finite scale whose largest cell coordinate is not finite after the narrowing (`scale * (rows - 1) / cols`), or correct the doc to say the finite check bounds the *parameter* and the coordinate check happens at `snapshot` |
| LR-03 | MAJOR | `layout/basic_3d.rs:41-42` (repeated `registry/three_d.rs:86`) | `SCALE` is a fixed `5.0` justified by the doc claim "no caller in SciGraphs ever passes anything else". **The claim is false**: `layout_scale` is a user-facing slider (`min=0.1, max=100.0`) whose value is handed straight to the layout, so any value but 5.0 reaches `_sphere_layout`/`_helix_layout`/`_cube_layout` — and `SCALE` being a const makes that scale inexpressible here | user drags the layout-scale slider to 12.0 and lays out with `layout.basic3d.sphere` | `basic_3d.rs:41-42` "no caller in SciGraphs ever passes anything else"; `SciGraphs/SciGraphs/properties/scene_properties.py:478-483` `layout_scale: FloatProperty(..., min=0.1, max=100.0)`; `SciGraphs/SciGraphs/ui/operators/scigraphs/layout_operators.py:315` `scale=self.scale` into `apply_graph_layout` → `dispatcher.py:101-110` `pos = _call_with_props(_sphere_layout, num_nodes, scale, props=props)` | either give the three layouts a `Params { scale }` like every other stage, or correct the doc to the narrower true claim (the dispatcher *default* is `scale=5.0`, `dispatcher.py:14`) and record the capability loss in the `registry` row's `ponytail` |
| LR-04 | MAJOR | `registry/three_d.rs:172-173` (constant at `layout/basic_3d/cube.rs:62`) | the `CUBE` row's stated escape hatch for its **only** VISIBLE-REGRESSION risk ("a reordering of that array is a VISIBLE REGRESSION, not a refactor") names `crate::graph_core::layout::basic_3d::CORNERS` as public. It is `pub(super)`, is never re-exported anywhere in the crate, and the named path omits the `cube::` segment entirely — so the escape hatch does not exist | any consumer of the registry row that follows its own instruction | `three_d.rs:172-173` "crate::graph_core::layout::basic_3d::CORNERS is public, so a caller can read the order"; `cube.rs:62` `pub(super) const CORNERS: [[f64; 3]; 8]`; crate-wide `grep -n CORNERS crates/graph-core/src/` returns only `cube.rs`, its test module, and the two `registry` prose lines — no `pub use` | either `pub` it (the corner order is already a public contract by the row's own argument) or point the escape hatch at a reader that exists — `cube.rs:109-113` output, or `basic_3d/cube/tests.rs:31,46-47`, which pins all eight slots in order |
| LR-05 | MAJOR | `layout/basic_3d/cube/tests.rs:187-200` | `the_seed_moves_only_the_interior` never varies the seed. It calls `cube(&bare(20))` **twice with identical arguments** and asserts `a == b` — byte-identical in shape to `the_interior_is_the_same_twice_over` (`:157`), which already runs `cube(&bare(257))` twice. The doc's claim "changing it changes the interior while leaving the corners exactly where they were" is asserted by no line, and the test passes for **every** possible value of `SEED` | set `cube.rs`'s `SEED` to any constant, including one that makes the interior degenerate — the test is still green | `cube/tests.rs:189-191` `let a = space(&cube(&bare(20))…); let b = space(&cube(&bare(20))…); assert_eq!(a, b);`; the doc claim at `cube/tests.rs:185-186`; `registry/three_d.rs:149-150` rests its seeding decision on it | the negative control this test is named for needs a second seed: either expose `SEED` as a `cube::run_with`-style parameter and compare two seeds, or drop the doc's second clause and keep only the determinism assertion, which `:157` already makes |
| LR-06 | MINOR | `layout/basic_3d/sphere.rs:19-20`, `:34` (repeated `registry/three_d.rs:86`) | "`pi*(3 - sqrt(5))` is `2*pi/phi` to the last bit the `f64` holds". It is not: `pi*(3 - sqrt 5) = 2.399963229728653` and `2*pi/phi = 3.883222077450933`. The **code value is the reference's and is correct**; only the stated identity is false, and it is repeated in the registry row | none — a reader checking the port against the identity in the doc finds a 1.6-radian disagreement | `sphere.rs:19-20` and `sphere.rs:34`; reference `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:32` `theta = np.pi * (3.0 - np.sqrt(5.0)) * i` | drop the `2*pi/phi` identity (or state it as the *nearby* constant it actually is). The doc's real point — "not `2*pi/1.6180339887`" — is correct and worth keeping |
| LR-07 | MINOR | `layout/basic_3d/helix.rs:84-86` | the doc contradicts itself inside three lines: "`levels - 1` is zero exactly when `n` is 0 or 1" and then "`(n + 1) // 2` is 0 at `n = 0` and 1 at both `n = 1` and `n = 2`". `levels` is 1 at `n ∈ {0,1,2}`, so `levels - 1` is zero at **three** node counts, not two | `n = 2`: the code takes the `else` branch and puts both nodes at `t = 0.5`, which the doc says happens only at 0 and 1 | `helix.rs:84-86`; reference `basic.py:70-74` `levels = (num_nodes + 1) // 2` / `if levels > 1:`; the code is faithful — `helix.rs:89-95` | say "when `n < 3`". The `if levels > 1` guard is what keeps `levels - 1` from being evaluated at all, so there is no underflow — the code is right and only the prose is wrong |
| LR-08 | MINOR | `layout/basic_3d/cube.rs:137-141` | the doc says `uniform` is "`rng.uniform(-reach, reach)` (`basic.py:101`), which numpy computes as `low + (high - low) * next_double()`" and that the operand order here "is the one whose distribution `harness` measures". The reference at that line is a **post-multiply**, `rng.uniform(-1.0, 1.0, (k, 3)) * (scale * 0.8)`; only its *distribution* matches, not its arithmetic | none — `harness` gates the interior distributionally, not by coordinate | `cube.rs:137-141` vs reference `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:99-101` | correct the citation to say the port reproduces the *distribution*, and drop "the reference's form" — the doc's real point (the two spellings differ in the last bit) survives |
| LR-09 | MINOR | `layout/basic_3d/cube.rs:142`, `:81-83` | the "strictly inside" guarantee and the metadata's "interior uniform on [-0.8*scale, 0.8*scale]" are probabilistic, not structural: `uniform` is `-reach + (reach - -reach) * next_f64()`, so a draw of exactly `0.0` returns exactly `-reach`, landing a node on the shell | a stream whose first `next_f64()` is 0.0 (p ≈ 2⁻³² per draw); the reference's `rng.uniform` shares the property | `cube.rs:142-144`; `crate::synthetic.rs:49` `f64::from(t ^ (t >> 14)) / 4_294_967_296.0`; `cube/tests.rs:126` is therefore a flaky-but-live gate | say "uniform on the closed interval" (or state the 2⁻³² caveat) instead of "strictly inside". This is a claim/doc overreach, not a port error |
| LR-10 | MINOR | `layout/basic_3d.rs:69-84` | `n = 0` reaches `Ok(Geometry)` from all three entry points with empty `x/y/z`, so a `Dim::D3` snapshot with no nodes is produced; the reference does the same (`np.zeros((0,3))`), but nothing differential or unit-test exercises it | a 0-node topology | `basic_3d.rs:69,76,81` all `Ok`; the declared sweeps start at 1 — `registry/three_d.rs:91` "sizes 1 to 257", `:123` "node counts 1 to 601"; reference `basic.py:26` `positions = np.zeros((num_nodes, 3))` | add `n = 0` to the three modules' degenerate tests, so the agreement with `np.zeros((0,3))` is pinned rather than assumed |
| LR-11 | MINOR | `layout/basic_3d.rs:72` | the only 3D-dimension guard on `sphere` is a `debug_assert_eq!`, compiled out in release; `helix` and `cube` have none. `Geometry::in_space` deliberately does not length-check `z` (`layout/mod.rs:73-75`), so a short column reaches `snapshot` as a runtime refusal under `node.z` rather than as a layout-side error | a future edit that drops or shortens the z column in `helix`/`cube` | `basic_3d.rs:72` `debug_assert_eq!(geometry.dim(), …Dim::D3)`; `mod.rs:73-76` "Its length and values are not checked here" | hoist the check into `in_space` so one place owns it for all three, or drop the debug assert and rely on the documented `snapshot` rule explicitly |
| LR-12 | MINOR | `layout/circular.rs:3` and `registry/hierarchy.rs:119` | the module doc and the ledger row both declare `O(n)`, but `run` calls `Hierarchy::of(topology)`, which reads every topology edge (`keep_lowest_parent_edges`, `break_cycles`) — real cost is `O(n + m)`. The **same registry file's** sibling row `CIRCULAR_HIERARCHY` (`registry/hierarchy.rs:48`) declares `O(n + m)` for the same family | m = 9M edges: the declared `O(n)` hides a full edge pass and two sort passes | `circular.rs:3` "`O(n)`"; `circular.rs:69` `Hierarchy::of(topology)`; `hierarchy.rs:54-55`; `registry/hierarchy.rs:48` vs `:119` | correct both declarations to `O(n + m)` |
| LR-13 | MINOR | `layout/circular/ring.rs:3` vs `:73`, `:106` | the doc states the angle as `k * 2*pi / n`; the code computes `(k * step) * 2*pi` with `step = 1.0 / n` hoisted — a different `f64` association, so the doc's formula does not describe the arithmetic | none behaviourally (well inside the 1e-6 oracle tolerance, and it stays bit-identical across targets) | `ring.rs:73` `k * self.step * (2.0 * PI)` vs the doc formula at `ring.rs:3` | restate the doc as `(k * step) * 2*pi`, or note the reassociation |
| LR-14 | MINOR | `layout/coords.rs:79`, `:84` | no finiteness check anywhere in the transform, and `x.iter_mut().zip(y.iter_mut())` silently truncates on unequal column lengths — the tail of the longer column keeps its raw value and is then divided by `limit` in the second loop, so the cloud is **half**-transformed and `point_geometry` emits `x`/`y` `Vec<f32>` of unequal length | `x = [inf, 1.0], y = [0.0, 0.0]` → `mean_x = inf`, node 0 → `inf - inf = NaN`; `limit.max(NaN)` returns the non-NaN operand, so the NaN survives the guard and narrows to `f32::NAN`. `x = [1.0, 1.0, 1.0], y = [0.0]` → node 0 recentred to 0.0, nodes 1 and 2 stay at 1.0 | `coords.rs:82` `limit = limit.max(px.abs()).max(py.abs())` (`f64::max` returns the non-NaN operand); `coords.rs:84-88` the divide; `coords.rs:79` the truncating `zip`; `coords.rs:24` and `:38` validate only `x.is_empty()` | assert `x.len() == y.len()` at `rescale_under`'s entry (or take one column pair instead of two), and poison `limit` to `INFINITY` on any non-finite value so the existing `limit > 0.0` gate refuses instead of dividing. **Latent, not reachable in-tree**: all four callers (`bipartite.rs:43-44`, `circular/ring.rs`, `random.rs`, `spiral.rs`) size both columns `count`, and `snapshot` refuses non-finite at the seam (measured for LR-01) |
| LR-15 | MINOR | `layout/coords.rs:74-76` | "the fold's order is part of the bytes as much as the sum's is" is false for the fold in question: `f64::max` is associative and commutative on non-NaN operands, so its order changes nothing; only NaN handling is order-sensitive, and it is sensitive in the opposite direction from the sentence's claim | none — no behaviour defect | `coords.rs:78-83` `limit = limit.max(...).max(...)`; `coords.rs:76` the doc claim | keep the "one pass, and serial" decision (it is right for the *divide*), but drop the false claim about the fold's order |
| LR-16 | MINOR | `layout/spiral.rs:91-96` vs `:100-102` | validation order is inconsistent with itself: the `count < 2` early return runs **before** `params` is inspected, so the same bad `resolution` that yields all-NaN at `n = 2` is silently accepted at `n = 0` and `n = 1` | `resolution: 0.0` — `n = 1` gives `Ok([(0.0, 0.0)])`, `n = 2` gives `Ok([(NaN, NaN), (NaN, NaN)])` | measured (scratch test, deleted): `equid res=0 n=1 -> [(0.0, 0.0)]` and `equid res=0 n=2 -> [(NaN, NaN), (NaN, NaN)]`; `spiral.rs:91-96` the early return, `:100` the branch that reads `params` | fixed by LR-01: validate `params` before the `count < 2` short-circuit |
| LR-17 | MINOR | `layout/spiral.rs:83-89` | `run_under` takes **5** parameters, over the house limit of ≤4. Not a spiral slip: the same 5-parameter `run_under` shape is the crate-wide negative-control convention | every call site | `spiral.rs:84-88` `topology, params, runner, workers, split`; the same signature at `layout/force/barnes_hut.rs:164`, `layout/force/yifan_hu.rs:88`, `layout/circular/ring.rs:56` | record the `run_under` shape once as a named exception to the ≤4-parameter rule (or fold `split` into the `params` struct) rather than treating one call site as the defect. Listed so the deviation is visible, not so it is "fixed" in one place and left inconsistent in three |
| LR-18 | MINOR | `layout/spiral.rs:152-158` | `range.zip(out)` silently truncates, so a `Runner` handing a range longer than `out` leaves the tail at the default `(0.0, 0.0)` with no error — a misbehaving runner yields coincident nodes at the origin instead of a refusal | any `Runner` whose `step_range` is given a longer `out` | `spiral.rs:153` `for (i, slot) in range.zip(out)` | `debug_assert_eq!(range.len(), out.len())`, or index `out[(i - range.start) as usize]` so a short `out` panics loudly rather than truncating quietly |
| LR-19 | MINOR | `layout/spiral.rs:7` | the doc says the equidistant branch places "successive nodes one chord apart along the curve". At the declared defaults the **first** gap is ≈0.9843, not 1: the reference's own pre-loop `theta += chord/(step*theta)` (`layout.py:1331-1332`) offsets the first node, and the port reproduces that faithfully | `SpiralParams::default()` with `equidistant: true`: hand trace `θ=0.35 → 6.0642857`, `r₀=3.0321429`, `p₀=(3.0134, 0.3357)`, `θ=6.3940816`, `r₁=3.1970408`, `p₁=(2.9136, 1.3149)`, `‖Δ‖ = 0.9843` | `spiral.rs:7` "successive nodes are one chord apart"; `spiral.rs:112-113` reproduces `layout.py:1331-1332` exactly; reference `refs/networkx-3.6/.../drawing/layout.py:1328-1336` | say "one chord apart, except the first gap, which the reference's own start-angle offset shortens" |
| LR-20 | MINOR | `layout/grid.rs:135-149`, `:169-171` | `spacing`'s only rule is `is_finite() && > 0`, and `offset` multiplies a half-integer of up to 32767.5 by it, so a **legal** spacing overflows the `f32` coordinate | `GridParams { spacing: f32::MAX }` at n ≥ 2 (offset −0.5 already overflows) | measured (scratch test, deleted): `f32::MAX spacing -> [(-inf, -inf), (-inf, -inf), …]` | same shape as LR-02: reject a finite spacing whose largest offset is not finite after the product, or state the rule as "finite, above 0, and the extreme cell finite". `snapshot` refuses the result as `NonFinite` (measured for LR-01), so nothing corrupt ships |
| LR-21 | MINOR | `layout/grid/scaled.rs:15-17` | "Exact, not a heuristic: IEEE-754 binary64 multiply and divide **on values below `2^24`**, then the single narrowing". Only the *inputs* are bounded by `2^24`; `cell * scale` is an unbounded `f64` product that overflows to `inf` and then narrows to `inf` | `scale = 1e300`, as LR-02 | `scaled.rs:15-17` the doc claim vs `scaled.rs:107` | state the bound that actually holds ("the inputs are below `2^24`; the product is not bounded"), and let LR-02's guard carry the rest |
| LR-22 | MINOR | `layout/grid/scaled.rs:138-142`, `:191-196` | `every_node_matches_the_reference_formula` claims "the expectation is numpy's arithmetic", but its `reference()` helper calls the **production** `dimensions(n)` for `cols`, so only the per-cell multiply/divide is independent of the code | any regression in `dimensions` — e.g. `n = 50` — is reproduced by the expectation and the test stays green | `scaled.rs:191-196` `let (cols, _) = dimensions(n);` inside the test's `reference()` | compute `cols` in the test helper independently (`ceil(sqrt(n))` by hand, as `grid.rs:192-206` does for its own table), so a `dimensions` regression is caught |
| LR-23 | MINOR | `layout/grid.rs` (whole file) | the file is **296 of the 300 permitted lines** — four lines of headroom, with three `#[cfg(test)] mod tests` items still to come | any further addition to this file breaks the house limit | `wc -l crates/graph-core/src/layout/grid.rs` → 296; longest function is `Grid::run_with` at 19 lines (`:100-118`), so the function and parameter limits are clean | split the test module into `grid/tests.rs` (the tree's convention everywhere else) before the next feature lands |
| LR-24 | MINOR | `layout/planarity.rs:168`, `:180` | `.expect("x is a neighbour of v")` sits on the **public, infallible** path: `planar_embedding` → `euler_certificate` → `faces` → `Positions::next_face` → `position`. A rotation missing a reciprocal half-edge panics instead of the fail-safe `None` the module doc promises, and `next_face` then computes `end - 1` on `w`'s row with no guard (safe only because `position` already panicked for a degree-0 row) | any input where `embed::build` drops a half-edge — no test asserts `planar_embedding` never panics | `planarity.rs:168` `binary_search_by_key(&x, …).expect("x is a neighbour of v")`; `planarity.rs:181` `if back == start { end - 1 } else { back - 1 }`; the fail-safe intent at `planarity.rs:9-19,276` | return `None` (or a checked error) instead, or — cheaper and sufficient — add the exhaustive small-graph sweep property test asserting `planar_embedding` never panics, so an internal bug surfaces as a red test rather than a crash in a caller that asked a yes/no question |
| LR-25 | MINOR | `layout/planarity/embed.rs:149`, `:153` | two `.expect`s ("an ancestor with a return edge has a right/left reference") rest on an invariant that nothing in these files establishes or tests: that a node with a DG-in edge always had a tree child | any `Sides` whose `roots` omits a component, or an LR result whose root row holds only DG-in edges | `embed.rs:149-153` | assert the invariant where `Sides` is built (`lr.rs`), or return a `StageError` naming it |
| LR-26 | MINOR | `layout/planarity/embed.rs:166-183` | `into_embedding` follows `cw` for exactly `degree` steps with no cycle sanity check, so a missed splice yields a wrong rotation (duplicated or lost neighbours) rather than an error — and `euler_certificate`'s face-count check cannot see it, since `V`, `E` and `F` are identical for every planar rotation | any embedding where a splice was skipped | `embed.rs:174-177` `for _ in 0..degree { … slot = self.cw[slot] }`; the certificate's stated blindness at `planarity.rs:20-25` ("it does **not** check that the rotation is the right one") | add a `debug_assert_eq!(slot, start)` after the walk, so the invariant is checked in every debug build rather than nowhere |
| LR-27 | MINOR | `layout/planarity/triangulate.rs:18-19` vs `:29-35` | the doc claims the returned list is "that face's nodes as the outer boundary". On a tie for the longest face the fold keeps the **first in row order**, which may be an interior face | `K_{2,3}` — Euler gives three faces all of length 4, so the tie-break can leave an interior quad untriangulated and return it as the outer boundary | `triangulate.rs:29-35` `if f.len() > faces[best].len() { i } else { best }` (strict `>`, so the first max wins); `triangulate.rs:41` returns `faces[outer]` | the result is deterministic and a valid canonical ordering still exists, so this is a doc claim to soften ("the largest face, ties broken by row order"), not a bug — but a caller that draws *that* face as the boundary gets the wrong one |
| LR-28 | MINOR | `layout/planarity/triangulate.rs:41`, `:38` | `faces[outer]` is indexed with only `node_count() <= 1` as a guard (`:21`); emptiness is not otherwise excluded, and `face[0], face[1]` (`:38`) has no length check | an `Embedding` with no traced faces | `triangulate.rs:38` `builder.triangulate_face(face[0], face[1])`; `triangulate.rs:41` `faces[outer].clone()` | return early on `faces.is_empty()`, and skip any face shorter than 2 at `:36-39` |
| LR-29 | MINOR | `layout/planarity/triangulate/faces.rs:92-96` | the "fewer than 3 nodes on this face" guard is dead code for the case it names: `next_face_half_edge` is called at `:92-93`, **before** the check at `:94`, and a 2-node face (`v1 == v3`) has already panicked inside `nfh` — which is the `next_face_half_edge`'s own `.expect` at `:13` | any 2-node face reaching `triangulate_face` | `faces.rs:92-94` — the two `let` bindings precede the `if v1 == v2 \|\| v1 == v3 { return }` | move the guard above the two `next_face_half_edge` calls, as the reference's own ordering effectively does (`planar_drawing.py:317-322` computes `v3`/`v4` then tests `v1 in (v2, v3)` — so this is a faithful port of a check the reference also places after; note it in the doc rather than "fixing" a faithful order) |
| LR-30 | MINOR | `layout/planarity/adjacency.rs:100` | the `.expect` in `slot` is documented as caller-guaranteed ("every caller here only ever asks about an edge it already knows exists"), but the guarantee lives in `lr/testing.rs` and `embed.rs`, so it is unenforced at the boundary | any new caller in `lr` or `embed` that asks `slot` about a non-edge | `adjacency.rs:100`; the callers are `embed.rs:127,146,160` | return `Option` and let the callers name the invariant, or keep the `expect` and add the small-graph property test from LR-24, which would catch a violation |
| LR-31 | MINOR | `layout/random.rs:13` | the doc's citation for "the motor has no global RNG" points at `hashgate.rs:169`, which is prose inside `threads_lines`; the threaded-arm list it means is at `crates/graph-cli/src/hashgate/tiered.rs:26-27`, and it does not list `layout.random` (so the claim itself holds) | none — a wrong anchor | `random.rs:13` "hashgate.rs:169"; `crates/graph-cli/src/hashgate/tiered.rs:26-27` `const THREADED_STAGES: [&str; 5]` | fix the anchor. `docs/measurements/tiers-audit.md:47` has the same drift (cites `random.rs:25`/`26-28`; the actual `run` is `:33`, body `:35-37`) and is outside this job's paths |
| LR-32 | MINOR | `layout/random/tests.rs:14-20` | `positions_are_the_seeded_stream_row_major` re-derives with `super::SEED` and the same generator, so it pins the draw **order and count** but not the seed **value**: `SEED = 0x00_5EED` changed to any other value still passes | any seed change | `random/tests.rs:14-20`; seed coverage rests entirely on the single pinned pair at `random/tests.rs:33-36`. I recomputed the Mulberry32 stream for `0x00_5EED` independently (draw 0 → 0.7100320369936526 → f32 0.710032045841217) and it matches the pinned literal, so the const is not stale | keep the order test, and add one literal-value assertion (which `:33-36` already provides) under a name that says which one it is — the gap is that nothing says the second test exists |
| LR-33 | MINOR | `layout/random.rs:30`, `:33` | `SEED` is a private module const, not a parameter, so `run` returns the same coordinates for every caller and there is no way to obtain a second independent scatter without editing code; the draws of an `n`-node graph are a **prefix** of any larger one | `run(&graph(4, &[]))` and `run(&graph(7, &[]))` are not decorrelatable by any API | `random.rs:6` "the motor takes no seed" (documented), `random.rs:30` `const SEED: u32 = 0x00_5EED` | disclosed and consistent with `registry/closed_form.rs:24-30`, so this is a **scope note, not a defect** — recorded so the next reviewer does not re-report it |
| LR-34 | MINOR | `layout/mod.rs:3` | the module doc links `[tests]` — a `#[cfg(test)]`-gated private module — from non-test rustdoc, so a docs build without `--cfg test` leaves a link to a non-existent item | `cargo doc` on the crate | `mod.rs:3` "…[`tests`] holds the seam's tests."; `mod.rs:30-31` `#[cfg(test)] mod tests;` | drop the intra-doc link, or state the module name in plain text |
| LR-35 | MINOR | `layout/mod.rs:5-8` | the module doc still describes the seam's history — "Phase 2 has one, the grid… Phase 3 adds the tidy tree, treemap, circular and circle-packing layouts" — while **21** modules are declared, including `sugiyama`, `spectral`, `forceatlas2`, `planarity`, `pivot_mds`, `force`, `graphviz` | none — a reader looking for `layout.sugiyama` in the doc is not told it exists | `mod.rs:5-8` next to `mod.rs:11-33` | replace the phase narrative with the current module list; the phase history belongs in `docs/` |
| LR-36 | MINOR | `layout/tests.rs:11-19` | the seam tests cover no degenerate topology: the only fixture is a 3-node/2-edge graph with a duplicate node and a dangling edge, so `snapshot`'s 0-node path (empty `StringTable`, `Dim::D2` label) and its 1-node path (empty z column) are unasserted | `snapshot(&index_model(&[], &[]).unwrap(), points(0))` | `layout/tests.rs:11-19`; no empty-`nodes` fixture anywhere in the file | add a 0-node and a 1-node seam case. The z-column length and non-finite refusals are already asserted as negative controls (`:132-157`), and the notes tie-refusal at `:75-81` |
| LR-37 | MINOR | `layout/adjacency.rs:20` | the `seen_by` sentinel is `u32::MAX`, the same width as the node index it stores, so dense node index `u32::MAX` treats its first neighbour as a repeat and **drops** it | a topology whose dense node index is `4_294_967_295` | `adjacency.rs:20` `vec![u32::MAX; lists.len()]` vs `:23` `seen_by[other] != node as u32` | unreachable at any real ceiling (4.6M), so this is a sentinel-width hygiene item: use `u64` for the marker, or `Option<u32>`, so the sentinel cannot alias a value |
| LR-38 | MINOR | `layout/adjacency.rs:10-19` | the `O(n + m)` doc is true asymptotically but hides the cost: one `Vec` header per node (24 B × n) plus geometric `push` growth, with a peak of 2m `u32` before the dedup pass; no `reserve` | 1M nodes with no edges: 24 MB of empty `Vec`s for an empty answer; a dense m = 2·10⁶ edge set pays the doubling | `adjacency.rs:13` `vec![Vec::new(); node_count]`; `adjacency.rs:15-16` `push` with no `reserve`; the flat CSR alternative already exists at `crates/graph-core/src/csr.rs` | build the CSR form (`src/csr.rs`) and expose it as slices, which also matches the documented cost |

---

## Refuted (subagent findings that did not survive verification)

Recorded so the next reviewer does not re-derive them. Each was checked against the source and, where
relevant, run.

- **`triangulate/faces.rs:49-63` — "unbounded loop + parallel half-edges on a triangle with a pendant"
  (claimed BLOCKER).** Refuted twice over. (1) The port is line-for-line faithful to the reference:
  `make_bi_connected` at `faces.rs:40-65` reproduces `refs/networkx-3.6/.../planar_drawing.py:415-465`
  including the `v2 = v1` restart with a deliberately **stale** `face_set`/`on_face` and the
  un-guarded `while`. The stale-set restart *is* the reference's algorithm. (2) The input terminates:
  run on `planar_embedding(4, &[(0,1),(1,2),(0,2),(2,3)])` it returns `outer = [0,1,2,3]`,
  `edges = 5`, `euler_certificate = true`, `tri rot3 = [2, 0]` — a normal 2-connected result.
- **`triangulate.rs:38` — "`face[1]` panics on a 1-node face from a self-loop" (claimed BLOCKER).**
  Unreachable. `make_bi_connected` returns `vec![starting_node]` only when its `while` never runs, i.e.
  `next_face_half_edge(start, outgoing).1 == outgoing`, which requires a self-loop half-edge — and
  `Adjacency::simple` (`planarity/adjacency.rs:21-31`) filters `a != b` and dedups before anything
  downstream sees the graph. Confirmed by running: `planar_embedding(5, &[(0,1),(1,2),(2,3),(0,3),(0,0)])`
  yields `rot4 = []` (the self-loop is gone) and `triangulate_embedding` returns cleanly. The
  guard's *absence* is still worth a defensive check — that is LR-28.
- **`triangulate/faces.rs:13`, `:26-35` and `embed.rs:129,146` — "multi-edge handling is broken"
  (claimed MAJOR).** Both wrong on the reference. `Builder::find` and `visited` are keyed by **node
  pair**, and so is the reference: networkx's `PlanarEmbedding` is `dict[node, dict[node, attrs]]`,
  so `planar_drawing.py:432-433`'s `edges_counted` is equally pair-keyed, and
  `planar_drawing.py:317`/`planar_drawing.py:429`'s `embedding.next_face_half_edge(v, w)` is a
  dict lookup that could not distinguish parallel half-edges either. The port is faithful; and the
  input is unreachable anyway (see the previous bullet). The doc at `faces.rs:39` ("marking every
  half-edge it crosses visited") overstates what the pair-keyed `visited` can do — that doc phrase
  is the only real residue, and it is minor.
- **`bipartite/tests.rs:24-34` — "the 5-node expectation is stale and self-inconsistent"
  (claimed MAJOR).** Refuted by measurement: the code produces exactly the asserted
  `[(-0.6666667, -0.625), (1.0, -0.625), (-0.6666667, 0.0), (1.0, 0.625), (-0.6666667, 0.625)]`,
  and the same run shows the 3-node path putting its singleton second-set node at `y = 0.75` while
  the pair sits at `-0.375`. That is `np.linspace(0, 1, len)` per set followed by
  `rescale_layout` — the reference at `refs/networkx-3.6/.../drawing/layout.py:433-437`, where
  `np.linspace(0, 1, 1) == [0.0]`, so a singleton column correctly lands at the bottom of its
  cloud. The `1/(n-1)` step is *linspace*, not `i*H/len`, and the two sets are offset by
  `width/2` exactly as `left_xs/right_xs - offset` are.
- **`bipartite/partition.rs:31`, `:72`, `:78` — "the visiting order diverges from the reference"
  (implied).** Refuted: the reference iterates `for start in G.nodes()` and
  `_greedy_max_cut` iterates `for node in G.nodes()`, but SciGraphs builds its graph with
  `G.add_nodes_from(range(num_nodes))` (`SciGraphs/.../layouts/common.py:238`), so `G.nodes()` *is*
  dense-index order — exactly what `partition.rs:31,72,78` walk. `adjacency::neighbours`
  reproduces `G.neighbors`' first-edge order, as its doc claims, and the whole module is a faithful
  port of `hierarchical.py:149-211` (including the early `return None` at the first odd cycle and
  `side[node] = 0 if placed[0] <= placed[1] else 1`).
- **`layout/adjacency.rs:11` — "duplicated primitive, a fourth spelling of neighbour lists"
  (claimed MAJOR).** The duplication is real but **deliberate and correct**:
  `sugiyama/acyclic.rs:106` `unique_neighbours` sorts ascending (it needs degree counts), while
  `layout/adjacency.rs:11` keeps first-edge order (it reproduces `G.neighbors`). Two orderings, two
  consumers, one documented rationale each. Recorded as a cost/hygiene item (LR-38), not a defect.
- **`spiral.rs:5` / `tests.rs:91-103` — "the equidistant branch is never compared against the named
  oracle" (claimed MAJOR).** The branch *is* a faithful port — `spiral.rs:109-121` reproduces
  `layout.py:1328-1336` term for term, including the pre-loop
  `theta += chord / (step * theta)`, and the default archimedean arm is golden-tested against
  networkx values at `spiral/tests.rs:13-30`. What is true is narrower: no **equidistant** golden
  values are pinned, so a mutation of `CHORD` or `STEP` would survive. That is recorded as a test-coverage
  MINOR below, not a conformance MAJOR.
- **`planarity/lr/orient.rs:110` — "tie-break diverges from the reference's second `sorted`".** The
  reference's second sort (`refs/networkx-3.6/.../algorithms/planarity.py:373-375`) is
  `sorted(self.DG[v], key=…)`, which preserves `self.DG[v]`'s order on ties; the port's stable
  `slots.sort_by_key` preserves ascending slot order, and `Adjacency` rows are ascending by
  neighbour, so both are insertion-ordered — the tie-break difference is not observable as a
  different rotation. **Unverified** in detail; see below.
- **`basic_3d/cube.rs:95` — "`Vec::with_capacity(n as usize)` aborts instead of returning
  `StageError`".** `n` is a `u32`, so `n as usize` is exact on wasm32 and the registry's
  `scale_ceiling` is a host-side check, not a stage-level one — that is the documented house pattern
  for every layout, not a defect specific to this one.

### Additional test-coverage MINOR (evidence: reading the cited test)

| id | severity | file:line | defect | failing input | evidence | proposed fix |
|---|---|---|---|---|---|---|
| LR-39 | MINOR | `layout/spiral/tests.rs:91-113` | no golden values pin the **equidistant** arm against networkx, so a mutation of `CHORD` (1.0) or `STEP` (0.5) survives; the test only proves width-invariance and that the arm differs from the Archimedean one | `SpiralParams { resolution: 0.5, equidistant: true }` on `n = 7`: mutants to `STEP`→0.6 and `CHORD`→1.5 both stay green | `spiral/tests.rs:91-103` — no golden literal anywhere in the file for this arm; the default arm has them at `:13-30` | add one pinned point set for `equidistant: true` at a stated `resolution`, taken from `spiral_layout(..., equidistant=True)` |
| LR-40 | MINOR | `layout/spiral/tests.rs:1-113` | no test exercises a degenerate `resolution` (0, negative, NaN, inf) in either arm, which is why LR-01 shipped | `SpiralParams { resolution: 0.0, equidistant: true }` | every `SpiralParams` literal in the file is `0.35` or `0.5` | the four refusal cases, once LR-01's guard exists |

---

## Checked and found correct (0 findings)

Recorded so the next reviewer does not re-derive them.

- **`layout/bipartite` + `layout/bipartite/partition.rs` — 0 findings.** A faithful port of
  `_bipartite_parts` / `_greedy_max_cut` (`hierarchical.py:149-211`): the same dense-index walk
  (the reference's `G.nodes()` is dense too, `common.py:238`), the same BFS discovery order, the same
  even-sets orientation rule `abs(skew + len0 - len1) > abs(skew + len1 - len0)`, the same
  `side[node] = 0 if placed[0] <= placed[1] else 1`, the same `MAX_CUT_PASSES = 8`, and the same
  early `return None` at the first odd cycle (so one odd cycle *does* re-cut earlier components —
  that is the reference, not a drift). No hash order reaches output; ties are dense-index resolved.
  Placement in `bipartite.rs:45-51` is `np.linspace(0, 1, len)` per set, exactly
  `layout.py:433-434`, and the module's documented singleton-at-the-bottom behaviour is the
  reference's (`np.linspace(0, 1, 1) == [0.0]`). Degenerates covered: 0 nodes, 1 node, self-loop,
  triangle (non-bipartite), disconnected — all in `bipartite/tests.rs`.
- **`layout/planarity/{lr,lr/sign,lr/orient,adjacency,embed,triangulate,triangulate/faces}` — the LR
  port is faithful and the degenerate surface is covered.** `lr.rs:167` `too_dense` is
  `G.order()>2 and G.size()>3*order()-6` (`planarity.py:331`); `lr.rs:42` `conflicts_with` is
  `Interval.conflicting`; `lr.rs:73` `lowest` is `ConflictPair.lowest`; `lr.rs:78`'s `unreachable!`
  is the reference's own `lowpt[None]` KeyError; `lr/sign.rs:31` `resolve_side` is the iterative twin
  of `planarity.py:748-759`; the phase order at `lr.rs:176-183` matches `planarity.py:339-376`
  including that the second sort happens only after signing. **No recursion anywhere** in the eight
  files — every pass is an explicit stack/queue, so a 1e6-node graph cannot overflow the stack
  (`tests/properties.rs:31` pins a 100k path). No `HashMap`/`HashSet` reaches output;
  `adjacency.rs:28` `sort_unstable` on canonical `(u32,u32)` pairs makes the answer
  edge-insertion-order independent (`tests/properties.rs:18`, `tests/sweep.rs:110-117`). 0-node,
  1-node, no-edge, isolated, self-loop, multi-edge and out-of-range endpoints are covered at
  `adjacency.rs:125-150` and `tests/faces.rs:18-35`. `planarity.rs:258`'s Euler rearrangement
  `V + F + isolated == E + 2C` is algebraically right for a disconnected rotation system.
  `Triangulate`'s `HashSet` (`faces.rs:6,47`) is used only through `insert`/`contains`, never
  iterated, so no hash order reaches output (D2).
- **`layout/circular` + `circular/{ring,hierarchy}` — 0 findings beyond LR-12/LR-13.** `circular/hierarchy.rs`
  reproduces `_circular_hierarchy_layout` (`hierarchical.py:718-729`) exactly, including the
  `level == 0 && count == 1 → 0.0` case and the `max(2, max_level)` floor via `deepest.max(2)`, and
  `points` matches `(i / count) * 2 * np.pi`. `circular.rs` cannot divide by zero: `n = 0`
  short-circuits, and `count` is provably ≥ 1 for every ring `counts` contains;
  `seen[ring as usize]` cannot go out of bounds because `width = deepest + 1`; and
  `Hierarchy::of` is refused (`StageError::Capacity`) before any depth ≥ `u32::MAX`.
  `ring.rs:63-68` returns zeros before `1.0 / f64::from(count)` is formed, so `n = 0` and `n = 1`
  never divide. Slot assignment walks rings in dense-index order; all trigonometry is `libm`.
  The module's own Ponytail ("no two real nodes ever collide", `circular.rs:41`) holds even in `f32`:
  the angle is what carries the separation, and `y = r*sin(theta)` stays distinct long after `x`
  collapses to `1.0` at ring populations above ~10⁶.
- **`layout/coords.rs` + `coords/probe.rs` — 0 findings beyond LR-14/LR-15.** `merge`
  (`coords.rs:60-70`) is bit-for-bit `Iterator::sum` (asserted at `coords.rs:26-50`), so it cannot be
  split by a runner; `limit > 0.0` is exactly networkx's zero-span guard and, because `limit` is the
  max over the recentred values, the quotient is bounded by 1.0; `v as f32` is a saturating
  float-to-float cast, so no truncation or UB path; no `usize` on the wire, no hash order, no clock,
  no `mul_add`/`powi`; 109 lines, longest function 14, widest signature 4. `probe.rs` is
  `#[cfg(test)]`-only, so its `expect`/`panic!` are unreachable from production.
- **`layout/random.rs` — 0 findings beyond LR-31/32/33.** `synthetic.rs:49` computes
  `f64::from(u32) / 4_294_967_296.0`, an exact power-of-two scale of a full 32-bit word, so the range
  is uniformly `[0,1)` — no modulo bias, no inclusive/exclusive off-by-one — and an `f64→f32`
  narrowing of a value `< 1.0` cannot round up to `1.0`, so no node escapes the declared square.
  Draws are in dense-index order, 2 per node, no reduction. 0/1-node and edgeless cases are covered.
- **`layout/grid.rs` + `grid/scaled.rs` — no `usize`-width divergence, no rounding question.** All
  kernel index arithmetic is `u32` (`i % cols`, `i / cols`, with `i < count ≤ u32::MAX`), and
  `dimensions` is `isqrt`-based with `rows = n.div_ceil(cols)`, so `cols * rows` is **never formed** —
  there is no expression that overflows on wasm32 but not natively. `cols ≥ 1` for every `n ≥ 1`, so
  no `% 0`; every `i in 0..count` is written, so no node is left at a default `(0,0)` and there is no
  "missing cell" state; and there is no `ceil`/`floor`/`round` on a cell index at all, so the
  boundary question does not arise. Every `expect`/`assert`/`panic!` is inside `#[cfg(test)]` —
  none is reachable from `Stage::run`/`run_with`/`run_scaled` — and `spacing`/`scale` NaN and ±inf
  are both rejected and tested.
- **`layout/mod.rs` — structurally clean.** All 21 declared modules have both `<name>.rs` and
  `<name>/`; `adjacency` and `coords` are correctly private `mod`s; `tests` is `#[cfg(test)]`-gated;
  there is no `pub use` at all, so no `pub use` of a private item and no module declared-but-not-`mod`-ed.
  146 lines, longest function 26 (`snapshot`), widest signature 4 (`Geometry::in_space`), every `pub`
  item documented. `snapshot` sorts notes and routes every column, `z` included, through the one
  finiteness check (`crates/graph-contract/src/geometry.rs:277-285`), which is what makes LR-01,
  LR-02 and LR-20 refusals rather than corrupt output.
- **House limits across the whole scope.** No file exceeds 300 lines (`grid.rs` is the closest, at
  296 — LR-23); no function exceeds 40 lines (the longest in scope is `grid.rs:100-118` at 19);
  determinism-wise: no `mul_add`, no `powi`, no relaxed-simd, no wall-clock, no unseeded randomness
  (the only draws are `Mulberry32` at fixed `SEED`s in `random.rs` and `basic_3d/cube.rs`), no
  `HashMap`/`HashSet` iteration anywhere in an output path, no `usize` on the wire, every kernel
  gather-form.

---

## Unverified

Carried from the subagents, not confirmed here. Each is a question, not a finding.

- **`layout/planarity/lr/orient.rs:110` vs the reference's second sort.** The port stable-sorts by
  `nesting_depth` over ascending slots; `planarity.py:373-375` stable-sorts over `self.DG[v]`'s
  order. Both are insertion-ordered, so I could not construct a graph where the tie-break yields a
  **different rotation**; I did not prove they agree on all graphs. The practical consequence would
  be an oracle test comparing rotations, not planarity verdicts — the planarity answer is
  order-independent either way (`adjacency.rs:28` canonicalises the edge set).
- **`layout/planarity/adjacency.rs:100`'s "caller-guaranteed" invariant.** The guarantee lives in
  `lr/testing.rs` and `embed.rs`, which are in scope but which I read only at the call sites cited in
  LR-25/LR-30, not exhaustively.
- **`crates/graph-core/src/registry.rs`'s `LAYOUTS` array order** (load-bearing by index per
  `graph-wasm/src/exports/build.rs` and `bench/campaign.rs`, type-checked only on length). Raised in
  `docs/reviews/review-layout-tree.md` as L-28 for the layouts that job covered; the rows for this
  job's ids are in the same array and the same guard (or absence of one) applies. Not re-reported.
- **`docs/measurements/tiers-audit.md:47`** cites `random.rs:25` / `:26-28` where `run` is `:33` and
  its body `:35-37`. Outside this job's paths (the job writes one file), so recorded here only.

## Conformance-matrix rows cited, not re-reported

From `docs/measurements/scigraphs-conformance.md`, and deliberately **not** duplicated above:

- **Row 1 `RANDOM` / `layout.random`** — "same size, different shape: two independent uniform
  draws". Our `SEED` is not numpy's `RandomState` stream (`registry/closed_form.rs:24-30` declares
  this). That is LR-33's scope note, not a defect.
- **Row 2 `GRID` / `layout.grid`** — "same shape; only the last `f32` rounding is left", max abs
  2.12e-07. Nothing in LR-20/LR-21/LR-22 contradicts it; those are about *degenerate parameters*,
  not the gate's model.
- **Row 14 `SPIRAL_3D` / `layout.spiral`** — "different shape: grey is a 3D spiral, green one point
  at the centre". SciGraphs' `SPIRAL_3D` is a conical 3D curve (`basic.py:36`) and the motor's is
  networkx's planar spiral; the matrix names the disagreement, and the engine chose the networkx
  oracle (`registry/closed_form.rs:59-61`). Not re-reported.
- **Row 16 `CUBE` / `layout.basic3d.cube`** — "the eight corners land on the grey corners; the
  interior is redrawn from another generator". This is the declared seeding decision. LR-05 is
  about the *test* that backs it, which the matrix does not cover.
- **Rows 12/15 `SPHERE`/`HELIX`, 18 `BIPARTITE_3D`, 32 `CIRCULAR_HIERARCHY`** — all "same shape"
  within ~2.4e-07. LR-03 and LR-06/07/08 are doc claims about the same code, which a shape
  comparison cannot catch; they do not contradict these rows.

---

## Counts by module

| module | files read | BLOCKER | MAJOR | MINOR |
|---|---|---|---|---|
| `layout/basic_3d.rs` | 1 | 0 | 1 | 3 (LR-10 ×2 sites, LR-11) |
| `layout/basic_3d/sphere.rs` | 1 | 0 | 0 | 1 (LR-06) |
| `layout/basic_3d/helix.rs` | 1 | 0 | 0 | 1 (LR-07) |
| `layout/basic_3d/cube.rs` (+tests) | 2 | 0 | 1 | 3 (LR-08, LR-09, LR-05) |
| `layout/basic_3d/bipartite_3d.rs` (+tests) | 2 | 0 | 0 | 0 |
| `layout/bipartite.rs` + `partition.rs` (+tests) | 3 | 0 | 0 | 0 |
| `layout/circular.rs` | 1 | 0 | 0 | 1 (LR-12) |
| `layout/circular/ring.rs` | 1 | 0 | 0 | 1 (LR-13) |
| `layout/circular/hierarchy.rs` | 1 | 0 | 0 | 0 |
| `layout/coords.rs` + `probe.rs` (+tests) | 3 | 0 | 0 | 2 (LR-14, LR-15) |
| `layout/planarity.rs` | 1 | 0 | 0 | 2 (LR-24, LR-36 seam) |
| `layout/planarity/adjacency.rs` | 1 | 0 | 0 | 1 (LR-30) |
| `layout/planarity/embed.rs` | 1 | 0 | 0 | 2 (LR-25, LR-26) |
| `layout/planarity/triangulate.rs` (+faces.rs) | 2 | 0 | 0 | 4 (LR-27, LR-28, LR-29) |
| `layout/planarity/lr.rs`, `lr/sign.rs`, `lr/orient.rs` | 3 | 0 | 0 | 0 |
| `layout/random.rs` (+tests) | 2 | 0 | 0 | 3 (LR-31, LR-32, LR-33) |
| `layout/spiral.rs` (+tests) | 2 | 0 | 1 | 5 (LR-16…LR-20) |
| `layout/grid.rs` | 1 | 0 | 0 | 2 (LR-20, LR-23) |
| `layout/grid/scaled.rs` | 1 | 0 | 1 | 2 (LR-21, LR-22) |
| `layout/adjacency.rs` | 1 | 0 | 0 | 2 (LR-37, LR-38) |
| `layout/mod.rs` (+tests) | 2 | 0 | 0 | 2 (LR-34, LR-35) |
| `registry/three_d.rs`, `registry/closed_form.rs`, `registry/grid.rs`, `registry/hierarchy.rs` | 4 | 0 | 1 | 0 |
| **total** | **31** | **0** | **5** | **40** |

Commands run (all through `scripts/orch/gr`, the only permitted toolchain):

```
scripts/orch/gr cargo test -p graph-core --lib layout::planarity                    -> 0  (74 passed)
scripts/orch/gr cargo test -p graph-core --lib layout::                              -> 0  (754 passed, 2 ignored)
scripts/orch/gr cargo test -p graph-core --lib scratch_probe -- --nocapture          -> 0  (planarity: bridge/pendant,
                                                                                                 self-loop, multi-edge probes)
scripts/orch/gr cargo test -p graph-core --lib scratch_probe_huge -- --nocapture    -> 0  (grid/scaled: scale 1e300,
                                                                                                 f64::MAX, spacing f32::MAX)
scripts/orch/gr cargo test -p graph-core --lib scratch_probe_spiral_nan -- --nocapture -> 0 (resolution 0 / -0.35 / NaN,
                                                                                                 and the snapshot refusal)
```

Every scratch test was deleted; `git status --short` is empty at the end of this job.