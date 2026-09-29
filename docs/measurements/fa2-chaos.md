# FA2 chaos: why the coordinate differential gates at 2 iterations, not 100

**Host line.** Measured 2026-09-29 in the `ge-python-oracle` image (networkx 3.6 from its
pinned source tarball in `/goinfre/dlesieur/refs/networkx-3.6`, numpy 2.3.3) on the
`fa2fix` worktree `/goinfre/dlesieur/wt/fa2fix`, driven through
`/goinfre/dlesieur/orch/bin/gr` with `CARGO_BUILD_JOBS=4`, the port built `--release`,
x86_64. 600 seeds, which is the whole gate model: `gate_node_count(seed) = 2 + seed % 600`,
so seeds 0..599 enumerate every model the 1000-seed gate ever draws (the gate's seeds
1000.. repeat them).

## What is measured, and the control that makes it a measurement

`harness/fa2-chaos.py` runs the *same* reference function three times per fixture line,
from the port's own initial positions with the port's parameters:

| arm | what it is |
|---|---|
| `ours` | the coordinates `emit-fa2-fixtures` wrote — the port |
| `nx` | networkx 3.6 `forceatlas2_layout` from the fixture's start |
| `nx_ulp` | networkx from that start scaled by `1 + 2**-23`, one float32 ulp |

`nx_ulp` is the control the whole argument rests on. It is the same library, the same code
path, the same summation orders, differing only by a perturbation *smaller than the port's
own f32 snapshot rounding* — the coordinates under comparison are ours after the snapshot
rounds them. So anything `nx_vs_nx` measures is ForceAtlas2 amplifying arithmetic order,
never the port disagreeing with networkx. Both gaps are `max |a - b|` over both
coordinates over the larger of the two extents: scale-free, and the same normalization
`harness/oracle-fa2.py` gates on.

```
gr cargo run -q --release -p graph-cli -- emit-fa2-fixtures --seeds 600 --max-iter <K> --out target/fa2-k<K>
docker run --rm -v "$PWD:/w" -w /w ge-python-oracle python3 harness/fa2-chaos.py target/fa2-k<K> <K>
```

## The table

`ours_vs_nx` is only comparable at the budget the fixture ran, so the 600-seed row for
budget 2 is the only one with both arms filled; the 200-seed sweep below has each budget
measured from its own emit.

200 seeds, gate model 2..201 nodes:

| max_iter | nx_vs_nx median | nx_vs_nx p99 | nx_vs_nx max | ours_vs_nx max | nx worst seed |
|---:|---:|---:|---:|---:|---:|
| 1 | 3.389e-08 | 6.553e-08 | 6.626e-08 | 3.270e-08 | 0 |
| 2 | 3.315e-08 | 1.176e-07 | 1.236e-07 | 3.156e-08 | 167 |
| 3 | 3.308e-08 | 1.284e-06 | 1.488e-06 | 3.050e-08 | 105 |
| 5 | 5.271e-08 | 3.741e-06 | 3.789e-06 | 2.865e-08 | 105 |
| 10 | 1.021e-07 | 5.538e-06 | 6.171e-06 | 3.201e-08 | 180 |
| 20 | 1.450e-07 | 3.533e-05 | 5.720e-05 | 3.201e-08 | 88 |
| 40 | 4.376e-04 | 2.076e-01 | 2.338e-01 | 3.259e-08 | 136 |
| 100 | 4.786e-02 | 2.332e-01 | 2.469e-01 | 4.217e-02 | 125 |

600 seeds (every gate model), which is what the choice is made on:

| max_iter | nx_vs_nx median | nx_vs_nx p99 | nx_vs_nx max | ours_vs_nx max |
|---:|---:|---:|---:|---:|
| 2 | 3.116e-08 | 6.279e-08 | 1.236e-07 | 3.156e-08 |
| 3 | 4.184e-08 | 1.073e-06 | 1.541e-06 | not comparable |
| 4 | 8.065e-08 | 2.955e-06 | 1.291e-05 | not comparable |

`RESUME.md` item 4 guessed "max_iter 10-20, e.g.". **The measurement does not support
that**: networkx against itself passes 1e-6 at budget 2 and breaks it at budget 3, two
orders of magnitude below the guess, and the growth from there is four orders of
magnitude to budget 100. The guess came from reading the *port-vs-nx* column of a
1000-seed run, where the port looks flat until budget 100 — but the port-vs-nx column is
flat *because* both arms are equally chaotic, not because the system is stable.

## The choice

