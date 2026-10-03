# SciGraphs conformance: 32 layouts, byte by byte

Every name `apply_graph_layout` dispatches on, compared **byte for byte** against the motor
layout it is supposed to be, on one fixture set both arms read, with every row classified by
its first cause and tiered by what it would take to make it agree.

`docs/measurements/scigraphs-coverage.md` answers *does it agree within a tolerance*. This
answers the question a tolerance hides: **how close, in bytes, and what is the first thing
standing in the way.** Run it with `scripts/scigraphs-conformance.sh`; judge it with
`graph-cli scigraphs-conformance`, which gates on the pinned baseline in
`crates/graph-cli/src/oracle_python/conformance/baseline/table.rs`.

## What was measured, and on what

**24 fixtures, emitted once and read by both arms** (`conformance.jsonl`): `lesmis` (SciGraphs'
own gallery graph, 77 nodes / 254 edges), the gate model at seeds 0..19 (2..21 nodes each, 230
nodes in total — the 200-node cap is a bound no seed reaches, and a seed that would reach it is
refused rather than clipped, because a clipped graph measures a different layout), one rooted
tree (`fixtures/hierarchy/tree-balanced.json`, 15 nodes), one DAG (`fixtures/dag/diamond.json`,
4 nodes) and one bipartite graph (`K(6,8)`, 14 nodes / 48 edges, written out because the repo
has no bipartite generator). 340 nodes and 1020 coordinates per row; **23** of the 24 fixtures
are compared for every row except `IGRAPH_KK`, which reaches 22.

**Node order is the contract.** SciGraphs' graph is a plain dict and the motor's is a topology,
so the rule *SciGraphs node `i` is the motor node whose id sorts `i`-th in byte order* is stated
in the fixture and **checked by the arm** (`sc_fixture.Fixture._check_order`): it sorts the ids
and refuses if the order it recovers is not the order the fixture listed. `tree-balanced.json`
lists `r` before `a`, so its order is genuinely not byte order and the fixture re-lists it.

**The motor arm** (`graph-cli emit-conformance-fixtures`) runs every motor id over every fixture
at SciGraphs' own `iterations = 50`, `scale = 5.0` and `get_layout_seed() = 981798123`
(`derive_seed(42, "layout")`, `SciGraphs/core/scigraphs_core/repro/determinism.py:56-62`), and
writes **raw little-endian `f64`** per row plus the `f32` the snapshot narrows to. A decimal
round trip in the middle would be a rounding step between the two values whose equality is the
question. Exactly **five** ids get an override. Three because their registered default is not
SciGraphs' parameter: `CIRCLE_PACKING` (500 sweeps, not 50), `FORCEATLAS2` (`max_iter` 100, not
50) and `GRAPHVIZ_SFDP` (`run` hard-codes `DEFAULT_SEED = 1`; the arm calls `run_seeded`). The
other two are layouts whose *placement* is SciGraphs' rather than the registered stage's, both
through a scaled entry point beside the registered one and both at `scale = 5.0`:
`layout.grid`, whose arm calls `Grid::run_scaled` because `_grid_layout` starts the first cell
at the origin and pitches it at `scale / grid_size`, and `layout.dag.sugiyama`, whose arm calls
`sugiyama::run_scaled`, which applies SciGraphs' per-axis normalisation
(`hierarchical.py:679-685`) at the same `scale = 5.0`. That normalisation reads the dummy
vertices' X, which `Geometry` does not carry, so it lives beside the stages that produce it
rather than in this arm.

**The reference arm** calls `apply_graph_layout` itself for 23 names in `ge-python-oracle` with
the `SciGraphs/` submodule on the path. The other nine go through `scigraphs_utils`, which is in
neither oracle image, so they take the pinned Graphviz 16.1.0 engine's own `-Tplain` points at
`-Gstart=981798123` — the arm `harness/oracle-graphviz.py` already drives. `YIFAN_HU` is among
them and that is a finding rather than a placement: SciGraphs runs it through the same sfdp call
as `GRAPHVIZ_SFDP` (`yifan_hu.py:366`, and `yifan_hu.py:11` is the name-to-engine map), so
those two rows compare two different motor layouts against **one** reference.

**Both arms' defaults are recorded.** `ref/<NAME>.json` carries the library versions the
reference ran on and any `layout_substituted` SciGraphs reported, because agreement here is
against one pinned build and a different scipy would move the last digit of a spectral
eigenvector. `layout_substituted` is `null` on all 32 rows: no library was missing and no
fallback fired.

## The tier rule

A row's tier is **what it would take to make the two coordinate lists identical**, and it is
about the `f32` narrowing rather than the `f64`, for a reason worth stating once:

> **The motor is `f32` end to end.** `graph_contract`'s node geometry is `Vec<f32>`
> (`crates/graph-contract/src/canonical_json/schema.rs:110`), so the `f64` file is an `f32`
> widened and **no `f64` agreement is structurally reachable** except where the reference's own
> value happens to be `f32`-representable — `0.0`, small halves, `CUBE`'s literal corners. That
> is why the matrix reports both counts: `f64 k/N` says whether even the `f32` was exact, and
> `f32 k/N` is the strongest statement this tree can make.

- **`bitwise`** — byte equality is reachable **without changing graph-core's algorithm**: a
  closed form, a scale or centring convention undoable in one place, or an RNG that can be
  ported away. This is the tier a repair job moves a row *up* to.
- **`tolerance`** — both arms are deterministic and run the same method, and agree in shape to
  a few parts per million; only the reduction order and the `libm` differ. Reachable, but not at
  a sane cost: it is an `f64` kernel and the motor's columns are `f32`.
- **`shape`** — the two arms do not start from the same place, do not run the same method, or
  there is no reference to compare against. Only the picture is comparable.

## The cause rule

Every row that is not bitwise equal is classified by its **first** cause, in this order, and the
order is the argument: a row can have three of them and the repair that matters is always the
one that unlocks the rest.

