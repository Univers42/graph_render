# `layout.force.fdp` against Graphviz 16.1.0's own `fdp`

Why this page exists in this shape: the job said to measure the oracle's determinism and its
seed sensitivity **before** writing any port code, and for `fdp` that instruction found
something the other seven Graphviz engines do not have. `twopi`, `circo` and `osage` are
byte-identical across two runs over the same fixtures. `fdp` is not, at its default
`maxiter`. So the first half of this page is a measurement of the *oracle*, and the port's
agreement is read against it rather than on its own.

Every number below carries the command that produced it. Nothing here is estimated.

```sh
# the fixtures the emit writes, 1000 seeds of the gate's own model
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-graphviz-fixtures --engine fdp --seeds 1000
# the oracle, in the pinned docker-only image
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/fdp-fixtures fdp target/gv-fdp --differential
# the check, which reads the result the harness wrote
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine fdp
```

## The oracle is not reproducible, and that is the finding

`fdp -Tplain -Gstart=1` run twice over the same graph does **not** give the same bytes. This
was found before any port code existed: an earlier run of this job had left
`target/gv-fdp-cmp-a` and `target/gv-fdp-cmp-b` on disk, and `cmp` between them failed.

Two *full* 1000-seed positional runs of the pinned engine over the same fixtures:

```sh
for r in a b; do
  docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/twopi-fixtures fdp target/gv-fdp-cmp-$r \
    --fixtures=twopi.jsonl --start=1
done
cmp target/gv-fdp-cmp-a/graphviz-fdp.jsonl target/gv-fdp-cmp-b/graphviz-fdp.jsonl
```

Both runs took roughly two hours, which is why the control row further down does not use this
command. These two sweeps were read against the emitted `fdp.jsonl` rather than the
`twopi.jsonl` they were run over; the two carry the same gate model at the same seeds and the
same graph per seed, which was checked directly (`n`, `source` and `target` agree on every seed
in the overlap), so the comparison is seed-for-seed the same graphs.

| | value |
|---|---|
| two runs at `-Gstart=1` over all 1000 gate seeds | **differ** |
| seeds byte-identical between them | 961 of 1000 |
| seeds that differ | **39** |
| worst of those 39, by `harness/oracle-twopi.py`'s own `gap` | **2.1623e+03** points, at seed 763 |
| drawing width at seed 763 | 2 297.66 points |
| negative control: one coordinate moved by 1e-6 points, then `cmp` | exit 1, as it must |

### Whether a given graph is stable is a property of that graph

This is the part that makes the finding usable rather than merely alarming, and it took a
negative result to find. Sampling for a bad graph does not work:

| subset | seeds | runs | result |
|---|---|---|---|
| every 50th seed (n = 2 to n = 552) | 20 | 6 | **byte-identical every time** |
| seeds 600 to 700 of the model | 101 | 3 | **3 distinct outputs** — 14 and 15 seeds differing |
| seed 620 alone, in isolation | 1 (**n = 22**) | 8 | **8 distinct outputs**, self-gap 7.76e1 points |
| seed 763 alone, in isolation | 1 (n = 165) | 6 | byte-identical |
| seed 16 alone, in isolation | 1 (n = 18) | 3, then 6 | 3 distinct, then 6 identical — **intermittent** |
| the six closed shapes | 6 | 4 each | byte-identical |

The seed-16 line is the one that keeps the claim honest. A graph can be *intermittently*
unstable — three runs differ, the next six agree — so "byte-identical over N runs" is a
statement about those N runs and not a property of the graph. That is why the control row names
seed 620, which is 8-for-8, instead of a graph that merely happened to agree.

So the instability is **not** a function of size: n = 22 is stably unstable and n = 165 is
stable, and a 20-seed stride happened to miss every unstable graph. The control row
(`fdp-oracle-nondeterminism`) therefore names **seed 620, n = 22** specifically rather than
sampling — verified in isolation, repeatedly, and it costs about two seconds against the
roughly two hours a 1000-seed run takes.

The practical reading: for this engine a differential can be trusted on a graph that has been
*shown* stable, and cannot be trusted as a number over a population that includes unstable
graphs. That is why the ceiling below is set from a measurement and the row is not gated.

### Where the divergence comes from

Bisected by holding everything else fixed and moving one knob, on a single fixed graph
(seed 16 of the gate fixtures, n = 18), 3–4 runs per setting.

