# `layout.force.neato` against Graphviz 16.1.0's own `neato`

Measured 2026-10-01 on the p13-gv2 tree, 1000 gate seeds (`n = 2 + seed % 600`, one connected
preferential-attachment graph each, `graph_core::seeded_model` at `REFERENCE_DEGREE`), against the
docker-only oracle image `ge-graphviz-oracle` (Graphviz 16.1.0, pinned by sha256 in
`scripts/orch/fetch-refs.sh`).

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- \
  emit-graphviz-fixtures --engine neato --seeds 1000
docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py --compare neato target/neato-fixtures target/gv-neato
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine neato
```

Neither subcommand takes a directory: both default to the engine's own
(`target/neato-fixtures`), from the same static table the `--engine` parser validates against,
so the two halves of the chain cannot be pointed at two different places. The override flags
still exist, and the negative control below is what uses one.

The chain, run on this tree, in full:

| step | exit | result |
|---|---|---|
| `emit-graphviz-fixtures --engine neato --seeds 1000` | 0 | `target/neato-fixtures/neato.jsonl`, 1000 seeds |
| `oracle-graphviz.py --compare neato …` | 0 | `neato: 1000 seeds, worst 6.732e-02 points` |
| `oracle-graphviz --engine neato` | 0 | `layout.force.neato: 1000 cases, worst 6.732e-2, ceiling 1e-1: ok` · `PASS` |

## `-Gstart` is load-bearing here, and that is the whole difference from `twopi`

`twopi` is closed form and reads no `start`; its `-Gstart` is measurably **inert**. `neato` is not.
`initLayout` reads two `drand48` draws per node into its initial placement
(`lib/neatogen/stress.c:154-155`) and `checkStart` seeds the generator from the `start` attribute
(`lib/neatogen/neatoinit.c:989`), so the drawing is a function of the seed. Measured over the full
1000-fixture sweep, hashing the whole `-Tplain` output per fixture:

| `-Gstart` | combined sha256 over all 1000 fixtures |
|---|---|
| 1 | `c5a0d33ddaf349d794e2e369c6fd7380da055077122620d152e5a95481efc13c` |
| 7 | `456a495103b4454b3c0d573dc985e6005384f3c872117f891d9485a272c9938e` |
| 99 | `96bc53484cbc4c8bb2808a751b95068bc44cb00ccc613514e6e79bdae5f3cea3` |

**All 1000 seeds differ** between 1 and 7, and between 1 and 99 — not a handful, all of them. So
this port reproduces the generator exactly rather than drawing from the same distribution:
[`rng`] is the POSIX 48-bit LCG (`X = 0x5DEECE66D * X + 0xB mod 2^48`, seeded
`X = (seed << 16) | 0x330E`) in `u64` arithmetic, and its sequence is pinned against the
algorithm's own first four draws for seed 1 in `neato/tests.rs`. A port that matched the
*distribution* would have nothing to say about a difference from Graphviz, because the difference
would be a difference of seeds.

## Oracle determinism

| run | result |
|---|---|
| `harness/oracle-graphviz.py target/spectral-fixtures neato target/gv-neato-a` | exit 0, 1000 seeds |
| the same again, into `target/gv-neato-b` | exit 0, 1000 seeds |
| `cmp` of the two `graphviz-neato.jsonl` | **silent**, exit 0 |
| manifest sha256, both runs | `490d44e24b480021fe6e678d9c1b5d10b0e02fb7942f8c44240d58024e02fcb9` |

The determinism check is not vacuous, which is the property a `cmp` on its own does not have: a
1e-6-point perturbation of one coordinate in the first record changes byte 119, and

```sh
cmp target/gv-neato-a/graphviz-neato.jsonl target/gv-negctl/perturbed.jsonl
```

exits 1. So a run that differed from another in the sixth decimal would be caught.

## The differential

The metric is the largest absolute node-coordinate difference **in points**, after both arms are
rescaled onto the same bounding box: per seed, Graphviz's own node-centre bounding box is the target
and both arms are mapped onto it with one uniform scale taken from the larger axis. The comparison
is against Graphviz's output, never against a second native run.

| layout | cases | worst max abs coordinate gap (points) | ceiling |
|---|---|---|---|
| `layout.force.neato` | 1000 | 6.732e-2 | 1e-1 |

**The ceiling is 1e-1 and the gap is the oracle's own printed resolution, not a
disagreement.** `-Tplain` writes five significant digits, so one printed digit at the largest
gate drawing is 0.001 inch = 0.911 points, and the worst gap is **0.074 of that**. The
evidence that the gap *is* that quantum and nothing more:

| | value |
|---|---|
| worst gap over 1000 seeds | 6.732e-2 points, at seeds 967 and 367 (n=369, 735.9 pt across) |
| median gap | 1.610e-2 points |
| 95th percentile | 6.044e-2 points |
| smallest gap | 5.793e-4 points |
| seeds above 1e-2 | 527 of 1000 |
| seeds above 3e-2 | 401 of 1000 |
| seeds above 1e-1 | **0 of 1000** |
| largest gate drawing | 911.2 points = 12.655 inches across, so one printed digit is 0.911 points |
| ratio, worst gap to the largest drawing's own printed quantum | **0.074** |
| correlation of the per-seed gap with the drawing's extent | 0.71 |

The per-seed gap tracks the drawing's extent, and no seed exceeds one printed digit, which is
what a formatter's resolution looks like and what an algorithmic difference does not: an
algorithmic difference would show up as a handful of seeds at O(1) drawing widths. The gap as a
fraction of the drawing is 2.4e-5 at the median and 9.2e-5 at the worst — a hundred-thousandth
of the drawing, which is the resolution of the numbers being compared.

**What it took to get there, and what would have hidden it.** The first implementation of the
inner product computed `f64::from(a) * f64::from(b)`, which reads as a harmless precision
improvement over the reference's `double += float * float`. It is not: in C, `a * b` on two
`float`s is a `float` expression, so the reference **rounds the product to `f32` and widens
only the accumulator**. The two differ by one ulp per term, and over a slowly-converging graph
that is everything:

| seed 44 (n=46, 182 stress passes) | worst gap, points |
|---|---|
| product in `f64`, accumulator in `f64` | 1.714e+02 |
| product in `f32`, accumulator in `f64` (**the reference's arithmetic**) | 4.704e-3 |

The first run's distribution said "an iterative engine, a ceiling of 10 points, 39 seeds above
one point" — all of it true, all of it a precision bug wearing a plausible explanation. The
tell was not the size of the gap but that **two seeds had identical gaps to fourteen
significant figures**: the gate's model repeats every 600 seeds, and seeds 44 and 644 are the
same graph, so a deterministic error is exactly what duplicates. The second tell was that a
*coarser* tolerance made the gap **worse** (seed 546: 4.50 points at `eps=1e-4`, 258 at
`3e-4`, 471 at `1e-3`) — a converged layout does not behave that way, and a drifting one does.
`matrix.rs::dot` now rounds the product and the test beside it asserts the rounding on a value
where the two differ, so this cannot be "cleaned up" into a regression without a red test.

## The closed cases, compared token for token

Five of the six small graphs agree with `neato -Tplain` **character for character** at the
plain format's own five-significant-digit precision. Unlike `twopi`, these answers are **not
derived by hand from the reference's source** — they are not derivable, because the engine is
iterative and the drawing depends on the seed — so each row below is two independent runs
compared token by token: ours from the port's own arithmetic, Graphviz's from the oracle.

| case | nodes | stress passes | printed tokens that differ |
|---|---|---|---|
| one node | 1 | 0 (returns before the iteration, `neatoinit.c:1290`) | 0 of 2 — both arms at the origin |
| two nodes | 2 | 2 | **0 of 4** |
| 3-path | 3 | 132 | **0 of 6** |
| 4-cycle | 4 | 9 | 1 of 8 (one node's y, fifth digit) |
| 5-star | 5 | 30 | **0 of 10** |
| 6-branch | 6 | 121 | 10 of 12 — differs at the **fourth** significant digit |

The two that are not token-exact are not a missing feature and not a wrong pass, and the pass
counts are the evidence: the 6-branch is the case that runs the most passes on the fewest
nodes, and it is the one that agrees to four digits instead of five. The 3-path runs **more**
than twice as many passes and agrees exactly, so the count is not a sufficient explanation
either — what separates them is how close to the optimum the two arms stop, and that is the
engine's tolerance, not a defect in either arm.

This is why the ceiling is a tolerance and not a claim of identity, and why the ledger row is
`Status::Implemented`: five cases exact, one within a digit, and a 6.73e-2-point worst case
that is 0.074 of the format's own resolution.

## Scale

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.neato \
  --n 220,1000,2000,4000,10000 --past-ceiling --repeat 3
```