| cause | test | what it means |
|---|---|---|
| `reference-absent` | the arm produced no comparable coordinates | the row cannot be measured here |
| `arithmetic` | disparity <= 1e-3 **and** max gap <= 1e-6 | same method, different summation order or `libm` |
| `rng` | the row declares a `layout seed` gap | the two arms cannot start alike; undoing the scale would not make the bytes agree |
| `convention` | disparity <= 1e-6 | the same shape to a part in a million, and what is left is units, centring, axis order or `z` |
| `algorithm` | anything else | a different method or a different step |

**Both halves of the `arithmetic` test are needed, and the gap is the half that matters.**
`SUGIYAMA` has a disparity of 1.26e-16 and an absolute gap of 1.91e-07: the same drawing, one
`f32` ULP out. Before `GRID` was repaired it sat at a disparity of 5e-32 and an absolute gap of
4.0 — the same lattice, in a different unit at a different origin — and calling that `arithmetic`
because the shape was exact would have sent a repair job to chase a summation order that was
already right. `GRID` is now `f32` 1020/1020 with a gap of 2.12e-07, so the row that makes the
argument is one this series has already fixed (`docs/measurements/sg-grid-scale.md`).

**The `convention` threshold is 1e-6 and it is measured, not chosen.** Every row this
classifier calls a convention measures between 2e-10 (`GRAPHVIZ_TWOPI`) and 4e-10
(`GRAPHVIZ_PATCHWORK`); every row that measures 0.08 to 0.7 does not, and its overlay says the
same thing. An earlier 0.35 threshold here called `SPECTRAL_3D`, `MDS_3D`, `GRAPHVIZ_CIRCO` and
`CIRCLE_PACKING` conventions, and all four are a different shape in the picture — the threshold
was wrong, not the data.

**The discriminator is the worst of the median and the two named fixtures**, not the median.
Twenty of the 24 fixtures hold 2 to 21 nodes, so the median is dominated by them:
`CIRCLE_PACKING` measures 5e-16 on the gate models and 0.517 on `lesmis`, and a cause decided
on the median alone is a cause about a graph nobody looks at. `sc_propose.NAMED_FIXTURES` is the
pair the shape verdicts are read from.

**Equality is over the IEEE-754 bits, not `==`.** `+0.0 == -0.0` in every language and zero is a
third of this matrix's coordinates, so `sc_metrics.bit_counts` compares encodings; the ULP
distance keeps the monotone ordinal, which deliberately maps `-0.0` onto `0.0`.

## The matrix

Cells are over all the fixtures the row could be measured on. `max ULP` is the largest distance
in units in the last place over the coordinates; `max gap` the largest `|ours - theirs|`; the two
Procrustes columns are `scipy.spatial.procrustes`'s own disparity — the sum of squared
differences after a translation, a uniform scale and a rotation — as a median and a max. The
shape verdict is what the SVG panels show: **grey is SciGraphs, green is the motor
Procrustes-aligned over it**, so a green point sitting on a grey point is a node that matches.