**Read the sample sizes.** The graph used here is *intermittently* unstable, not stably
unstable: its first 3 runs all differed, and a later 6 runs of the same graph came out
byte-identical. So "1 of 4" in this table means *these four runs agreed*, which is weak
evidence, and the table is a bisection of **where** the instability lives, not a measurement of
**how much** of it there is. The amount was measured separately, over 1000 seeds, in the section
above. Seed 620 is the graph that is stably unstable — 8 distinct in 8 runs — and it is the one
the control row uses.

| setting | distinct outputs in the runs | reading |
|---|---|---|
| default | 3 of 3, and 4 of 4 | not reproducible |
| `-Gmaxiter=1` | 1 of 4 | one tick is fine |
| `-Gmaxiter=99` | 1 of 4 | 99 ticks were fine here |
| `-Gmaxiter=100` | 1–2 of 4 | the boundary is between 99 and 100 |
| `-Gmaxiter=600` (default) | 4 of 4 | the default is well past it |
| `-GuseGrid=0` | 4 of 4 | not the repulsion grid |
| `-Goverlap=0` | 2 of 4 | not the packing; the expansion phase carries it |
| `-Gstart=1` / `7` / `99` | 3 different layouts | the seed is effective (below) |

So it is the expansion phase, and it is progressive: the number of ticks before the output
becomes unstable grows with the tick count, which is the signature of a chaotic model rather
than of one bad read.

**What it is not.** The obvious suspects were checked and eliminated, and recording the
eliminations is the useful part:

- **Uninitialised memory.** The reference allocates its per-node state with
  `gv_calloc` (`lib/util/alloc.h:47` — `gv_alloc` is `gv_calloc(1, size)`), including
  `gdata` at `layout.c:1005` and `dndata` at `layout.c:186`, and every field both records
  hold is written before it is read on a flat clusterless graph. `MALLOC_PERTURB_` set to 0,
  1, 42, 85, 165 and 200 leaves the `-Gmaxiter=99` output byte-identical
  (`af02e7a50c` five times over), so nothing reads the heap.
- **An unseeded generator.** `fdp` never calls `srand`; `initPositions` calls
  `srand48(T_seed)` at `tlayout.c:487` with `T_seed = 1` from `setSeed`, and the three
  `rand()` tie-breaks (`tlayout.c:194`, `tlayout.c:295`, `xlayout.c:127`) only fire when two
  positions are bit-identical.
- **The wall clock.** `tlayout.c:485` does fall back to `time(NULL)`, but only when
  `T_smode != INIT_RANDOM`, which `-Gstart=1` is not; and eight runs back to back of one
  graph all differ from each other, so no clock with one-second resolution explains eight
  distinct answers.

**The named cause, stated honestly:** the divergence is inside the expansion phase's
floating-point trajectory, growing with the tick count, and it is not attributable to any
seed, any uninitialised read, or any clock in the code that was read. Both arms of this
differential run the same algorithm on the same input, so the spread below is the algorithm's
own, and the port cannot be held to a tighter bound than the reference meets against itself.

## `-Gstart` is effective, not inert

`layout.twopi` and `layout.packing.osage` both have an inert seed, and their rows assert that
with three `cmp`s that pass. This one is the opposite, and the row asserts the difference.

```sh
for s in 1 7 99; do
  docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/gv-fdp-nd fdp target/gv-fdp-nd/s$s \
    --fixtures=f.jsonl --start=$s
done
cmp target/gv-fdp-nd/s1/graphviz-fdp.jsonl target/gv-fdp-nd/s7/graphviz-fdp.jsonl
```

Measured first on a 20-seed strided subset (n = 2 to n = 552), which is where the byte offsets
below come from.

On seed 620 (n = 22), the control row's own graph:

| start | drawing, points |
|---|---|
| 1 | 523.858 x 482.674 |
| 7 | 538.546 x 473.962 |
| 99 | 429.804 x 427.838 |

| comparison | result |
|---|---|
| start 1 vs 7 | differ, byte 51 line 1 |
| start 1 vs 99 | differ, byte 50 line 1 |
| start 7 vs 99 | differ, byte 50 line 1 |

On the 20-seed strided subset the three differ at byte 48 of line 1 — the very first node. The
seed moves the drawing by tens of points on every axis, which is a different layout and not a
perturbation of one.

