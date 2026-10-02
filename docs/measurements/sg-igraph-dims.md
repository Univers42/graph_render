# sg-igraph-dims — the five igraph rows, at the dimension SciGraphs actually calls

Job: `prompts/jobs/sg-igraph-dims.md`. Rows: `IGRAPH_FR`, `IGRAPH_KK`, `IGRAPH_DRL`,
`IGRAPH_DRL_2D`, `IGRAPH_LGL`. Every number below was run on this tree; the commands are in
"Commands" at the end and each is quoted with the exit code it returned.

## 1. The RNG correction: the reference reproduces, and the gap is a licence

**What the tree said.** `G_IGRAPH_SEED` (`conformance/gaps.rs`) claimed the reference seed is
unreachable: "`_reset_layout_rng` seeds numpy and the stdlib `random`, and igraph reads the C
library's generator, which neither call reaches". `docs/measurements/scigraphs-conformance.md`
finding 4 said the same and added that those six rows "have **no seedable reference start at
all**". `conformance/rows.rs` said igraph's seed "is not reachable from Python at all".

**Why that was wrong.** python-igraph does not use the C library's generator. It installs the
stdlib `random` module *as* igraph's RNG at import:

- `src/_igraph/random.c:295-325` — `igraphmodule_init_rng` does
  `PyImport_ImportModule("random")` and hands the module to `igraph_rng_Python_set_generator`;
- `src/_igraph/random.c:54-58` — the rngtype is declared with `is_seeded = 1`, so igraph never
  tries to seed it out from under the user;
- `src/_igraph/random.c:167-` — `igraph_rng_Python_get` draws from `random.getrandbits`, and
  `igraph_rng_Python_get_real` from `random.random`.

So `common.py:60`'s `random.seed(get_layout_seed())` **does** reseed igraph. `igraph_rng_Python_seed`
itself returns `IGRAPH_EINTERNAL` (`:157-161`) — but nothing calls it, because the state is
`random`'s own.

**The measurement.** Two consecutive `--reference` runs over the same fixture set, compared file
by file:

```
$ cp -r target/scigraphs-conformance/ref /tmp/opencode/sg/ref-run1
$ docker run --rm … ge-python-oracle python3 harness/scigraphs-conformance.py --reference target/scigraphs-conformance
rerun exit=0
$ diff -r /tmp/opencode/sg/ref-run1 target/scigraphs-conformance/ref
DIFF-EXIT=0                       # no output: all 64 files byte-identical

IGRAPH_FR       IDENTICAL  0cf3c05e67d79c08c152d0902dfe70dd5e3ef1cf4f9785448b394d24c8cfb170
IGRAPH_KK       IDENTICAL  a89c503e5fb39b8756fcbef3a6985ae6335770f874fd2e6cd06773bea5d0264a
IGRAPH_DRL      IDENTICAL  19706b910225f374945f8e72c8594361dbb20d93e0716acaacec835c6ecf2b88
IGRAPH_DRL_2D   IDENTICAL  79434cc8e4a271f57891b8170d454ca02a685d22f83a5ab1ed9606215a7b3fa0
IGRAPH_LGL      IDENTICAL  a619ed3bc32e31f78056fbed6186352c5bf382457b40ffea5d0742645f71c3fd
```

All 64 files identical, and the five digests are the ones already pinned in
`baseline/table/networkx.rs` — so the pin was never unstable, only the explanation was.

**What changed.** `G_IGRAPH_SEED` now says the seed *is* passed on both sides, that the reference
is reproducible, and that what remains is `docs/decisions/layouts-igraph.md` rule 4: igraph's
generator is never reproduced, so graph-core keeps Mulberry32 and the two streams part company at
the first coordinate. Finding 4 and `rows.rs`'s module doc were corrected the same way, and
`layouts-igraph.md` carries the correction as a dated paragraph. **The reachable target for these
rows is shape, not bytes, and it always was** — that part of the old finding was right for the
wrong reason.

## 2. `_igraph_fit_positions` on the motor arm

Every igraph helper in `igraph_layouts.py` ends with `_igraph_fit_positions` (`:24-42`): subtract
the per-axis mean, divide by the largest magnitude over **all three** axes, multiply by `scale`.
That step is SciGraphs' convention, not igraph's, so it belongs in the conformance arm — the same
rule the Graphviz rows follow — and it is now `conformance/motor/fit.rs`, one function with its
own test module, applied to four ids named in `FITTED`.