| # | SciGraphs name | motor id | reference reached | tier | f64 k/N | f32 k/N | max ULP | max gap | Procrustes med | Procrustes max | cause | shape verdict |
|--:|---|---|---|--:|--:|--:|--:|--:|--:|--:|---|---|
| 1 | `RANDOM` | `layout.random` | `apply_graph_layout` | `tolerance` | 0/1020 | 1020/1020 | 2.68e+08 | 2.35e-07 | 2.28e-15 | 2.95e-15 | `arithmetic` | **same shape** — the green cloud sits on the grey one, node for node; the `f64` column cannot be exact because the motor is `f32` |
| 2 | `GRID` | `layout.grid` | `apply_graph_layout` | `tolerance` | 842/1020 | 1020/1020 | 2.39e+08 | 2.12e-07 | 3.39e-32 | 3.96e-15 | `arithmetic` | **same shape** — the aligned motor lands on every grey lattice point; only the last `f32` rounding is left |
| 3 | `SPRING` | `layout.force.spring` | `apply_graph_layout` | `bitwise` | 362/1020 | 866/1020 | 5.17e+13 | 2.37e-03 | 3.77e-16 | 1.66e-08 | `convention` | **same shape on every fixture** (Procrustes median 3.77e-16, worst 1.66e-08), and `f32`-identical on 21 of the 23 measured ones. 153 of the 154 coordinates that are not identical are on `lesmis` (78/462) — the 77-node fixture whose 50 chaotic iterations amplify a 1.6-ulp reduction difference to 2.4e-3; `gate-16` carries the last one (53/54). The `convention` label is the classifier's, and it is wrong — see Repair 4 |
| 4 | `SPRING_3D` | `layout.force.spring3d` | `apply_graph_layout` | `tolerance` | 24/1020 | 1020/1020 | 2.68e+08 | 2.36e-07 | 5.03e-16 | 6.67e-16 | `arithmetic` | **same shape** — the same seed, the same kernel and the same split reduction as `SPRING`, and the third column absorbs the whole difference: 1020/1020 `f32` |
| 5 | `CIRCLE_PACKING` | `layout.packing.circle` | `apply_graph_layout` | `shape` | 344/1020 | 808/1020 | 9.22e+18 | 2.70 | 5.3e-16 | 0.863 | `algorithm` | **bit-for-bit the same packing on the 20 gate models** (5e-16) and on the two planar fixtures. `lesmis` — the non-planar one, so the only fixture whose seed moved — goes **0.517 -> 0.0895**; `bipartite` is non-planar too and still differs (0.863); `tree-balanced` (0.418) is a **tree**, so it takes the exact path and did not move |
| 6 | `FORCEATLAS2` | `layout.forceatlas2` | `apply_graph_layout` | `bitwise` | 0/1020 | 0/1020 | 9.25e+18 | 183 | 0.241 | 0.927 | `rng` | different shape |
| 7 | `IGRAPH_FR` | `layout.force.fruchterman_reingold` | `apply_graph_layout` | `bitwise` | 0/1020 | 0/1020 | 9.24e+18 | 11.6 | 0.267 | 0.901 | `rng` | different shape |
| 8 | `IGRAPH_KK` | `layout.force.kamada_kawai` | `apply_graph_layout` | `shape` | 0/957 | 0/957 | 9.23e+18 | 7.95 | 0.812 | 0.935 | `algorithm` | different shape: grey is a blob, green is a near-straight line |
| 9 | `IGRAPH_DRL` | `layout.force.drl` | `apply_graph_layout` | `bitwise` | 0/1020 | 0/1020 | 9.25e+18 | 52.8 | 0.536 | 0.881 | `rng` | both are near-collinear; green runs along the grey line with different spacing |
| 10 | `IGRAPH_DRL_2D` | `layout.force.drl` | `apply_graph_layout` | `bitwise` | 340/1020 | 340/1020 | 9.25e+18 | 50.4 | 0.514 | 0.971 | `rng` | different shape (the same motor layout as `IGRAPH_DRL`, run without its z) |
| 11 | `IGRAPH_LGL` | `layout.force.lgl` | `apply_graph_layout` | `bitwise` | 340/1020 | 340/1020 | 9.24e+18 | 33.1 | 0.611 | 0.81 | `rng` | different shape |
| 12 | `SPHERE` | `layout.basic3d.sphere` | `apply_graph_layout` | `tolerance` | 111/1020 | 1020/1020 | 2.68e+08 | 2.38e-07 | 4.63e-16 | 9.66e-16 | `arithmetic` | **same shape** — the green ring sits on the grey ring, node for node |
| 13 | `SPECTRAL_3D` | `layout.spectral` | `apply_graph_layout` | `shape` | 1/1020 | 1/1020 | 9.22e+18 | 5.95 | 0.333 | 0.807 | `algorithm` | different shape: grey is a vertical line, green a small cluster at one end — and **both arms start from the origin with no RNG**, so this is the algorithm |
| 14 | `SPIRAL_3D` | `layout.basic3d.spiral` | `apply_graph_layout` | `tolerance` | 120/1020 | 1020/1020 | 2.68e+08 | 2.35e-07 | 3.34e-16 | 5.59e-16 | `arithmetic` | **same shape** — green covers grey node for node on 22 of 24 fixtures |
| 15 | `HELIX` | `layout.basic3d.helix` | `apply_graph_layout` | `tolerance` | 327/1020 | 1020/1020 | 2.67e+08 | 1.51e-07 | 1.37e-16 | 4.16e-16 | `arithmetic` | **same shape**, mirrored on 4 of the 22 fitted fixtures |
| 16 | `CUBE` | `layout.basic3d.cube` | `apply_graph_layout` | `tolerance` | 501/1020 | 1020/1020 | 2.68e+08 | 1.19e-07 | 5.72e-17 | 3.29e-16 | `arithmetic` | **same shape** — corners and interior alike; the 501/1020 `f64` are the `3*min(n, 8)` corner coordinates of every one of the 24 fixtures, all of them `±5.0` or `0.0` and so `f32`-representable |
| 17 | `HIERARCHICAL_3D` | `layout.hierarchical3d` | `apply_graph_layout` | `tolerance` | 374/1020 | 1020/1020 | 2.67e+08 | 7.95e-08 | 9.03e-17 | 2.93e-16 | `arithmetic` | **same shape** — green covers grey node for node |
| 18 | `BIPARTITE_3D` | `layout.bipartite_3d` | `apply_graph_layout` | `tolerance` | 480/1020 | 1020/1020 | 2.65e+08 | 1.18e-07 | 3.57e-16 | 5.26e-16 | `arithmetic` | **same shape** — the two rings land on the reference's two rings, node for node; the f64 residue is numpy's `cos`/`sin` against `libm`'s |
| 19 | `IGRAPH_DH` | `layout.force.davidson_harel` | `apply_graph_layout` | `bitwise` | 340/1020 | 340/1020 | 9.24e+18 | 34.7 | 0.767 | 0.99 | `rng` | different shape |
| 20 | `IGRAPH_GRAPHOPT` | `layout.force.graphopt` | `apply_graph_layout` | `bitwise` | 340/1020 | 340/1020 | 9.26e+18 | 189 | 0.557 | 0.919 | `rng` | different shape |
| 21 | `MDS_3D` | `layout.mds.pivot` | `apply_graph_layout` | `shape` | 0/1020 | 0/1020 | 9.22e+18 | 5.84 | 0.0783 | 0.541 | `algorithm` | different shape, and the closest of them (median 0.078) |
| 22 | `YIFAN_HU` | `layout.force.yifan_hu` | `sfdp -Tplain` | `shape` | 340/1020 | 340/1020 | 9.29e+18 | 508 | 0.829 | 0.985 | `algorithm` | different shape: the motor is a Barnes-Hut port and the reference is Graphviz sfdp |
| 23 | `GRAPHVIZ_DOT` | _none_ | `dot -Tplain` | `shape` | not run | not run | not run | not run | not run | not run | `reference-absent` | **not run:** not run: no motor layout for this name |
| 24 | `GRAPHVIZ_NEATO` | `layout.force.neato` | `neato -Tplain` | `bitwise` | 340/1020 | 340/1020 | 9.26e+18 | 473 | 0.424 | 0.95 | `rng` | different shape |
| 25 | `GRAPHVIZ_FDP` | `layout.force.fdp` | `fdp -Tplain` | `bitwise` | 344/1020 | 344/1020 | 3.10e+16 | 2.06e+03 | 0.661 | 0.944 | `rng` | different shape _(reference not pinned: the engine's own start is not seeded by -Gstart: two runs differ)_ |
| 26 | `GRAPHVIZ_SFDP` | `layout.force.sfdp` | `sfdp -Tplain` | `shape` | 345/1020 | 345/1020 | 1.71e+16 | 345 | 0.848 | 0.978 | `algorithm` | different shape at the **same seed on both sides**: grey is a line with a fan, green a small cluster |
| 27 | `GRAPHVIZ_TWOPI` | `layout.twopi` | `twopi -Tplain` | `bitwise` | 340/1020 | 340/1020 | 9.28e+18 | 303 | 2.04e-10 | 7.36e-10 | `convention` | **same shape** — the green ring sits on the grey ring; units only |
| 28 | `GRAPHVIZ_CIRCO` | `layout.circular.circo` | `circo -Tplain` | `shape` | 340/1020 | 340/1020 | 9.31e+18 | 6.43e+03 | 0.284 | 0.875 | `algorithm` | **same shape on the tree** (disparity 6.5e-05) and **different on lesmis** (0.308): the ring agrees where the tree is small and the boxes are equal |
| 29 | `GRAPHVIZ_OSAGE` | `layout.packing.osage` | `osage -Tplain` | `shape` | 387/1020 | 387/1020 | 1.95e+16 | 498 | 0.711 | 0.964 | `algorithm` | same grid of rows, different row assignment: the y coordinates agree to 1e-5 of the span, the x to 7% |
| 30 | `GRAPHVIZ_PATCHWORK` | `layout.treemap.patchwork` | `patchwork -Tplain` | `bitwise` | 340/1020 | 340/1020 | 9.27e+18 | 139 | 4.32e-10 | 1.6e-09 | `convention` | **same shape** — green on grey |
| 31 | `SUGIYAMA` | `layout.dag.sugiyama` | `apply_graph_layout` | `tolerance` | 597/1020 | **1020/1020** | 2.57e+08 | 1.91e-07 | 1.26e-16 | 4.63e-16 | `arithmetic` | **same shape**, and `f32`-identical on all 1020 coordinates |
| 32 | `CIRCULAR_HIERARCHY` | `layout.circular.hierarchy` | `apply_graph_layout` | `tolerance` | 509/1020 | 1020/1020 | 2.67e+08 | 2.2e-07 | 4.32e-16 | 1e-15 | `arithmetic` | **same shape**, and `f32`-identical on all 1020 coordinates |

Every row's **convention gaps** — the parameters of `apply_graph_layout` the motor has no slot
for, each with the line the fixed value is written on — are in
`crates/graph-cli/src/oracle_python/conformance/gaps.rs` and reach `motor.jsonl` verbatim.

Pictures, all 64 rendered by the script and all looked at:

- [all 32 on lesmis](../target/scigraphs-conformance/png/sheet.png) (`sheet.png`)
- [all 32 on the rooted tree](../target/scigraphs-conformance/png/sheet-tree-balanced.png)
- one pair per name: `target/scigraphs-conformance/png/<NAME>-lesmis.png` and
  `target/scigraphs-conformance/png/<NAME>-tree-balanced.png`

## What the matrix says that a tolerance could not

**1. Ten rows are `f32`-identical on every one of 1020 coordinates and the rest of their gap is
the narrowing.** `SPHERE`, `HELIX`, `HIERARCHICAL_3D`, `CIRCULAR_HIERARCHY`, `GRID` (after
`sg-grid-scale`), `BIPARTITE_3D`, `SPIRAL_3D`, `SUGIYAMA`, and — after `sg-mt19937` — `RANDOM` and
`CUBE`: 10 of 32. Their max gaps run from 7.9e-8 to 2.4e-7, one `f32` ULP at that magnitude, and
their Procrustes medians from 3.4e-32 (`GRID`) to 2.3e-15 (`RANDOM`) — all of them the same shape
to machine precision. `CIRCULAR_HIERARCHY` and `SUGIYAMA` are the strongest rows in the matrix.
`SUGIYAMA` came from `shape`/`algorithm` in the previous run; `docs/measurements/sg-sugiyama.md`
has the per-stage diff and the two causes it found, and
`docs/measurements/sg-grid-scale.md` has `GRID`'s. The two that arrived by porting the
reference's own generator rather than by fixing a convention are the proof that `f64 k/N` is not
the target: their `f64` counts are 0/1020 and 501/1020, and every one of those coordinates is
`f32`-exact — `RANDOM`'s because no draw is a `f32` value, `CUBE`'s because its 501 are the corner
coordinates `CORNERS[i] * 5.0`, which are `±5.0` and `0.0`.

**2. Two rows are the same shape to `1e-10` or better and differ only in units.**
`GRAPHVIZ_TWOPI` (2e-10), `GRAPHVIZ_PATCHWORK` (4e-10). Each is a convention fix, not an
algorithm; `GRID` (5e-32) was the third until `sg-grid-scale` fixed its units (row 2).

**3. `GRAPHVIZ_SFDP` differs at the same seed on both sides.** The motor arm calls
`sfdp::run_seeded(981798123)` and the engine is given `-Gstart=981798123`; the disparity is 0.848.
Same seed, same engine, different answer — so the cause is `algorithm` and no amount of seed
plumbing will reach it.

**4. igraph's RNG cannot be seeded from Python at all.** `_reset_layout_rng`
(`SciGraphs/core/scigraphs_core/mesh/layouts/common.py:60`) seeds `np.random.RandomState` and the
stdlib `random`; igraph reads the C library's generator, which neither call reaches. So
`IGRAPH_FR`, `IGRAPH_DRL`, `IGRAPH_DRL_2D`, `IGRAPH_LGL`, `IGRAPH_DH` and `IGRAPH_GRAPHOPT` have
**no seedable reference start at all**, and their `rng` cause is not a missing port.

**5. One row's reference is not reproducible.** Two consecutive `--graphviz` runs over the same
fixtures and the same `-Gstart` were compared file by file: 31 of 32 reference files were
byte-identical and `GRAPHVIZ_FDP.f64` differed. Graphviz's FDP does not seed its own start from
`-Gstart`. Its reference bytes therefore **cannot be pinned**, and the row is gated on its motor
bytes, on-disk re-hashing and its measured shape with the reason printed.
`verdict::tests::an_unreproducible_reference_passes_but_still_gates_the_motor` holds it to its
shape: a note with a moved motor sha still fails.

**6. `YIFAN_HU` and `GRAPHVIZ_SFDP` share one reference and disagree with each other.** SciGraphs
runs both through sfdp (`yifan_hu.py:366`); the motor has `layout.force.yifan_hu` — whose solver is
`layout::force::barnes_hut`, reused rather than a port (`force/yifan_hu.rs:8`, `:16`) — and
`layout.force.sfdp`. Two rows, one answer, two different motor layouts.

**7. The median over the fixture set is dominated by the twenty tiny gate models, and two rows hide
behind it.** `CIRCLE_PACKING` is 5e-16 on the gate models and **0.517 on lesmis**;
`GRAPHVIZ_OSAGE` is 3.6e-10 at best and **0.902 on lesmis**. Both rows' median reads like the gate
models and neither picture does. **Read the max column and the picture, not the median, when the
median and the picture disagree** — which is why both are in the table and why the classifier
takes the worst of the three.

**8. `GRAPHVIZ_OSAGE`'s two drawings are the same grid with a different row assignment.** Sorting
the y coordinates of both arms and differencing them gives 1e-5 of the span; the x coordinates give
7%. Same rows, different columns — a much smaller repair than "the layouts differ".

**9. `SPECTRAL_3D` and `MDS_3D` have no RNG on either side.** Both graph-core layouts start from
`vec![0.0; n * DIMS]` (`spectral.rs:245`, `pivot_mds.rs:240`) and both SciGraphs wrappers
delegate to networkx's eigen-decomposition. So their cause is `algorithm` — the degenerate
eigenvector's sign and scale are resolved differently — and not the `rng` an earlier reading of
this table gave them.

**10. `IGRAPH_KK` is not close, and it is deterministic.** igraph's Kamada-Kawai is documented as
deterministic, so its `rng` cause does not apply; the disparity of 0.812 is a different solver.

## Repairs, ordered by what they buy

Each is a concrete change with the metric it should move. **They are in separate jobs**: this one
measures and changes nothing under `crates/graph-core`.

### 1. `GRID` — `convention`, one line, a whole row — **landed**
**File:** `crates/graph-core/src/layout/grid/scaled.rs` (new); the registered
`crates/graph-core/src/layout/grid.rs` is unchanged. **Change:** `_grid_layout(num_nodes, scale)`
(`basic.py:16-17`) starts the first cell **at the origin** and pitches it at `scale / grid_size`,
where the registered stage centres the full lattice at `GridParams::spacing = 1.0` and nothing
rescales it. So `Grid::run_scaled(topology, scale, runner, workers)` is the second placement — the
shape of `sfdp::run_seeded` — and the conformance arm calls it. It is **`f64` inside**: `basic.py:16`
is `(i % cols) * scale / cols` in Python floats, and a `f32` pitch is a whole ULP off
(`scale = 5.0, cols = 9, k = 3`: `3 · fl32(5/9) = 1.6666667…`, `fl32(15/9) = 1.6666666…`), so
folding it into the registered `f32` kernel would have cost the row. `GridParams` gained **no
field**: eight struct-literal call sites in five files build it without `..Default::default()`, and
the registered default's bytes are a snapshot hash.
**Measured:** `max gap` 4.0 → 2.12e-07, `max ULP` 9.22e+18 → 2.39e+08, `f32` 342/1020 → **1020/1020**,
`f64` 342/1020 → 842/1020, Procrustes median 5.5e-32 → 3.39e-32. Tier `bitwise` → `tolerance` and
cause `convention` → `arithmetic`: the `f64` column cannot reach 1020/1020 because the reference's
value is not `f32`-representable, which is the position `SPHERE`, `HELIX` and `HIERARCHICAL_3D` are
already in. Commands, before/after lines and the `--break` run: `docs/measurements/sg-grid-scale.md`.

### 2. `GRAPHVIZ_TWOPI`, `GRAPHVIZ_PATCHWORK` — `convention`, the same class, two files
**Files:** `crates/graph-core/src/layout/radial/twopi.rs:104`,
`crates/graph-core/src/layout/graphviz/patchwork/squarify.rs:41`. **Change:** the motor works in
its own units and the reference arm reports the engine's points in **points**; the layout's own
extent is the missing scale. Normalising to `scale = 5.0` the way the three layouts that already
publish a `SCALE` const do (`basic_3d.rs:43`, `hierarchical_3d.rs:78`,
`circular/hierarchy.rs:45`) is a one-line change per layout.
**Expected:** max gap 303 -> ~1e-7 (twopi) and 139 -> ~1e-7 (patchwork).

### 3. `CIRCLE_PACKING` — `algorithm` on the fallback, `arithmetic` once it is seeded
**File:** `crates/graph-core/src/layout/circle_packing/fallback/seed.rs`. **Change:** the fallback
exists and was ported; what it drew its start from was the whole of the remaining gap. SciGraphs
seeds it with `nx.spring_layout`'s own uniform-random positions
(`max(10, min(50, 20000 // n))` iterations, `scale = scale * 0.45`, `seed=get_layout_seed()`,
`circle_packing.py:424-429`) — so `RandomState(981798123).rand(nnodes, 2)`, row-major, `f64`.
**This repair paragraph was stale when it was written** (2026-10-02): it said "port that
fallback", and the fallback was already there. Only the seed was missing.

**Repaired 2026-10-03** (`docs/measurements/sg-spring-seed.md`): `seed_positions` became
`start_positions(n, seed)` — the reference's `RandomState` run at `Some(s)`, the golden-angle
spiral unchanged at `None` — and `CirclePackingParams` grew a `seed: Option<u32>` whose default
is `None`, so the registered `layout.packing.circle` keeps every hashed byte it had and only the
conformance arm opts in.
**Measured:** `lesmis` disparity **0.517 -> 0.0895**, row max gap 3.17 -> 2.70. **Which
fixtures this can move at all was measured, not assumed**: the fallback is taken exactly when
`_planar_triangulation` returns `None` (`circle_packing.py:307-311`), and `networkx.check_planarity`
over the 24 emitted fixtures finds **2 of 24 non-planar** — `lesmis` (77 nodes, 254 edges) and
`bipartite` (14 nodes, 48 edges). So the 20 gate models and the two planar fixtures
(`tree-balanced` is a 15-node tree, `dag-diamond` has 4 nodes and 4 edges) never read the seed,
which is why the row's `f32` and `f64` totals did not move at all. `tree-balanced`'s own 0.418 is
the **exact** path's residual and predates this repair.

**Still not exact on the two that reach it, and the cause is now arithmetic.** The fallback's own
reduction differs from the reference's in three named ways — `seed.rs` fuses nothing where
`layout.py:703-705` forms one factor per pair, measures distance with `libm::hypot` where
`np.linalg.norm` is `sqrt(x*x + y*y)`, and scales by `d*t/len` where numpy computes `d*(t/len)`.
`lesmis` at 0.0895 is the size of that: 0.517 was the seed, 0.0895 is the arithmetic. The row
keeps `shape`/`algorithm` because `sc_propose.py` reads the Procrustes **worst** (0.8629 on
`bipartite`) and a shape a similarity does not explain is `algorithm` by that rule — which is at
least the right neighbourhood, and unlike `SPRING`'s it is not claiming a repair that would move
nothing.

### 4. `SPRING`, `SPRING_3D` — `rng`, and the parameter was the whole repair — **done, `sg-spring-seed`**
**File:** `crates/graph-core/src/layout/force/spring.rs`. **Change:** `SpringParams` grew
`seed: Option<u32>`. At `Some(s)` the start is `np.random.RandomState(s).rand(n, D)`, row-major,
bit for bit; at `None` it is this crate's own `Mulberry32` at `0x5EED`, byte for byte what it was,
so both spring ids keep every hashed snapshot and every hash-gate record. The conformance arm
passes `Some(LAYOUT_SEED)`. The generator, not the seed, was the cause: networkx turns an `int`
seed into `RandomState(seed)` (`utils/misc.py:290-291`), and its `random_sample` is two `u32`
words per double, which is [`Mt19937`](../../crates/graph-core/src/rng.rs).

**Measured** (`docs/measurements/sg-spring-seed.md`): `SPRING_3D` `f32` **4/1020 -> 1020/1020**,
`f64` 3 -> 24, max gap 10 -> 2.36e-07, Procrustes median 0.198 -> 5.03e-16, cause
**`rng` -> `arithmetic`**, tier `bitwise` -> `tolerance`. `SPRING` `f32` **341/1020 -> 866/1020**,
`f64` 341 -> 362, max gap 10 -> 2.37e-03, Procrustes median 0.377 -> 3.77e-16.

**Why `SPRING` is 866 and not 1020, and why its recorded cause is wrong.** 153 of the 154
coordinates that are not `f32`-identical are on `lesmis` (the 24th is one coordinate of `gate-16`);
the other 21 fixtures are exact to the last bit. The residual is one reduction, measured in
isolation on the smallest graph that has it (`gate-00`, two nodes, one edge, 50 iterations, no
chaos): the reference forms **one factor per pair**, `k*k/d**2 - A*d/k`, and sums `delta*factor`
once over `j` (`layout.py:703-705`), while `forces.rs:118-139` sums repulsion over every `j` and
then subtracts attraction over `i`'s own CSR row — the same sum, a different rounding, and the
price of not materialising an `n x n` matrix. Both variants on the same start differ by
**1.78e-15** after the rescale. Fifty chaotic iterations on a 77-node 2D layout turn that into
2.37e-03; the identical kernel at `D = 3` turns it into 2.36e-07, which is why `SPRING_3D`
reaches 1020/1020 and `SPRING` does not.

`sc_propose.py` calls `arithmetic` only when the gap is also `<= 1e-6` (`ARITHMETIC_GAP`), so a
row whose 2D chaos exceeds it falls through to `convention` — which is **not** what is wrong here:
no scale, centre or axis order accounts for 2.37e-03, and undoing one would move nothing. The row
is pinned as the classifier computed it rather than hand-edited, and the discrepancy is recorded
here and in `docs/measurements/sg-spring-seed.md` rather than papered over. Closing it needs either
the fused reduction (a change to the kernel of two registered layouts, their goldens, the
1000-seed differential and a `--past-ceiling` benchmark) or a classifier rule that treats a
fixture-counted residual as arithmetic.

**One defect repaired 2026-10-02, and it was not this one**
(`docs/measurements/sg-fix-spring-temp.md`): the opening temperature read the widest of all
`D` columns, where networkx reads `pos.T[0]` and `pos.T[1]` and nothing else at every `dim`
(`layout.py:687` dense, `:776` sparse) — so a z-dominant `dim = 3` start opened up to 19.95x
too hot, measured against networkx's own `t`. `SPRING_3D`'s motor bytes moved and its first
sha was re-pinned; its disparity was **unchanged** at the time (3/1020, max gap 10), because the
motor's near-isotropic start made the two rules differ by at most 2.01% on any real fixture. The
seed was the gap that was actually open, and it is closed above.

### 5. `RANDOM` — `rng`, the smallest possible port — **done, `sg-mt19937`**
**File:** `crates/graph-core/src/layout/random.rs`. **Landed:** `random::run_seeded(topology, seed)`
draws `np.random.RandomState(seed).rand(n, 3) * SCALE` row-major through `basic_3d::in_space`,
and the conformance arm calls it with `LAYOUT_SEED` the way it calls `sfdp::run_seeded`. The
registered `run` keeps its own `Mulberry32` stream at `0x5EED`, 2D and unscaled, so its hash-gate
record does not move; the generator is [`Mt19937`](../../crates/graph-core/src/rng.rs), numpy's
legacy `RandomState` (`init_genrand` plus the 53-bit `random_sample`).
**Measured:** `f32` 0/1020 -> **1020/1020**, disparity 0.9 -> 2.3e-15. `f64` stays 0/1020 and is
**not** a miss: the motor's columns are `f32`, so `f64` byte equality is unreachable for a value
the reference writes in `f64`. The row therefore ends at `tolerance`/`arithmetic`, which is the
`tier` column's own meaning, not a partial repair. See `docs/measurements/sg-mt19937.md`.

### 6. `CUBE` — `rng`, corners already correct — **done, `sg-mt19937`**
**File:** `crates/graph-core/src/layout/basic_3d/cube.rs`. **Landed:** the interior draws from
`Mt19937::new(981_798_123)` — `derive_seed(42, "layout")` is the reference's own seed — three
words per node in `x, y, z` order, each `(-1.0 + 2.0 * u) * (SCALE * 0.8)`, the reference's operand
order. This changed the **registered** layout's bytes on purpose: the reference is SciGraphs, so
the motor's drawing is now SciGraphs' drawing. The module doc also carried a wrong sentence — that
only the first call after a reset is reproducible — which `dispatcher.py:22`'s `_reset_layout_rng()`
on entry of `apply_graph_layout` refutes. What makes the row pass is the generator and the seed,
not the operand order: at `reach = 4.0` the three algebraically equal forms are bit-identical
(2e6 draws, 0 mismatches).
**Measured:** `f32` 501/1020 -> **1020/1020**, disparity 0.202 -> 5.7e-17. The remaining 501/1020
`f64` are the `3 * min(n, 8)` corner coordinates of **all 24** fixtures — 24 each wherever `n >= 8`,
and `±5.0` / `0.0` are `f32`-representable, so those are exact in both widths. Not "the small
fixtures": only 8 fixtures have `n <= 8` and they hold 117 of the 501. See
`docs/measurements/sg-mt19937.md`.

### 7. `GRAPHVIZ_OSAGE` — `algorithm`, and it is a row assignment
**File:** `crates/graph-core/src/layout/graphviz/osage.rs:163`. **Change:** the y coordinates already
agree to 1e-5 of the span, so the packing **places the same rows**; only the node-to-row
assignment differs (x off by 7%). Compare the engine's row assignment against ours for one
fixture and fix the order in which `osage` claims rows.
**Expected:** `lesmis` disparity 0.902 -> ~1e-16.

### 8. `GRAPHVIZ_SFDP` — `algorithm`, the one a seed cannot fix
**File:** `crates/graph-core/src/layout/graphviz/sfdp.rs:116`. **Change:** both sides are at seed
981798123 and the disparity is 0.848, so the port's *step* differs from the engine's. Diff one
sfdp iteration's force evaluation against Graphviz 16.1.0's `spring_electrical.c`.
**Expected:** the disparity falls; whether it reaches `bitwise` is the open question, which is
why it is not first on this list.

### 9. `SPECTRAL_3D`, `MDS_3D` — `algorithm`, a degenerate eigenvector resolved two ways
**Files:** `crates/graph-core/src/layout/spectral.rs:241`, `pivot_mds.rs:236`. **Change:** both
start from the origin and neither draws, so the difference is which eigenvector sign and scale
each side lands on. Compare one eigenvector's sign convention against networkx's.
**Expected:** `SPECTRAL_3D` 0.333 and `MDS_3D` 0.078 fall; `MDS_3D` is the closest non-matching row
in the matrix and the most likely to close.

### 10. `SPIRAL_3D` — `algorithm`, and **not** `layout.spiral` (landed)
**Files:** new `crates/graph-core/src/layout/basic_3d/spiral.rs`, `registry/three_d/spiral3d.rs`,
`conformance/rows.rs`. **Change:** an earlier version of this paragraph said to raise
`RESOLUTION = 0.35` to 1.0 in `layout/spiral.rs:32`. **Both halves of that were wrong, and
correcting it is most of the repair.** (1) SciGraphs does not call `nx.spiral_layout` at all:
`SPIRAL_3D` dispatches to its own `_spiral_layout_3d` (`basic.py:36-63`,
`layouts/dispatcher.py:105-106`), which is a *conical 3D spiral* — radius `scale*0.5` to
`scale`, `z` from `-scale` to `scale`, spaced evenly along its own arc length. `layout.spiral`
is graph-core's planar Archimedean spiral and SciGraphs has no 2D spiral to compare it to, so
no value of `resolution` could have moved this row. (2) Even the right diagnosis — three
columns where the motor's planar layout has no `z` — does not make this a `resolution` edit:
the new id is a 3D layout of its own, through `basic_3d`'s `in_space`, with `layout.spiral`
left untouched. The port is an arc-length inversion, not a formula: `t = interp(wanted,
length, grid)` over a 65 536-point grid whose `cumsum` is sequential, and numpy's `linspace`
and `interp` each have an arithmetic of their own.
**One correction to the shape of the work, not to the row:** the `max(2, ...)` turn-count floor is
narrower than "for every `n <= 14`" suggests. Measured with numpy 2.3.3, the raw rounded
`sqrt(n/(0.75*pi))` is 1 for `n = 1..5` and is already 2 for `n = 6..14`, so the floor lifts the
value only at `n = 1, 2, 3, 4, 5` and is a no-op from 6 to 14; the first node count whose raw
round is 3 is `n = 15`.
**Measured:** disparity 0.585 -> 3.34e-16 median (5.59e-16 max), `f32` **1020/1020**,
`f64` 120/1020, tier `shape` -> `tolerance`, cause `algorithm` -> `arithmetic`. The judge's own
line for row 14 and the row's `metrics.json` cells are pasted verbatim in
`docs/measurements/sg-spiral3d.md` ("The run those numbers come from"), which is also where the
caveats this paragraph omits live: the cross-language oracle covers sphere, helix and cube only
(the spiral arm arrives in a later job), and `layout.basic3d.spiral` has no
`THREE_D_LAYOUT_STAGES` entry and no negative control yet. `n = 0` is a deliberate divergence
from the reference, unreachable from this matrix.

### 11. `IGRAPH_KK`, `YIFAN_HU`, `GRAPHVIZ_NEATO`, `GRAPHVIZ_FDP`,
`GRAPHVIZ_CIRCO` — `algorithm`
Each is a different method rather than a convention or an RNG, so each needs its own porting job
and none is a one-line change. `GRAPHVIZ_CIRCO` is the one row here that matches on the tree
(6.5e-05) and not on lesmis (0.308), so its repair is whatever makes the equal-box case behave at
lesmis's box sizes.

**`BIPARTITE_3D` was on this list and is not any more, because this list had the picture
backwards.** It read "networkx draws two **columns** and graph-core's `partition` places
differently", and both halves are the wrong way round: networkx's `bipartite_layout` is the
**motor** here (row 18 ran `layout.bipartite`), and the two columns were never the disagreement.
The **reference** is SciGraphs' `_bipartite_layout_3d` (`hierarchical.py:213-242`), which puts
the two node sets on parallel **planes at `z = -+scale*0.5`, one ring each at radius
`scale*0.6`** — so the fix was a layout that draws rings, not a partition that moves. The two
layouts share SciGraphs' node sets (`_bipartite_parts`, or `_greedy_max_cut` where the graph does
not two-colour) and differ in every coordinate after it, which is why the new id is
`layout.bipartite_3d` beside the networkx one rather than a change to it. The repair is
`docs/measurements/sg-bipartite3d.md`.