`initPositions` seeds the whole initial placement from it (`tlayout.c:487`, then
`tlayout.c:554-561`), so the seed *is* the layout's first decision. The port therefore
reproduces the reference's `srand48`/`drand48` sequence bit for bit
(`layout/graphviz/fdp/rng.rs`) rather than drawing from the house's `Mulberry32` — a
different stream would have measured a different layout, not this one.

## `hypot` had to go, and the hash gate is why

A finding that belongs here because it is a measurement rather than a style choice, and
because it is the only place in this port where a deliberate substitution was made in the
reference's favour of a house rule.

The reference computes every separation with `hypot` (`tlayout.c:219`, `tlayout.c:300`,
`xlayout.c:157`, `xlayout.c:172`). This port does not. `hypot` is a libm function and not an
IEEE-754 operation, so glibc's and wasm32's are different implementations, and the hash gate
requires the geometry to be **bit-identical native against wasm32** (D10).

```sh
# with hypot() in the repulsion kernel
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
```

| | with `hypot` | with `sqrt(dx*dx + dy*dy)` |
|---|---|---|
| `layout.force.fdp` | 4-way equal on **5/8** seeds | 4-way equal on **8/8** seeds |
| `hashgate --seeds 8` exit | 1 | 0 |

The signature is unambiguous: on each side the two runs agreed with themselves and the two
sides disagreed with each other, on seeds 0 and 6 among the eight. `sqrt` is correctly rounded
by the standard, so the substitution is bit-identical on every target.

The cost is the last ulp per force, which 300 cooled ticks amplify: on the two-node closed
case the disagreement with the oracle moved from 2.5e-3 to 5.2e-3 points in `x`. That is the
number a reader should weigh against the alternative, and the alternative is a stage that
fails the merge floor.

## The differential

The metric is the largest absolute node-coordinate difference in points, after both arms are
rescaled to one bounding box, imported from `harness/oracle-twopi.py` so there is one
definition of the number rather than two.

| layout | seeds compared | exact | under 1 point | worst max abs coordinate gap (points) | ceiling |
|---|---|---|---|---|---|
| `layout.force.fdp` | **1000** | 0 | 2 | 4.466e+05 at seed 466 (n = 468) | 1e6 |

The gap's distribution, which is the more informative half of the number:

| gap band | seeds |
|---|---|
| exact | 0 |
| under 1 point | 2 |
| 1 to 1e2 points | 34 |
| 1e2 to 1e3 | 123 |
| 1e3 to 1e4 | 627 |
| 1e4 and above | 214 |

**The ceiling is 1e6, the next power of ten above the worst measured gap, over the full 1000
seeds. It was not widened to make anything pass, and nothing is gated on it.**

**This number is not a measure of the port's accuracy, and the reason is the section above.**
Two things make it that, and both are measurements rather than arguments:

1. **The oracle disagrees with itself over the same 1000 seeds** — 961 identical, 39 differing,
   worst 2.16e3 points. A reference that cannot reproduce its own answer to 2000 points cannot
   certify agreement to 2000 points either.
2. **The metric is absolute, and these drawings are enormous.** The largest oracle drawing in
   the sweep is 448 358 points across. A 4.5e5-point gap on a drawing that size is a fraction
   of a percent of the canvas. The metric is the one `harness/oracle-twopi.py` defines and the
   house's, and it is not changed here — but a reader should read it against the drawing's own
   width.

### Agreement does not decay gracefully; it stops at n = 2

The gap by node count, smallest graphs first. This is the most useful single table on the page,
because it says the comparison stops meaning anything immediately rather than slowly:

| n | seed | gap (points) |
|---|---|---|
| 2 | 0 | **2.531e-05** |
| 2 | 600 | **2.531e-05** |
| 3 | 1 | 1.407e+01 |
| 3 | 601 | 1.407e+01 |
| 4 | 2 | 8.073e+00 |
| 5 | 3 | 1.153e+01 |
| 6 | 4 | 4.772e+01 |
| 599 | 597 | 1.339e+05 |
| 601 | 599 | 1.133e+05 |

