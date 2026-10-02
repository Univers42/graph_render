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
question. Exactly **three** ids get a parameter override, because exactly three have a
registered default that is not SciGraphs' parameter: `CIRCLE_PACKING` (500 sweeps, not 50),
`FORCEATLAS2` (`max_iter` 100, not 50) and `GRAPHVIZ_SFDP` (`run` hard-codes `DEFAULT_SEED = 1`;
the arm calls `run_seeded`).

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
`GRID` has a disparity of 5e-32 and an absolute gap of 4.0: the same lattice, in a different
unit at a different origin. Calling that `arithmetic` because the shape is exact would send a
repair job to chase a summation order that is already right.

**The `convention` threshold is 1e-6 and it is measured, not chosen.** Every row this
classifier calls a convention measures between 5e-32 (`GRID`) and 4e-10 (`GRAPHVIZ_PATCHWORK`);
every row that measures 0.08 to 0.7 does not, and its overlay says the same thing. An earlier
0.35 threshold here called `SPECTRAL_3D`, `MDS_3D`, `GRAPHVIZ_CIRCO` and `CIRCLE_PACKING`
conventions, and all four are a different shape in the picture — the threshold was wrong, not the
data.

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
| 1 | `RANDOM` | `layout.random` | `apply_graph_layout` | `bitwise` | 0/1020 | 0/1020 | 4.62e+18 | 4.83 | 0.9 | 0.996 | `rng` | same size, different shape: two independent uniform draws |
| 2 | `GRID` | `layout.grid` | `apply_graph_layout` | `bitwise` | 342/1020 | 342/1020 | 9.22e+18 | 4 | 5.5e-32 | 9.68e-31 | `convention` | **same shape** — the aligned motor lands on every grey lattice point; only scale and origin differ |
| 3 | `SPRING` | `layout.force.spring` | `apply_graph_layout` | `bitwise` | 341/1020 | 341/1020 | 9.23e+18 | 10 | 0.377 | 0.779 | `rng` | different shape: green does not follow the grey drawing anywhere |
| 4 | `SPRING_3D` | `layout.force.spring3d` | `apply_graph_layout` | `bitwise` | 3/1020 | 4/1020 | 9.23e+18 | 10 | 0.198 | 0.756 | `rng` | different shape: as `SPRING`, in space |
| 5 | `CIRCLE_PACKING` | `layout.packing.circle` | `apply_graph_layout` | `shape` | 344/1020 | 808/1020 | 9.22e+18 | 3.17 | 5.3e-16 | 0.827 | `algorithm` | different on lesmis (0.517) and **bit-for-bit the same packing on the 20 gate models** (5e-16): SciGraphs' non-planar fallback is where the two part company |
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
| 16 | `CUBE` | `layout.basic3d.cube` | `apply_graph_layout` | `bitwise` | 501/1020 | 501/1020 | 9.23e+18 | 7.58 | 0.202 | 0.847 | `rng` | the eight corners land on the grey corners; the interior is redrawn from another generator |
| 17 | `HIERARCHICAL_3D` | `layout.hierarchical3d` | `apply_graph_layout` | `tolerance` | 374/1020 | 1020/1020 | 2.67e+08 | 7.95e-08 | 9.03e-17 | 2.93e-16 | `arithmetic` | **same shape** — green covers grey node for node |
| 18 | `BIPARTITE_3D` | `layout.bipartite` | `apply_graph_layout` | `shape` | 2/1020 | 2/1020 | 9.22e+18 | 4 | 0.405 | 0.437 | `algorithm` | different shape: grey is one ring, green two columns inside it |
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
| 31 | `SUGIYAMA` | `layout.dag.sugiyama` | `apply_graph_layout` | `shape` | 348/1020 | 349/1020 | 9.24e+18 | 49.4 | 0.384 | 0.934 | `algorithm` | different shape: the layering differs, so the columns do not line up |
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

**1. Four rows are `f32`-identical on every one of 1020 coordinates and the rest of their gap is
the narrowing.** `SPHERE`, `HELIX`, `HIERARCHICAL_3D`, `CIRCULAR_HIERARCHY` — 4 of 32. Their max
gaps are 2.4e-7, 1.5e-7, 7.9e-8 and 2.2e-7, one `f32` ULP at that magnitude, and their Procrustes
medians are ~1e-16: the same shape to machine precision. `CIRCULAR_HIERARCHY` is the strongest row
in the matrix.

**2. Three rows are the same shape to `1e-10` or better and differ only in units.** `GRID` (5e-32),
`GRAPHVIZ_TWOPI` (2e-10), `GRAPHVIZ_PATCHWORK` (4e-10). Each is a convention fix, not an
algorithm.

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