**`SUGIYAMA` was on this list and is no longer.** It is now `tolerance`/`arithmetic` at
`f32 1020/1020`; the repair was the per-axis normalisation plus two stage-one causes the
reference's own functions named (`ArcOrder::NodeIndex`, because `common.py:238` builds an
`nx.Graph`, and the reference's `arcs` being a `set`). `docs/measurements/sg-sugiyama.md` has
the numbers and the per-stage diff.

## Cells that say `not run`, and why

No cell in the matrix is blank. Three kinds say `not run` and each carries its reason:

- **`GRAPHVIZ_DOT`** — there is no `dot` layout among `crates/graph-core/src/registry.rs:108`'s 35,
  so the motor half produced nothing. Its cells are `not run`, its cause is `reference-absent`, and
  the judge passes it only while it still says exactly that **and** both pairs of bytes still
  match. Its shape panels are not drawn: there is nothing to draw beside the reference, and the
  contact sheet says so in a card rather than showing a broken image.
- **`IGRAPH_KK` on `gate-19`** — `apply_graph_layout` returned `False`, having raised
  `IGRAPH_KK produced 9 non-finite coordinate(s)` (`common.py:183`, caught and reported `False` by
  `dispatcher.py:169-174`), so the row compares 957 coordinates rather than 1020 and names the
  fixture it is missing.