| n | edges | time | per node | stress-1 |
|---|---|---|---|---|
| 220 | 329 | 112.02 ms | 509 us | 0.3446 |
| 1 000 | 1 541 | 2 646.89 ms | 2 647 us | 0.3614 |
| 2 000 | 3 075 | 7 487.97 ms | 3 744 us | 0.3715 |
| 4 000 | 6 193 | 35 538.29 ms | 8 885 us | 0.3746 |
| 10 000 | 15 474 | 251 237.79 ms | 25 124 us | 0.3821 |

`--release`, `--repeat 3` medians, one host.

**The cost is `O(k n^2)` and `k` is what makes it worse than quadratic.** The per-node time
rises 49x from 220 to 10 000 nodes while `n` rises 45x, and the *stress-1* it reaches is flat
(0.345 to 0.382) — the layout is getting the same answer quality, it is just taking longer to
get there. The extra factor is the number of stress passes, which grows with `n`: the reference
converges in 2 passes on two nodes, 9 on a 4-cycle, 30 on a 5-star, 132 on a 3-path, 182 on
seed 44's 46-node graph, and takes the full 200-pass budget on the larger gate models. A graph
that has not converged does not stop early, so the `maxiter` ceiling is a **real** cost at
scale, unlike `twopi`'s where the ceiling is unreachable.