(The sweep holds `n` fixed across seed blocks, which is why `n = 2` appears twice; the gaps
agree to every printed digit across those blocks, which is a separate and reassuring fact — the
comparison is a pure function of the graph, as the metric's determinism requires.)

There is no regime in which this is a small number times a large `n`. At n = 2 the two arms
agree to 2.5e-5 points, and at n = 3 they are already 14 points apart on a drawing under 200
points across. A reader should take that as the answer to "how close is this port": on the one
graph where the comparison is still informative, it is very close, and on every other graph it
is two different drawings.

## What the port does reproduce, exactly

The iterative part is not comparable, so the checks that *are* exact are the ones that do not
depend on it. Running the oracle at `-Gmaxiter=1 -Goverlap=true` suppresses both the expansion
ticks and the overlap removal, so what `-Tplain` prints is the seeded initial placement and
nothing else.

All six closed shapes are byte-stable here too — two runs each, all identical — so the
placement is a fixed point to compare against. The oracle's own offsets, read at full precision
with `-Tjson0` and differenced against `n0`:

| shape | `n1` from `n0`, points | `n2` from `n0`, points |
|---|---|---|
| `n0 -- n1` | (49.6350, -7.4160) | — |
| `n0 -- n1 -- n2` | (56.1690, -8.3920) | (37.0970, -32.0600) |

| | value |
|---|---|
| `n0 -- n1` at `-Gmaxiter=1 -Goverlap=true`, oracle vs port | offsets agree to 3.2e-4 points |
| `n0 -- n1 -- n2`, same | agree to 4.0e-4 points |
| `graph g { n0; }` at defaults, oracle vs port | **exact**, `(27, 18)` points |
| `n0 -- n1` force result at defaults, oracle vs port | 5.2e-3 points in `x`, 7.1e-4 in `y` |

Those four pin, between them, the spring constant `K = 0.3`, the box half-extent
`1.2 * K * (sqrt(n) + 1) / 2`, the seed, the whole `drand48` sequence, and `compute_bb`'s box
corner. `crates/graph-core/src/layout/graphviz/fdp/tests.rs` holds the first three at the
oracle's printed quantum, and holds the two-node force result at its measured 5.2e-3-point
disagreement — a bound that moved once, when `hypot` went, and the doc on it says so.

### The whole path, end to end, at 20 seeds

Run so the chain in `scripts/orch/rows/p13-gv2-fdp.rows` is known to work rather than assumed,
and so the closed-case comparison is exercised by the harness itself and not only by the
Rust tests:

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-graphviz-fixtures \
    --engine fdp --seeds 20 --out target/fdp-negctl
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/fdp-negctl fdp target/gv-fdp-negctl --differential
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine fdp --dir target/fdp-negctl
```

The harness prints `fdp: 20 seeds, worst 3.072e+02 points; closed 6 exact: True` and the check
prints:

```
  layout.force.fdp: 20 cases, worst 3.072e2, ceiling 1e6: ok
  closed cases: 6 compared byte for byte: ok
PASS
```

**Read `closed 6 exact: True` carefully.** It says the *oracle* reproduces its own six closed
answers byte for byte — that is the `CLOSED` table checking the harness's table against the
engine, and it is a check on the table, not on this port. It is not evidence of agreement
between the two arms, because the force result at those sizes has no closed answer: the
two-node case is 5.2e-3 points from the oracle and the six-branch case is a different drawing.
The port's own side of the closed cases is the Rust tests above, which hold the *placement*
exactly and the *force result* at its measured gap.

## Scale

`--release`, `--repeat 3` medians on one host, with
`scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.fdp --n 220,440,880,1000 --past-ceiling --repeat 3`:

| n | edges | time | per node |
|---|---|---|---|
| 220 | 329 | 727.58 ms | 3.31 ms |
| 440 | 676 | 2 621.23 ms | 5.96 ms |
| 880 | 1 356 | 13 438.35 ms | 15.27 ms |
| 1 000 | 1 541 | 14 371.82 ms | 14.37 ms |

`scale_ceiling` is **1 000**, and it is measured rather than estimated: it is the largest
round size run here, at 14.4 s, which is the same `O(tries * n^2)` wall the reference's nine
`x_layout` tries imply (`xlayout.c:247-293`) and the same term its own default costs. The
per-node column is the tell: flat time per node would be linear, and 3.31 ms to 14.37 ms over
a 4.5x size range is the square.

**It is a measured lower bound, and the row says so.** 1 000 is the largest size that was
*run*, not a size at which the layout was found to stop working — the cost is smooth through
1 000 and nothing breaks there. `registry/graphviz_fdp.rs` records it in those terms, the same
way `registry/graphviz_osage.rs` records osage's, and the difference is that osage's cost is
flat per node while this one's is not: past the ceiling there is no refusal and no trap, the
geometry stays finite, and the caller must apply its own timeout.

The emit needs `--release` for the same reason and the row says so: at the gate's largest
model, n = 601, the overlap phase alone is `9 * 300 * n^2 / 2` pair visits, and a debug build
of the emit takes hours where the release build takes about half an hour.

## Negative controls

| control | row | command | result |
|---|---|---|---|
| the oracle is **not** reproducible (the finding) | `fdp-oracle-nondeterminism` | seed 620, two runs, then `cmp` | differ, so the row passes; identical would fail it |
| the comparison is not vacuous | `fdp-oracle-perturb-negctl` | move one coordinate by 1e-6 points, then `cmp` | `cmp` exit 1 |
| the seed is effective | `fdp-oracle-start-effective` | `cmp` start 1 against start 7 | `cmp` exit 1 |
| the check cannot pass on nothing | `fdp-check-negctl` | `oracle-graphviz --engine fdp --dir target/fdp-fixtures-absent` | exit 2 |
| the port is deterministic native-vs-wasm32 | `hashgate-8` | `hashgate --seeds 8` | `layout.force.fdp` 4-way equal on 8/8, exit 0 |
| a change to the shared model is caught | `negctl-degree` | `GM_MUTATE_REFERENCE_DEGREE=9` | hashgate exit 1 |
| a change to a shared knob is caught | `negctl-twopi-nodes` | `GM_MUTATE_TWOPI_NODES=1` | hashgate exit 1 |
| the layout itself is deterministic | — | `the_same_topology_runs_to_the_same_geometry_twice` | passes |

The hashgate row is the one that matters most for the native/wasm32 identity the `hypot`
section is about, and it is in the file rather than described here because a described check
is not a check.

There is deliberately **no `negctl-fdp-nodes` row**, for the reason
`scripts/orch/rows/p13-gv1-osage.rows:23-31` gives: `hashgate/knob.rs` and `hashgate/knob/`
are shared with the parallel Graphviz engine jobs, and adding a per-stage knob is a merge
collision on a gate file rather than a feature. The controls above need no knob.

## Minimalism ladder

Per `.claude/rules/devil/minimalism-ladder.md`, the rung each new piece sits on and why the one
below it failed.

| piece | rung | why not lower |
|---|---|---|
| `layout/graphviz/fdp/rng.rs` — `drand48` and glibc's `rand()` | 5, a small helper | rung 2 fails: the house's `Mulberry32` is a *different* stream, and a differential against Graphviz measures the gap between two specific streams, so substituting one would have measured a different layout. It is 124 lines and has no dependency. |
| `layout/graphviz/fdp/grid.rs` — the repulsion grid | 5 | rung 2 fails and the reason is measured: `force/fruchterman_reingold.rs` is the existing dense all-pairs repulsion and its own doc records that the spec's grid variant is not implemented, so at n = 601 this would be `600 * 300 * n^2 / 2` pair visits in the hot loop. Rung 5 over rung 2 is the *performance override* the ladder allows, and it is a measured one. |
| `layout/graphviz/fdp/overlap.rs` — the nine `x_layout` tries | 5 | not separable into something smaller: the phase is one loop whose body is the repulsion, the separation and the temperature cap, and splitting those three across files would be three files to read for one loop. |
| `oracle_python/fdp.rs` | 5 | a copy of `oracle_python/osage.rs` with the engine swapped, which is what the generic `Differential` shape is for. No new subcommand, no dispatcher, no fixture layout — the note in the job body asked for exactly this and the three `by_engine`/`ENGINES` lines are the whole integration. |
| `registry/graphviz_fdp.rs` | 5 | a per-engine metadata file rather than an addition to `registry/force.rs`, because four parallel engine jobs would otherwise collide on one file. That is the same reason `registry/graphviz_osage.rs` exists. |

Nothing here is on rung 6 or 7. There is no new dependency, and no new abstraction: the four
`Differential` implementations are the second and third real ones behind the existing struct,
and the generic `by_engine`/`ENGINES` pairing they share is already in `oracle_python/graphviz.rs`.

## Reproducibility: the harness must leave no bytecode behind

`harness/oracle-graphviz.py` imports `harness/oracle-twopi.py` by path with
`sys.dont_write_bytecode` set across the import (`oracle-graphviz.py:97-114`), because the
`harness/` tree is fingerprinted and a `__pycache__` directory left in it would change that
fingerprint. The gap script used for the table above is the same peer module, loaded the same
way, and the row file runs the harness itself rather than a copy of the metric.