- **`GRAPHVIZ_FDP`'s reference sha** — not reproducible run to run, above.

## What this does not measure

- **One libm, one BLAS, one scipy, one Graphviz.** These are numbers from the repo's pinned
  images; `ref/<NAME>.json` records which. A different build moves the last digit of every
  `arithmetic` row's ceiling and nothing else.
- **Twenty of the 24 fixtures have 2 to 21 nodes.** `lesmis` and the tree are the two a reader can
  see, and finding 7 is what a median over the other twenty hides.
- **The 2D arms have no z and SciGraphs always writes three coordinates**
  (`dispatcher.py:149`). That is a real difference on most rows and it is reported as part of the
  coordinates rather than dropped.
- **`GRAPHVIZ_DOT`'s DOT is undirected** while SciGraphs builds `dot` directed
  (`yifan_hu.py:294`). It would matter if there were a motor layout to compare; it is recorded as
  a convention gap because `gv_plain.write_dot` writes one graph kind for all eight engines and a
  second DOT writer in this arm is a second thing that can disagree with it.
- **The igraph family is compared against igraph 0.11.9's** defaults, not against a version this
  project has chosen. Where SciGraphs ignores `apply_graph_layout`'s own `iterations` in favour of
  igraph's (`igraph_layouts.py:461` for DH's `maxiter=10`, `:493` for graphopt's `niter=500`,
  `:423` for LGL's `maxiter=150`) the two arms agree on the budget without either being told,
  which the matrix records as **no** `iterations` gap.

