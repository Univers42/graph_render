# `layout.force.sfdp` against Graphviz 16.1.0's own `sfdp`

Measured 2026-10-01 on the p13-gv2 tree, 1000 gate seeds (`n = 2 + seed % 600`, one connected
preferential-attachment graph each, `graph_core::seeded_model` at `REFERENCE_DEGREE`), against
the docker-only oracle image `ge-graphviz-oracle` (Graphviz 16.1.0, pinned by sha256 in
`scripts/orch/fetch-refs.sh`).

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine sfdp --seeds 1000
docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/sfdp-fixtures sfdp target/gv-sfdp --differential
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine sfdp
```

## The one thing to read first

**This engine is seed-sensitive, and its own seed-to-seed spread is larger than the gap between
our arm and its.** The oracle compared *against itself* at `-Gstart` 7 rather than 1 — same
binary, same image, same 1000 fixtures, only the seed changed — disagrees by **4.81e+02 points**
on the differential's own metric, while our arm's worst gap is **3.887e+02**. Our drawing is
therefore *closer* to Graphviz's than Graphviz's own drawing at a different seed.

| comparison | worst max abs coordinate gap (points), 1000 seeds |
|---|---|
| our arm vs Graphviz at `-Gstart=1`, 2026-10-04 (`sg-sfdp-collapse`) | 3.887e+02 |
| our arm vs Graphviz at `-Gstart=1`, 2026-10-01 (this table's first measurement) | 3.881e+02 |
| **Graphviz vs itself, `-Gstart=1` vs `-Gstart=7`** | **4.809e+02** |
| Graphviz vs itself, `-Gstart=1` vs `-Gstart=99` | 4.506e+02 |

That is not an excuse for a wide tolerance; it is the measurement that says **what agreement is
achievable**. The named cause is in the module's `Ponytail` note: the reference draws a random
permutation to order its multilevel matchings and re-`srand`s between levels
(`lib/sfdpgen/Multilevel.c`, via `gv_permutation`), so its layout is a function of a random
permutation stream this port does not reproduce. The *solver* is reproduced — glibc's `rand()` to
the bit (below), the cooled step, the absolute stop test, the exact all-pairs repulsion below 45
nodes, and `prolongate` with its `interpolate_coord` pass. What is not reproduced is the
**multilevel driver**, and there is a second, larger reason the gap cannot close:

> **At its defaults `sfdp` never coarsens at all.** `sfdpinit.c:213` reads the `levels` graph
> attribute with default `0`; `spring_electrical.c:56` comments that value *"if <=1, single
> level"*; and `Multilevel_establish` returns at `grid->level >= ctrl.maxlevel - 1`
> (`Multilevel.c:163`), which is `0 >= -1`. So the engine runs `spring_electrical_embedding` once
> on the whole graph: no coarsening, no `prolongate`, no `K` decay, no `adaptive_cooling = false`.
> This port always coarsens, down to `COARSEST_FLOOR = 8`.
> `docs/measurements/sg-sfdp-collapse.md` carries the measured cost; `sg-sfdp-step` owns it.

## The blind spot this ceiling cannot see

**The 1e3 ceiling is a coordinate ceiling, and the failure that actually happened was not a
coordinate failure.** On 2026-10-04 (`sg-sfdp-collapse`) the port's `lesmis` layout was found to
be a **one-dimensional strip**: shape ratio 0.006 against Graphviz's 0.414, with 40 of the 77
output points lying within `1e-6` of the drawing's own extent. After the repair the ratio is 0.271
and the coincident pairs are 0 — and over the same 1000 differential seeds the worst coordinate
gap moved from 3.881e+02 to 3.887e+02, i.e. **not at all**.

Two consequences, both about what a ceiling is evidence *for*:

1. **This differential cannot detect a collapsed layout.** A drawing squeezed onto a line is
   *closer* to a reference that is also narrow, and a strip running the wrong way is a large gap
   on an axis the metric does not weigh separately. The ceiling says the coordinates are within
   1e3 points; it says nothing about whether the drawing is a layout.
2. **No ceiling can be tightened on this row from this metric.** 3.887e+02 is 0.6% above the
   previous 3.881e+02, and both are below Graphviz's own 4.809e+02 seed-to-seed spread, so a
   smaller number would be a claim about luck rather than about agreement.

The check that *does* see it is a property test, not a distance test, and that is where this
ceiling's scope has to stop:

| property | where | what it would have caught |
|---|---|---|
| shape ratio > 0.15 on the gallery graph and on a 10x10 lattice | `sfdp/contract.rs::{the_layout_spreads_on_lesmis, the_layout_spreads_on_a_ten_by_ten_grid}` | the strip |
| no two output points within `1e-6` of the drawing's extent | `sfdp/shape.rs::assert_spread` | the 40 coincident pairs |
| the reference's own arithmetic, to 1e-12, on a level small enough to sum by hand | `sfdp/contract.rs`, all six tests | defects a-e one at a time |

Any future narrowing of this ceiling needs one of those to come with it.

## The differential

The metric is the largest absolute node-coordinate difference **in points**, after both arms are
rescaled onto the same bounding box, with one uniform scale (the largest axis's span), imported
from `harness/oracle-twopi.py` rather than copied. The comparison is against Graphviz's output,
never against a second native run.

| layout | cases | worst max abs coordinate gap (points) | ceiling |
|---|---|---|---|
| `layout.force.sfdp` | 1000 | 3.887e+02 | 1e3 |

The ceiling is 1e3, the next power of ten above the measured worst gap. It was **not** widened to
make the row pass, and it was not narrowed to hide anything: the measurement above is the reason
no smaller number is meaningful. It was also **not** narrowed on 2026-10-04, when the repair moved
the worst case from 3.881e+02 to 3.887e+02 — a change of 0.6%, which supports no ceiling at all.

## Determinism, measured before anything else

The oracle run twice over the same 1000 fixtures, `cmp`'d:

| run | sha256 of `graphviz-sfdp.jsonl` |
|---|---|
| a | `09d4efb80d5c08a83964656792a92ec7ff88dae1d39d033a02b008033e75ae98` |
| b | `09d4efb80d5c08a83964656792a92ec7ff88dae1d39d033a02b008033e75ae98` |

`cmp` is silent. This is a real check and not a vacuous one: the `sfdp-oracle-perturb-negctl` row
in `scripts/orch/rows/p13-gv2-sfdp.rows` moves one node coordinate of one seed by **1e-6 points**
— four orders of magnitude below anything the layout computes — and that `cmp` then fails. So
the determinism claim is not "the comparison never looks".

Our own arm is bit-identical native vs wasm32: `hashgate --seeds 8` reports
`layout.force.sfdp: 4-way equal on 8/8 seeds`, which is four arms (two native, two wasm32).

## `-Gstart` is NOT inert here — measured, not assumed

This is the opposite of every other Graphviz engine row, which asserts the seed makes no
difference. The same 1000 fixtures at three seeds:

| `-Gstart` | sha256 of `graphviz-sfdp.jsonl` |
|---|---|
| 1 | `09d4efb80d5c08a83964656792a92ec7ff88dae1d39d033a02b008033e75ae98` |
| 7 | `8c88e2f0d03c6d7a47d92955c6eb71bf7cd91eb8b9dcac9004c392bc9af92142` |
| 99 | `09e058ef5ea1e9f2b1977052ac8fd518c682d40ec5e7aba7fe6479e8386fbe8d` |

Three distinct files. The row `sfdp-oracle-start-sensitive` in the rows file therefore asserts
that `cmp` **fails**, which is the correct direction for this engine and the opposite of the
osage row's: a `cmp` that succeeded would mean the seed had stopped reaching the layout, which
is the failure mode this port is most exposed to.

## The closed cases

Exactly one of the six small cases in `harness/oracle-twopi.py`'s `CLOSED_CASES` is closed for
this engine, and it is compared byte for byte:

| case | our arm | Graphviz's printed line | exact |
|---|---|---|---|
| one node | `(27.0, 18.0)` points | `node n0 0.375 0.25 0.75 0.5` | yes |

`0.375 x 0.25` inch is the centre of the default `0.75 x 0.5` inch node box, and it is the same
at `-Gstart` 1, 7 and 99: a graph with one node has one position, so there is nothing left to be
random about.

The other five — two nodes, a 3-path, a 4-cycle, a 5-star and a 6-branch — are **not** closed
answers for this engine, and listing them as such would turn a failing row green for the wrong
reason. Measured, each prints three *different* answers at the three seeds:

| case | `-Gstart=1` | `-Gstart=7` | `-Gstart=99` |
|---|---|---|---|
| two nodes | `0.375, 0.25` `0.76401, 0.25` | `0.375, 0.25` `0.88417, 0.25` | `0.375, 0.25` `1.2302, 0.25` |
| 3-path | `1.4682, 0.25` `0.92427, 0.25` `0.375, 0.25` | `1.9776, 0.25` `1.1786, 0.25` `0.375, 0.25` | `2.3032, 0.25` `1.3361, 0.25` `0.375, 0.25` |

The seeded random start is the layout's only source of symmetry breaking, so every multi-node
case moves. They are covered by the measured gaps above instead.

## What *is* reproduced exactly: glibc's `rand()`

The random start is `srand(seed)` then `drand()` per coordinate
(`lib/sfdpgen/spring_electrical.c:282-284`), and `drand()` is `rand()/(double)RAND_MAX`
(`lib/sparse/general.c:25-27`) — the divisor is `RAND_MAX` = 2^31-1, **not** 2^31.
`crates/graph-core/src/layout/graphviz/sfdp/start.rs` implements glibc's TYPE_3 additive-feedback
generator and pins it against the system libc, not against a restatement of its algorithm:

| seed | first eight `rand()` outputs, glibc and this port |
|---|---|
| 1 | `1804289383 846930886 1681692777 1714636915 1957747793 424238335 719885386 1649760492` |
| 2 | `1505335290 1738766719 190686788 260874575 747983061 906156498 1502820864 142559277` |

Two details were each worth a wrong answer on their own, and both are recorded in the source:

- **The pointer advance is uniform.** The published description of TYPE_3 says the recurrence is
  `r[i] = r[i-3] + r[i-31]`, which reads as though the rear pointer skips an extra word when the
  front one wraps. Implemented that way it produces a plausible stream that agrees with glibc on
  *nothing*. Measured against `initstate_r`'s own post-warm-up state table, the uniform advance
  is what reproduces glibc's first eight outputs exactly.
- **The Schrage seeding folds a negative word up by `2^31 - 1`.** Omitting that fold is off by
  one ulp of the *state*, and the whole stream with it.

This is the part of the port that had to be exact and is. It is also not sufficient: the
coarsening permutation is a second, independent random stream that this port does not draw, and
that is the gap above.

## The oracle image cannot run this engine's defaults

`sfdp` calls `remove_overlap` unconditionally (`lib/sfdpgen/spring_electrical.c:1181`) and in an
image built without the triangulation library that function is an **empty stub** which prints one
line and returns (`lib/neatogen/overlap.c:588-610`). The notice sets Graphviz's error flag, so the
process exits 1 while stdout already holds the complete finished drawing.

The coordinates are therefore sfdp's own: overlap removal changed nothing, because it ran no
code. Measured: `-Goverlap` `false`, `true`, `scale`, `prism` and `vor` all produce
byte-identical stdout and the same exit 1, so **no flag value can suppress it** — it is removed
in `harness/oracle-graphviz.py` by an exact-match allowance (`ENGINE_BENIGN_STDERR`), not worked
around. Every other engine keeps the strict rule that a non-zero exit is a failure.

## Scale

`--release`, `--repeat 3` medians on one host, `--repeat 1` at 50 000:

| n | edges | time | per node |
|---|---|---|---|
| 220 | 329 | 66.5 ms | 302 µs |
| 2 000 | 3 075 | 570 ms | 285 µs |
| 10 000 | 15 474 | 3.47 s | 347 µs |
| 50 000 | 77 474 | 23.9 s | 478 µs |

`SFDP_CEILING` is **50 000**: the largest size actually measured, stated as such rather than as a
limit. The growth is roughly `n^1.3`, and the cause is the iteration count rather than the
per-iteration cost — the reference's loop runs to its own convergence test (up to 500 iterations)
and rebuilds the Barnes-Hut tree on every one. Extrapolating that fit to the 1 000 000 nodes the
other engines reach predicts on the order of half an hour for one layout, which is why the
registry says 50 000 and not a number no run here supports.

**Two measurements changed the implementation**, and both are recorded because the first version
of each was wrong:

| hotspot | before | after | how it was found |
|---|---|---|---|
| `multilevel::coarsen` scanned the whole edge list per node | — | — | `bench` at 50 000 nodes took **600 s** |
| `multilevel::coarse_edges` deduplicated with `Vec::contains` per edge | 600 s | 23.9 s | the same run; an `O(m²)` scan |

Rebuilding the adjacency list once and sorting-then-deduping the coarse edges took 50 000 nodes
from 600 s to 23.9 s, a 25x improvement, with no behavioural change to the layout. The dedup sorts
by the canonical `(min, max)` key, so the order it leaves behind is a deterministic function of
the edge list and no hash map is iterated (D2).

## Negative controls

| control | command | expected |
|---|---|---|
| the oracle is read to the last digit | `sfdp-oracle-perturb-negctl` | `nonzero` |
| the check cannot pass on nothing | `sfdp-check-negctl` | `nonzero` |
| the seed reaches the layout | `sfdp-oracle-start-sensitive` | `nonzero` |
| `--engine` cannot name a nonexistent engine | `sfdp-engine-negctl` | `nonzero` |

There is deliberately **no `negctl-sfdp-nodes` row**. The per-stage knob for a Graphviz engine
re-draws that stage's own model (`hashgate/knob/twopi.rs` is the precedent), and
`hashgate/knob.rs` plus `hashgate/knob/` are shared with the parallel Graphviz engine jobs — so
several agents editing one enum is a merge collision on a gate file, not a feature.

## Reproducibility: the harness must leave no bytecode behind

`harness/` is inside the fingerprinted set (`crates/graph-cli/src/fingerprint.rs:21`), and
importing a module by path would write `harness/__pycache__/*.pyc` into it — a transient file
that moves the fingerprint for as long as it exists, so `emit` (fingerprint without the `.pyc`)
and `oracle-graphviz` (fingerprint with it) would disagree on a clean checkout. The path imports
run with `sys.dont_write_bytecode` set, so the bytecode is never written.

## Ponytail (the sfdp differential)

**Failing input**: every graph, in the last digits. **Direction**: the drawing is a different but
equally valid sfdp layout of the same graph, not a wrong one — the coarsening matching is
deterministic here and random in the reference. **Escape hatch**: `sfdp::run_seeded` is the
seam; a port that drew glibc's permutation stream would only have to replace
`multilevel::coarsen` and re-seed per level, and the differential would then measure one number
instead of the gap between two random streams.

The secondary mark: **the two-node case keeps a residual rotation** (measured 0.04 rad from the
start direction). Two nodes sit in Barnes-Hut cells with *different* centres of mass, so their
two forces are only nearly antiparallel rather than exactly so. Failing input: a graph whose
node count makes the quadtree cells lopsided. Direction: the pair's axis drifts slightly;
nothing else moves. Escape hatch: none needed, and none wanted — the drift is the model.