### 1. `GRID` — `convention`, one line, a whole row
**File:** `crates/graph-core/src/layout/grid.rs:51`. **Change:** `GridParams::spacing` defaults to
1.0 and the snapshot then centres it; SciGraphs' `_grid_layout(num_nodes, scale)` (`basic.py:16-17`)
sets the pitch to `scale / grid_size` and starts at the origin. Make the pitch a function of
`scale` rather than a fixed 1.0.
**Expected:** disparity stays ~5e-32 and **max gap falls from 4.0 to ~2e-7**, moving `bitwise f64`
from 342/1020 towards 1020/1020.

### 2. `GRAPHVIZ_TWOPI`, `GRAPHVIZ_PATCHWORK` — `convention`, the same class, two files
**Files:** `crates/graph-core/src/layout/radial/twopi.rs:104`,
`crates/graph-core/src/layout/graphviz/patchwork/squarify.rs:41`. **Change:** the motor works in
its own units and the reference arm reports the engine's points in **points**; the layout's own
extent is the missing scale. Normalising to `scale = 5.0` the way the three layouts that already
publish a `SCALE` const do (`basic_3d.rs:43`, `hierarchical_3d.rs:78`,
`circular/hierarchy.rs:45`) is a one-line change per layout.
**Expected:** max gap 303 -> ~1e-7 (twopi) and 139 -> ~1e-7 (patchwork).

### 3. `CIRCLE_PACKING` — `algorithm`, and the gate models are already exact
**File:** `crates/graph-core/src/layout/circle_packing.rs:98`. **Change:** on the 20 gate models the
port is already exact to 5e-16; on `lesmis` it is 0.517. SciGraphs' `_circle_packing_layout`
(`circle_packing.py:281-291`) **falls back to force-directed when the graph is non-planar**, and
`lesmis` is non-planar. Port that fallback's branch and its solver.
**Expected:** `lesmis` disparity 0.517 -> ~1e-16 and `bitwise f32` 808/1020 -> ~1020/1020.

### 4. `SPRING`, `SPRING_3D` — `rng`, and the parameter is the whole repair
**File:** `crates/graph-core/src/layout/force/spring.rs:120` (`SpringParams` has no `seed` field).
**Change:** add `seed: u32` to `SpringParams`, default it to `get_layout_seed()`, and draw the
start from a **numpy MT19937 `RandomState`** rather than the kernel's own Mulberry32 — the
generator is the cause, not the seed. networkx's `spring_layout` takes `seed=` and SciGraphs
passes `get_layout_seed()` (`networkx_layouts.py:18`).
**Expected:** `bitwise f64` 341/1020 -> ~1020/1020; the cause becomes `arithmetic`.

### 5. `RANDOM` — `rng`, the smallest possible port
**File:** `crates/graph-core/src/layout/random.rs:30`. **Change:** `SEED` is the const `0x5EED`; the
reference draws from `np.random.RandomState(get_layout_seed())` (`basic.py:7`). Replace the
generator with MT19937 and take the seed as a parameter.
**Expected:** `bitwise f64` 0/1020 -> 1020/1020. The shortest path from `shape` to `bitwise` in the
matrix and the cheapest one to check.

### 6. `CUBE` — `rng`, corners already correct
**File:** `crates/graph-core/src/layout/basic_3d/cube.rs:78`. **Change:** `SEED = 0x00_C0BE` is
compiled into the cube's interior draw (`cube.rs:129`); the eight corners are a closed form and
stay exact. Move the interior onto MT19937.
**Expected:** `bitwise f32` 501/1020 -> ~1020/1020 with the corners unchanged.

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
**Measured:** disparity 0.585 -> 3.34e-16 median (5.59e-16 max), `f32` **1020/1020**,
`f64` 120/1020, tier `shape` -> `tolerance`, cause `algorithm` -> `arithmetic`. Numbers and
commands in `docs/measurements/sg-spiral3d.md`.

### 11. `BIPARTITE_3D`, `SUGIYAMA`, `IGRAPH_KK`, `YIFAN_HU`, `GRAPHVIZ_NEATO`, `GRAPHVIZ_FDP`,
`GRAPHVIZ_CIRCO` — `algorithm`
Each is a different method rather than a convention or an RNG, so each needs its own porting job
and none is a one-line change. `BIPARTITE_3D` in particular: networkx draws two **columns** and
graph-core's `partition` (`bipartite.rs:30`) places differently. `GRAPHVIZ_CIRCO` is the one row
here that matches on the tree (6.5e-05) and not on lesmis (0.308), so its repair is whatever makes
the equal-box case behave at lesmis's box sizes.

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