## The gate, and what makes it non-vacuous

Three checks per row, each able to fail alone, in `verdict.rs`:

1. **the motor's bytes**, `sha256(motor/<NAME>.f64)` against the pinned pair, **re-hashed from
   the file** rather than read out of the JSON — every layout is deterministic (D1–D10), so these
   bytes are a constant of this tree, and trusting a digest an arm wrote about its own output
   would only prove the arms agree with themselves;
2. **the reference's bytes**, same, unless the row declares them unreproducible with a reason;
3. **the shape**, the measured Procrustes median against the pinned ceiling.

An empty pin never passes. A row that cannot be measured passes **only** while the baseline says
`reference-absent`, it still says `not run`, and both pairs of bytes still match — the bytes are
checked *before* the "can this be measured" question, which is the ordering that makes
`GRAPHVIZ_DOT` (whose motor file is always the empty one) an actual check rather than a constant.

**The negative control names its row.** `--break` re-emits with
`GM_MUTATE_SCIGRAPHS_CONFORMANCE=SPRING_3D`, flipping bit 29 — the last bit an `f32` keeps — of one
`f64` in that one row's file, and re-runs everything. The script exits 1 **only** after reading
the judge's own log and finding `SPRING_3D` named in it. A bare `expect nonzero` row would have
been satisfied by the script turning a passing gate into exit 1 by hand, which a judge that passed
everything unconditionally would then have greened; with the log check, such a judge leaves the log
clean, the script exits 0, and the control row turns red. Measured: the control was run against a
judge that fails to name the row, and exits 0.

The cost of the exactness is the obvious one: **a repair that legitimately moves a layout updates
the baseline, and that edit is the record of the repair**, exactly as a moved ceiling is the record
in every other differential in this repository.

## Running it

```
scripts/scigraphs-conformance.sh              # exits 0
scripts/scigraphs-conformance.sh --break      # exits 1, after naming SPRING_3D in the judge's log
```

Six steps across three images, so it is a script and not one command:
`emit-conformance-fixtures` -> `--reference` (ge-python-oracle) -> `--graphviz`
(ge-graphviz-oracle) -> `--metrics` (ge-python-oracle) -> `render.py` (gm-chromium) ->
`scigraphs-conformance`. The gate row is `scigraphs-conformance` and its control is
`negctl-scigraphs-conformance`, both in `scripts/orch/rows/develop-full.rows`.