`NEATO_CEILING` is 10 000: the largest size measured here, and where the `O(n^2)` packed
triangle (5e7 entries, 200 MB at 4 bytes each) plus 200 passes of work over it is already
**251 s**. It is a measured lower bound on where it was run, not a measured failure — the
extrapolated cost at 100 000 nodes is around 7.0 hours (a 10x larger `n` against a 100x
`n^2` term) and 20 GB of triangle, which is why the ceiling sits
two orders of magnitude below `FORCE_CEILING = 100 000`, and the binding constraint is the
allocation rather than the arithmetic, which is why `degradation` names the packed triangle
rather than a timeout. This is two orders of magnitude below
`FORCE_CEILING = 100 000` and the `complexity` field says why: `O(n + m)` space for the
networkx-ported force layouts against `O(n^2)` for this one.

## Negative controls

Every row of `scripts/orch/rows/p13-gv2-neato.rows` was run on this tree, and every one
returned what it claims. The full set, with real exit codes:

| control | expected | measured |
|---|---|---|
| `negctl-neato-epsilon` — this stage's own control | nonzero | **1** |
| `negctl-twopi-nodes` | nonzero | **1** |
| `negctl-reference-degree` | nonzero | **1** |
| `negctl-grid-spacing` | nonzero | **1** |
| `negctl-sugiyama-layer-spacing` | nonzero | **1** |
| `negctl-node-count` | nonzero | **1** |
| `negctl-force-theta` | nonzero | **1** |
| `negctl-fa2-scaling-ratio` | nonzero | **1** |
| `negctl-tree-tidy-nodes` | nonzero | **1** |
| `negctl-treemap-nodes` | nonzero | **1** |
| `negctl-circular-nodes` | nonzero | **1** |
| `negctl-packing-scale` | nonzero | **1** |
| `negctl-split-sum` (`--tiers all`) | nonzero | **1** |
| `negctl-split-rescale` (`--tiers all`) | nonzero | **1** |
| `neato-check-negctl` — no recorded run | nonzero | **2**, `no neato-manifest.json` |
| `neato-engine-negctl` — an engine no differential exists for | nonzero | **2**, `invalid value 'nosuchengine' … [possible values: neato, twopi]` |
| `fmt`, `clippy`, `test`, `wasm32-core`, `wasm32-motor`, `hashgate-8` | 0 | **0** each |

`GM_MUTATE_NEATO_EPSILON=1e-2` diverges `layout.force.neato` and **no other stage** — the
honest run on the same command line reports `4-way equal on 8/8 seeds`, so the knob is what
turned it red.