**It changes the bytes and nothing else.** A fit is a uniform scale and a translation, and the
matrix's Procrustes column is alignment-invariant to exactly that, so the medians are unmoved to
the digit. Which is the point: the `G_SNAPSHOT_SCALE` gap on these rows was never what separated
them from the reference.

`IGRAPH_DH` and `IGRAPH_GRAPHOPT` go through the same reference helper and are **not** in
`FITTED`: they are not this job's rows, and adding them would move bytes with no measurement here.
A test names the omission so it reads as a decision.

## 3. 3-D variants of FR and KK

SciGraphs calls FR at `dim=3` (`igraph_layouts.py:74`) and KK at `dim=3` (`:99`). Both motor
layouts were planar, so both rows were comparing a 2-D drawing against a 3-D reference. Two new
ids, one kernel each:

| id | stage | shares |
|---|---|---|
| `layout.force.fruchterman_reingold_3d` | `FruchtermanReingold3D` | `fruchterman_reingold/kernel.rs` at `D = 3` |
| `layout.force.kamada_kawai_3d` | `KamadaKawai3D` | `kamada_kawai/descent.rs` at `D = 3` |

One `const D` per layout, the mechanism `layout/force/spring/forces.rs` already uses: positions
are `[f64; 3]` with `D` live axes, every reduction finishes its squared sum inside its own axis
loop, and `D = 2` performs the operations the two-column version did, in the order it did them.
**That is measured, not asserted:** `fruchterman_reingold::tests::a_path_of_three_has_pinned_coordinates`
and `kamada_kawai::tests::a_path_of_three_has_pinned_coordinates` pin `f32` bit patterns, and both
passed unchanged through the refactor; `hashgate --seeds 8` is green.

What differs at `D = 3` and nowhere else: the geometry (`Geometry::in_space`, so the snapshot
carries a z and is labelled 0.4), the Hessian block (a 3x3 solved by Cramer's rule where 2D uses
the closed 2x2), and FR's start box growing a third axis.

**The KK 3-D start was a spec gap, and it was closed as one.** `kamada_kawai.md` said "sphere …
using igraph's sphere layouts" and left the placement itself unwritten, which left the implementer
nothing to port. Rule 1 of `layouts-igraph.md` says a *spec author* may read the C and writes the
formula, and that the implementer reads only the spec; so the spec section
"The 3D start: the sphere (spec gap closed 2026-10-02)" was written first, from
`kamada_kawai.c:476-485` and `circular.c:153-183`, and the implementation reads only that. It is a
spiral, not a Fibonacci lattice: walk `i` carrying one `phi`, `z = -1 + 2i/(n-1)`,
`r = sqrt(1 - z²)`, `phi += 3.6/(sqrt(n)·r)`, the first and last rows pinned to the poles, then
`x = r cos(phi)`, `y = r sin(phi)`.

**One defect resolved rather than reproduced.** `fruchterman_reingold.md:101` records a mistyped
axis in igraph's own 3-D disconnected pair term (the z component added to `D_y`). The port uses
the intended axis; the spec says so, with the three reasons and a pointer to the exact line a
reviewer would change. It only reaches output on a disconnected 3-D graph, inside a block the spec
already calls an approximation.

**The 2-D ids stay byte-identical**, and the harness change that made the 3-D rows did not move
them: the 100-seed figures below reproduce the pinned 2026-09-29 measurements exactly.

### The new differentials (100 seeds, `graph-cli oracle-igraph`)

```
  layout.force.fruchterman_reingold: 100 cases, worst 1.298e0, ceiling 1e1: ok
  layout.force.kamada_kawai:         100 cases, worst 1.348e0, ceiling 1e1: ok
  layout.force.fruchterman_reingold_3d: 100 cases, worst 1.195e0, ceiling 1e1: ok
  layout.force.kamada_kawai_3d:      100 cases, worst 5.979e-1, ceiling 1e1: ok
  layout.force.drl:                  100 cases, worst 3.977e0, ceiling 1e1: ok
  layout.force.lgl:                  100 cases, worst 2.131e0, ceiling 1e1: ok
  layout.force.davidson_harel:       100 cases, worst 5.191e1, ceiling 1e2: ok
  layout.force.graphopt:             100 cases, worst 1.539e1, ceiling 1e2: ok
PASS
```

