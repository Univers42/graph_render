# `sg-propose-classify` — the cause classifier counts the fixtures carrying the residual

**Job:** option 2 of `docs/measurements/sg-spring-seed.md` §"`SPRING`'s recorded cause is wrong,
and it is pinned anyway" — widen `harness/scigraphs-conformance/sc_propose.py` so a row whose
residual lands on one fixture reads as `arithmetic` rather than `convention`.
**Status:** done. One row's `(tier, cause)` moved: `SPRING`. No layout byte moved.

## What the rule was, and what broke it

`sc_propose.classify` reached `arithmetic` only when the shape agreed (`disparity <= 1e-3`) **and**
the row's largest raw gap was `<= ARITHMETIC_GAP` (1e-6). Both halves were deliberate: `GRID`
measures a disparity of 5e-32 and a gap of 4.0, so the same lattice in a different unit must not
be sent to a repair job to chase a summation order that is already right.

But the largest gap over the row is **one fixture's** gap, and `SPRING` is not a unit mismatch:

| fixture | coordinates | f32 identical | max gap | Procrustes |
|---|--:|--:|--:|--:|
| `lesmis` | 231 | 78 | 2.3736557427289084e-03 | 1.663916971293011e-08 |
| `tree-balanced` | 45 | 45 | 9.054289717980168e-08 | 2.3562483399955027e-16 |
| `dag-diamond` | 12 | 12 | 1.4206554421747342e-07 | 2.954715407083551e-16 |
| `bipartite` | 42 | 42 | 2.2464913485009674e-07 | 6.87290889271226e-16 |
| the other 20 fixtures | 690 | 690 | <= 2.4e-07 | <= 7.6e-16 |

One fixture of 24 carries 2.37e-03 — the 77-node one, whose 50 chaotic iterations amplify a
1.6-ulp reduction difference (`sg-spring-seed.md` §"the one reduction that names it"). The other
23 are `f32`-identical to the last bit. The largest-gap rule read that single fixture as a
convention, so the row's recorded cause named a repair that would move nothing.

## The rule now

`sc_propose.ARITHMETIC_RESIDUAL_FIXTURES = 1`, applied by `sc_propose._same_method`: a row whose
worst shape verdict is within `ARITHMETIC_DISPARITY` is `arithmetic` when its aggregate gap is
within `ARITHMETIC_GAP` (the narrow test, unchanged) **or** when at most one of its fixtures
carries a gap above `ARITHMETIC_GAP`. `rng` still sits before `convention`, and a row that
declares a `layout seed` gap is excluded from the widened half: its arms never started alike, so
a residual on one fixture is the seed, not the summation order.

**Why a count of one is the smallest rule that separates the three cases.** The measured counts
over the 32 rows, fixtures with a gap above 1e-6: `SPRING` 1 of 24; `CIRCLE_PACKING` 3 of 24;
`IGRAPH_KK` 23 of 23; and **every other row with a gap at all, 24 of 24** — `GRAPHVIZ_TWOPI`
(270.5 on `lesmis`, 90.0 to 171.0 on the rest), `GRAPHVIZ_PATCHWORK`, all eleven Graphviz and
igraph rows. There is no row between 1 and 23, so `1`, "one half" and "a small fraction" draw the
same line on this matrix; the integer is the one that names what it counts, and it is the value
to raise if a row ever lands between. `Caveat:` a residual spread thinly over several **large**
fixtures — 4.0 on six of twenty-four and 0.0 on the rest — reads as `convention`; no such row has
been measured.

The two rows that must not move are the two halves of the old test: `GRAPHVIZ_TWOPI` measures its
whole 90.0-unit step on all 24 fixtures (`bitwise`/`convention`, unchanged) and `GRID` as it
measured before `sg-grid-scale` — disparity 5e-32, gap 4.0 on all 24 — is still a `convention`.

## The test

`harness/scigraphs-conformance/test_sc_propose.py` (new; `sc_propose.py` had no test). It drives
`classify` with measured rows rather than hand-written dicts, and holds three cases apart:

| case | fixtures over 1e-6 | cause |
|---|--:|---|
| `SPRING`, 24 fixtures transcribed from `metrics.json` | 1 | `arithmetic` |
| `GRAPHVIZ_TWOPI`, its four named fixtures plus its 20 gates | 24 | `convention` |
| `GRID` before its scale fix, 24 fixtures at 4.0 | 24 | `convention` |

plus four that keep the rest of the rule: two residual fixtures (`SPRING`'s numbers with a second
fixture lifted to 2.4e-03) → `convention`, a different shape with one residual fixture
(`CIRCLE_PACKING`'s 0.517 on `lesmis`) → `shape`/`algorithm`, a `layout seed` gap → `bitwise`/`rng`,
and an entry with no `per_fixture` → `convention`, because an uncountable residual is not zero.

```
docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle \
    python3 harness/scigraphs-conformance/test_sc_propose.py -v
```

**RED, before the change** (1 failure of 7 — the `SPRING` case only; the six negative controls
passed, so the change could not have been made by loosening them):

```
FAIL: test_one_fixture_carrying_the_residual_is_arithmetic
AssertionError: 'convention' != 'arithmetic'
Ran 7 tests in 0.001s
FAILED (failures=1)
```

**GREEN, after:** `Ran 7 tests in 0.000s / OK`.

## Re-proposed rows

`scripts/scigraphs-conformance.sh` on the changed tree: `conformance-baseline-proposed.rs` against
the pinned table, all 32 rows, one pair moved.

| row | old tier/cause | new tier/cause |
|---|---|---|
| `SPRING` (`baseline/table/basic.rs`) | `bitwise` / `convention` | `tolerance` / `arithmetic` |

The tier moves with the cause because `classify` returns them as one verdict: `arithmetic` is
reached from the `tolerance` branch, and pinning `arithmetic` beside `bitwise` would be a
hand-written record no measurement supports — the thing `sg-spring-seed.md` refused to accept. The
two shas and the `1e-15` ceiling are byte-identical, and the judge's own per-row lines are
identical between the two runs: **no f32 or f64 count moved on any row.** No other row's cause or
tier moved in either direction, which is the regression check `sg-common.md` step 4 asks for.

## Commands

| command | exit |
|---|--:|
| `scripts/orch/gr cargo build --release -p graph-cli` (untouched tree) | 0 |
| `scripts/scigraphs-conformance.sh` (untouched tree) | 0 |
| `docker run ... ge-python-oracle python3 harness/scigraphs-conformance/test_sc_propose.py` (RED) | 1 |
| `docker run ... ge-python-oracle python3 harness/scigraphs-conformance/test_sc_propose.py` (GREEN) | 0 |
| `scripts/scigraphs-conformance.sh` (after the rule, before re-pinning) | 0 |
| `scripts/scigraphs-conformance.sh` (after re-pinning `SPRING`) | 0 |
| `scripts/scigraphs-conformance.sh --break` | 1, and names `SPRING_3D` |
| `scripts/orch/gr cargo fmt --all -- --check` | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test -p graph-cli conformance` | 0 |

## Still open, and not this job's

The **2.37e-03** itself. `SPRING` is now labelled by what it is, but the fused reduction
(`sg-spring-seed.md` option 1) is what removes it, and that changes the bytes of two registered
layouts, their goldens, the 1000-seed `oracle-spring` differential and a `--past-ceiling`
benchmark. That is a deliberate re-pin and a job of its own; this one changed no layout byte.