**`GATED_MAX_ITER = 2`**, the largest budget at which the nx-vs-nx gap stays under 1e-6 for
every seed measured (max 1.236e-07 over all 600 gate models, seed 167). Budget 3 already
reaches 1.541e-06. The gate is therefore not "the port agrees with networkx to 1e-5 after
100 iterations" — that claim is not available at any budget, and no ceiling can be
honestly set for it. What the gate states instead is the one thing a chaotic trajectory
still supports: **at 2 iterations, where one ulp of arithmetic is still bounded, the port
and the reference agree to 3.156e-08 of the layout's extent**, over every gate model.

**`CEILING = 1e-7`** in `crates/graph-cli/src/oracle_python/fa2.rs`, from the measured
port-vs-nx worst at that budget (3.156e-08) rounded up to the next power of ten: a
tolerance sitting exactly on the number it was measured from is not a tolerance. The
margin is ~3.2x, which is thin by design — it is the next decade, not a chosen factor, and
the metric it bounds is the f32 floor plus one iteration of a 2-iteration run, so there is
nothing between 3.2e-08 and 1e-7 to sit in.

## The full-iteration comparison, reported and not gating

At `max_iter = 100` (networkx's own default, which `Fa2Params::default()` keeps) both arms
are chaotic: ours_vs_nx max **4.217e-02** and nx_vs_nx max **2.469e-01** on the same seeds.
The port is not the outlier — networkx's own one-ulp reproducibility is worse there by
about 6x at the maximum — but neither is within any coordinate tolerance, so this row
gates nothing and is kept as a reported measurement. The ratio form survives it, and is
the honest summary at that budget:

| max_iter | ours_vs_nx / nx_vs_nx median | p99 | max |
|---:|---:|---:|---:|
| 10 | 0.206 | 2.72 | 2.91 |
| 100 | 0.00003 | 5.13 | 7.57 |

The median seed at 100 iterations has the port tracking networkx some four orders of
magnitude more closely than networkx tracks itself. The tail does not: at the p99 the port
is 5x worse than the library's own reproducibility, and 7.6x at the maximum. So the
defensible full-iteration claim is "usually far closer than networkx's own run-to-run
spread, and at worst a small multiple of it" — which is the `stress`-metric shape Phase 6
already uses for Barnes-Hut, not a ceiling.

## The negative control at the gated budget

`GM_MUTATE_FA2_SCALING_RATIO=3` (`Fa2State::repulsion`'s own variable), port vs the same
unperturbed reference, 200 seeds:

| max_iter | ours_vs_nx max, perturbed | median |
|---:|---:|---:|
| 2 | 2.076e-01 | 1.065e-01 |
| 10 | 1.964e-01 | 1.004e-01 |
| 40 | 5.630e-01 | 2.310e-01 |

Six orders of magnitude over the 1e-7 ceiling at the gated budget of 2 — the gate is not
vacuous, which is the failure mode a short budget invites. `crates/graph-cli/tests/
cli_fa2.rs` runs both arms end to end against a pinned `nx` reference
(`crates/graph-cli/tests/fixtures/fa2-nx-reference.jsonl`, networkx's arm over the gate
model's first 64 seeds at the gated budget, written by
`python3 harness/fa2-chaos.py <dir> --write-reference`), so the control needs neither
networkx nor the `ge-python-oracle` image; over those 64 seeds the perturbed port measures
**1.295e-01** and the honest one **3.156e-08**. The test fails if either number moves.

## Ponytail (chaos, in the code as `oracle_python/fa2.rs`'s module marker)

Failing input: any graph dense enough that one ulp of summation order grows over
`GATED_MAX_ITER` iterations — which is the 100-iteration worst case made of. Direction:
the gap is **over-stated, never under-stated**. A real port bug is caught at every budget
(the control above is 2.076e-01 against a 1e-7 ceiling), and a clean port can still go
red if it changes shape, which is the false alarm this costs. Escape hatch, both ends:
`graph-cli emit-fa2-fixtures --max-iter K` re-measures the entire comparison at any
budget without touching the metric, and `GM_MUTATE_FA2_SCALING_RATIO` perturbs the port
against the same reference, so the ceiling is falsifiable from either side.

**Not measured, therefore not claimed.** The budgets here are the gate model's sizes (2 to
601 nodes, `REFERENCE_DEGREE` 5). A denser or larger graph is not in this table, and its
chaos growth is not bounded by it. The gate's premise is arithmetic, not stability: a graph
larger than 601 nodes does not make the gate *wrong* at 2 iterations, but it is not
evidence either way about how far 100 iterations of it would drift.