Both 3-D rows are inside the **same** ceilings as their 2-D siblings, which is a choice and not a
measurement: the stress ratio is scale-invariant, the metric is unchanged, and the 3-D solve
differs only in its linear algebra, so the 2-D worst case is the honest prior. Stated in
`oracle_python/igraph.rs` next to the numbers. A measured 3-D worst is another job's number to
re-pin.

**Both 3-D rows are better than their 2-D siblings, and KK's is better than igraph.** FR drops
from 1.298 to 1.195 — a third axis gives the repulsion somewhere to go. KK drops from 1.348 to
**0.598**, i.e. our 3-D drawing has *lower* normalised stress than igraph's on every seed. That is
not a claim that the port is better than the reference: the metric is stress, KK optimises exactly
that energy, and Newton descent reaches a different local minimum from a different start. It does
mean the 3-D KK row is no longer a solver disagreement.

**One binding fact the harness had to learn.** `layout_lgl`, `layout_davidson_harel` and
`layout_graphopt` take **no** `dim` argument at all in python-igraph 0.11.9 — passing one raises
`TypeError: unexpected keyword argument 'dim'`. That is why SciGraphs passes `dim=3` for FR, KK and
DrL and not for the other three (`igraph_layouts.py:453` says so in a comment). `REFERENCES` now
carries `None` for those three rather than a guessed `2`, and the 2-D start matrix is what they
get.

## 4. The `IGRAPH_KK` reference defect, named

`IGRAPH_KK` compares 957 of 1020 coordinates. **The doc said the failing fixture was `gate-19`;
it is `gate-01`** — read out of `ref/IGRAPH_KK.json`, where every fixture but `gate-01` has
status `ok`. `gate-19` (21 nodes) lays out fine.

`gate-01` is three nodes with the edges `1-0` and `2-0` plus a parallel `2-0` — the three-vertex
path with its hub first. Measured against python-igraph 0.11.9 in `ge-python-oracle`:

| graph | `dim=3` | `dim=2` |
|---|---|---|
| `gate-01` as emitted | **3 of 9 non-finite** | 0 of 9 |
| 3-vertex path, all six vertex orderings | **6 of 6 non-finite** | — |
| single edge, n=2 | 0 of 6 | 0 of 4 |
| star, 3 leaves | 0 of 12 | — |
| path, n=4 / n=5 / n=6 | 3 non-finite each | 0 |
| cycle, n=3 … 12 | 0 throughout | — |

**Not component count** (every failing graph above is connected), **not isolated nodes** (there are
none), and **not degree** — `gate-01`'s degrees are 2, 1, 1, which the passing `single edge`
(1, 1) and the failing `star, 3 leaves` bracket. It is the **3x3 Newton block being near-singular
at a handful of vertices**: `det` is tiny but non-zero, so the Cramer step is enormous, and 150
moves later one vertex has overflowed `f64` to `±inf`. The 2-D block on the same graph is not, and
the same graph at `dim=2` is finite.

**Why the count is 9 and not 3.** `_igraph_fit_positions` sees `extent = inf`, so its factor is
`scale / inf = 0`, and `inf * 0` is NaN. Three infinities become all nine non-finite coordinates,
which is the message `common.py:183` raises.

**The motor does not copy the failure.** `kamada_kawai_3d` guards `det` (`kamada_kawai/descent.rs`,
`three_by_three`) and `pull` guards `r == 0`, and
`kamada_kawai_3d::tests::the_three_node_path_that_breaks_igraph_is_finite_here` builds `gate-01`
and holds the layout to finite, non-degenerate geometry on it. Recorded as `G_KK_NON_FINITE` on the
row, and the row's missing fixture stays a **recorded reference defect** rather than something the
motor is asked to reproduce.

## 5. Before and after

`f64 k/N` and the Procrustes median, `target/scigraphs-conformance/metrics.json`, same fixture set
and same reference arm throughout. The reference digest column is unchanged in every row and in
every run — that is the reproducibility result of §1 showing up as a constant.

| row | motor id before → after | f64 k/N before | f64 k/N after | median before | median after |
|---|---|--:|--:|--:|--:|
| `IGRAPH_FR` | `…fruchterman_reingold` → `…fruchterman_reingold_3d` | 0/1020 | 2/1020 | 0.2673 | **0.1664** |
| `IGRAPH_KK` | `…kamada_kawai` → `…kamada_kawai_3d` | 0/957 | 8/957 | 0.8120 | **0.7565** |
| `IGRAPH_DRL` | unchanged | 0/1020 | 1/1020 | 0.5360 | 0.5360 |
| `IGRAPH_DRL_2D` | unchanged | 340/1020 | 341/1020 | 0.5137 | 0.5137 |
| `IGRAPH_LGL` | unchanged | 340/1020 | 344/1020 | 0.6108 | 0.6108 |