`GM_MUTATE_NEATO_EPSILON` is the gate's **first parameter control that perturbs a parameter rather
than re-drawing a model's size**, and that is what makes it a sharper probe than its neighbours:
`Epsilon` is a tolerance on convergence (`stress.h:25`), so `1e-2` stops the iteration sooner and
moves this stage's bytes with the graph untouched. The re-drawn-model controls would also move any
stage whose output happens to depend on the node count; this one reaches `layout.force.neato` and
nothing else by construction. `hashgate/tests/knob/neato.rs` holds that, and holds the negative
control beside it — a tolerance of `1e-4`, the layout's own `EPSILON`, moves **nothing**, so a
control whose perturbation never reached the layout cannot pass by moving no stages.

`negctl-split-sum` and `negctl-split-rescale` carry `--tiers all` for the reason
`scripts/orch/rows/p13-gv1.rows` gives: without it the two knobs reach a stage the default tier set
never runs, so both rows stayed green with the knob set — a negative control that cannot fail.

## Ponytail (the neato differential)

**Ponytail: a disconnected graph.** The reference gives a node it never reached the distance
`closestDist + 10`, where `closestDist` is the distance of the **last node its breadth-first queue
emptied on**. With more than one component, which component drains last is decided by the order the
queue happened to interleave them, so the "ten past the end" distance is a property of the search
order and not of the graph. **Failing input:** any graph with a node no edge reaches.
**Direction:** the unreachable node lands a different, still finite, distance off its component —
cosmetic, and every reachable node is still placed by exactly the same arithmetic. **Escape hatch:**
connect the graph; the differential's fixtures are connected by construction, so the two arms are
only ever compared where they agree.

**Ponytail: the iteration is stopped, not finished.** `Epsilon` bounds a *relative change in the
stress* and the conjugate gradient's own `1e-3` bounds a residual, so both arms are within a
tolerance of the stress optimum and not at it. **Failing input:** any graph, at the far end of
its convergence. **Direction:** spacing ratios are off by the tolerance rather than exact — a
3-path's ends measure 1.9830 where the hop counts say 2.0, and its two adjacent pairs agree to
2e-4; the 6-branch is the closed case that shows it, agreeing to four significant digits where
the other five agree to five. **Escape hatch:** the differential, which compares this port with
`neato -Tplain` itself and reports the gap the stopping rule leaves.

**Ponytail: the reference's `float`/`double` split is part of the algorithm, and a "clean-up"
that widens it is a regression.** In C, `a * b` on two `float`s is a `float` expression, so
`vectors_inner_productf` rounds every product to single precision and widens only the
accumulator. **Failing input:** any graph that takes more than a few dozen stress passes, which
is most gate fixtures — seed 44 takes 182. **Direction:** the drawing is *nearly* right and
nowhere near the tolerance: 1.7e+02 points instead of 4.7e-3, 39 seeds over one point instead
of none, and a plausible-looking distribution rather than an obviously broken one. **Escape
hatch:** the assertion in `matrix.rs`'s own test, on a value where the two products differ, plus
the two tells recorded above — repeated seeds giving identical gaps, and a coarser tolerance
making the gap *worse*.

**Ponytail: rotation, not translation.** The stress is invariant under any similarity, and the
initial placement is random, so this layout has **no canonical orientation** — two callers laying
out the same graph get drawings related by a *rotation*, not by a translation. The differential's
bounding-box rescale removes the translation but **not** the rotation, which is exactly why its
metric is a gap on a shared box and not a set-to-set distance, and why a port that got the shape
right but the orientation wrong would show a large gap rather than none. **Failing input:** any
graph compared at two seeds. **Direction:** a drawing is rotated, not merely moved. **Escape hatch:**
the seed, which both arms take from `-Gstart=1`.

**Ponytail: `-Tplain` resolution.** The plain format prints five significant digits, so at the
largest gate drawing one printed digit is 0.911 points. **Failing input:** any comparison against
this oracle at all. **Direction:** the gap cannot be tighter than that quantum, so a ceiling
below it would be a claim about Graphviz's formatter rather than about the layout. Here the
quantum *is* the gap — 0.074 of it, with no seed above it — so this ceiling measures the
format, and the escape hatch is the one that gets below it: the small cases, compared token for
token at the same precision, where the drawing is small enough that the quantum is finer.
`docs/measurements/p13-gv1.md` makes the same argument for `twopi`, from the same measurement.