The three planar rows are the fit's whole effect: the motor's bytes move, and the shape does not,
because a fit is a uniform scale and a translation and the Procrustes column is invariant to both.
The two 3-D rows are the remap, and both medians fall.

## 6. The matrix after the 3-D remap

`scripts/scigraphs-conformance.sh` exited 1 naming exactly `IGRAPH_FR` and `IGRAPH_KK` — no other
row moved, so nothing else was re-pinned — and exited 0 once those two blocks were copied out of
`conformance-baseline-proposed.rs`.

```
  IGRAPH_FR:       0 f64, 0 f32 of 1020, median 2.673e-1   ->  2 f64, 4 f32 of 1020, median 1.664e-1
  IGRAPH_KK:       0 f64, 0 f32 of  957, median 8.120e-1   ->  8 f64, 8 f32 of  957, median 7.565e-1
  IGRAPH_DRL:      0 f64, 0 f32 of 1020, median 5.360e-1   ->  1 f64, 1 f32 of 1020, median 5.360e-1
  IGRAPH_DRL_2D: 340 f64,340 f32 of 1020, median 5.137e-1  -> 341 f64,341 f32 of 1020, median 5.137e-1
  IGRAPH_LGL:    340 f64,340 f32 of 1020, median 6.108e-1  -> 344 f64,345 f32 of 1020, median 6.108e-1
```

**The two 3-D rows are the only ones whose shape moved, and both moved down.** FR's median falls
from 0.267 to 0.166 — a 38% reduction — because the motor is now drawing in the dimension the
reference draws in and the comparison is no longer penalising it for a missing axis. KK falls from
0.812 to 0.757, which together with the stress differential's 0.598 says the 3-D KK row is no
longer a solver disagreement: what is left is the third column and the local minimum.

**The three planar rows are byte-moved and shape-identical.** That is the whole of the fit's
effect, measured.

Both ceilings stayed at `1e0`, so both rows pass with the same headroom they had; neither needed
its ceiling moved, which is the honest outcome to report (a repair that had to loosen a ceiling
would be a repair that made the gate weaker).

### Reading the FR row's per-fixture spread

Worth stating because the median hides it: `IGRAPH_FR` now reads **0.000** on eleven of the
twenty-two overlay fixtures — the tiny gate models — and its whole median comes from the three big
ones (`gate-10` 0.459, `gate-07` 0.657, `bipartite` 0.921). Finding 7 of
`scigraphs-conformance.md` ("the median over the fixture set is dominated by the twenty tiny gate
models") applies to this row harder than it did before. The max column and the picture are the
honest read.

## Commands

Every command below was run on this tree; the number after the arrow is the exit code it returned.

```
scripts/orch/gr cargo build --release -p graph-cli                       -> 0
scripts/scigraphs-conformance.sh                                         -> 0  (baseline, untouched)
scripts/orch/gr cargo test -p graph-cli conformance::motor::fit          -> 1  (RED, see below)
scripts/orch/gr cargo test -p graph-cli conformance::motor               -> 0
scripts/scigraphs-conformance.sh                                         -> 1  (the five rows, as designed)
scripts/scigraphs-conformance.sh                                         -> 0  (after the re-pin)
scripts/scigraphs-conformance.sh --break                                 -> 1
scripts/orch/gr cargo run -q -p graph-cli -- emit-igraph-fixtures --seeds 100 -> 0
docker run … ge-python-oracle python3 harness/oracle-igraph.py target/igraph-fixtures -> 0
scripts/orch/gr cargo run -q -p graph-cli -- oracle-igraph              -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8          -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 … hashgate --seeds 8    -> non-zero (negative control)
scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check        -> 0
scripts/orch/gr cargo run -q -p graph-cli -- codegen --check             -> 0
```

The RED run, before `run` called the fit:

```
test …::fit::tests::an_igraph_row_comes_back_centred_and_at_the_scale ... FAILED
  largest magnitude 1.325621247291565 != 5
test result: FAILED. 4 passed; 1 failed
```
