# sg-spring-seed — `SPRING`, `SPRING_3D`, `CIRCLE_PACKING`: the layout seed

Three rows, one gap. `SpringParams` had no seed field and drew its start from the crate's own
`Mulberry32`, while the reference passes `seed=get_layout_seed()` — 981798123 — to
`nx.spring_layout`, which networkx turns into `np.random.RandomState(seed)`
(`utils/misc.py:290-291`). Equal seeds are not equal draws, so no coordinate of ours could ever be
the reference's. `CIRCLE_PACKING`'s non-planar fallback had the same defect one level down: it
runs its own dense Fruchterman–Reingold pass, and SciGraphs seeds that pass with
`nx.spring_layout`'s positions too (`circle_packing.py:428`).

The generator, not the seed value, was the cause. `RandomState` is seeded by `init_genrand`
(`mt[0] = seed`, then the `1812433253` recurrence) and its `random_sample` is **two `u32` words
per double**, `(a * 2^26 + b) / 2^53` with `a = word >> 5` and `b = next word >> 6`. That is
[`Mt19937`](../../crates/graph-core/src/rng.rs), ported by `sg-mt19937`; this job only had to
call it.

## What landed

| file | what |
|---|---|
| `layout/force/spring.rs` | `SpringParams.seed: Option<u32>`. `Some(s)` draws `np.random.RandomState(s).rand(n, D)` row-major; `None` keeps this crate's `Mulberry32` at `0x5EED`, **byte for byte what it was**. `start(n, seed)` picks the arm and two six-line helpers do the drawing. |
| `layout/force/spring/tests/seed.rs` | the vectors, a neighbouring-seed control, the `None`-is-still-Mulberry32 pin, and a public-path test that the field reaches the kernel. Split out because `spring/tests.rs` was already 258 of the 300-line house cap. |
| `layout/circle_packing.rs` | `CirclePackingParams.seed: Option<u32>`, default `None`. **Outside the job's listed paths** — see *Deviations*. |
| `layout/circle_packing/fallback/seed.rs` | `seed_positions` split into `seeded(n, seed)` (the reference's run) and `start_positions(n, seed)` (one arm or the other, never a blend); `fruchterman_reingold` takes the seed through. |
| `conformance/motor/overrides.rs` | the three arms pass `Some(LAYOUT_SEED)`. |
| `conformance/rows.rs`, `gaps.rs` | `G_SPRING_SEED` deleted and dropped from `SPRING` and `SPRING_3D`. |
| `baseline/table/basic.rs` | the three rows re-pinned from `conformance-baseline-proposed.rs`, copied not retyped. |

**The registered defaults did not move.** `layout.force.spring`, `layout.force.spring3d` and
`layout.packing.circle` all keep their own streams at `None`, so every hashed snapshot, every
hash-gate record and the fallback's golden packing are untouched. `spring/tests/seed.rs` pins the
`None` arm's own bits so "untouched" is an assertion and not a hope, and
`circle_packing/fallback/tests.rs` pins the spiral as its closed form
(`sqrt((i+1)/n)` out at `i * GOLDEN_ANGLE`).

## Step 1, measured on the untouched tree

`scripts/orch/gr cargo build --release -p graph-cli` then `scripts/scigraphs-conformance.sh`,
exit 0 on the untouched tree, rows read from `target/scigraphs-conformance/metrics.json`:

| row | tier | cause | f64 k/N | f32 k/N | max gap | Procrustes median |
|---|---|---|--:|--:|--:|--:|
| `SPRING` | `bitwise` | `rng` | 341/1020 | 341/1020 | 10 | 0.3770880938138461 |
| `SPRING_3D` | `bitwise` | `rng` | 3/1020 | 4/1020 | 10 | 0.19817674515400713 |
| `CIRCLE_PACKING` | `shape` | `algorithm` | 344/1020 | 808/1020 | 3.167440222876345 | 5.297068645513404e-16 |

## The RED runs

`crates/graph-core/src/layout/force/spring/tests/seed.rs` on the untouched tree — a field that does
not exist cannot be set, and a function that does not take one cannot be called:

```
error[E0061]: this function takes 1 argument but 2 arguments were supplied
  --> crates/graph-core/src/layout/force/spring/tests/seed.rs:53:5
   |     start::<D>(n, seed)
error[E0560]: struct `SpringParams` has no field named `seed`
  --> crates/graph-core/src/layout/force/spring/tests/seed.rs:108:21
   |  108 |                     seed,
   | note: available fields are: `threshold`, `scale`
```

`circle_packing/fallback/tests.rs`, same shape:

```
error[E0061]: this function takes 1 argument but 2 arguments were supplied
  --> crates/graph-core/src/layout/circle_packing/fallback/tests.rs:25:15
error[E0560]: struct `CirclePackingParams` has no field named `seed`
  --> crates/graph-core/src/layout/circle_packing/fallback/tests.rs:76:17
```

**One honest correction to the job's own recipe.** The vectors are pinned as bits of
`np.random.RandomState(981798123).rand(2, 3)`, but `Field` stores one column per axis, so a
helper that flattens the columns compares axis-major against a row-major reference and fails on a
correct implementation. The first green attempt did exactly that and produced

```
  left: [4606385690000596347, 4600361976408293038, 4603304166055163273, 4596209139354194488]
 right: [4606385690000596347, 4603304166055163273, 4600361976408293038, 4596209139354194488]
```

— every value right, in the wrong order. The fix is a `spread::<D>` that distributes the flat
row-major reference back over the columns; the draw *order* is unchanged by it, because
`rand(n, D)` fills in C order and `Field`'s columns are that same flat run viewed by axis.

## Before and after

| row | | f64 k/N | f32 k/N | max gap | Procrustes median | tier | cause |
|---|---|--:|--:|--:|--:|---|---|
| `SPRING` | before | 341/1020 | 341/1020 | 10 | 0.377 | `bitwise` | `rng` |
| | **after** | **362/1020** | **866/1020** | **2.3736557427289084e-03** | **3.7675322319180786e-16** | `bitwise` | `convention` (see below) |
| `SPRING_3D` | before | 3/1020 | 4/1020 | 10 | 0.198 | `bitwise` | `rng` |
| | **after** | **24/1020** | **1020/1020** | **2.3605908339163761e-07** | **5.031660098057955e-16** | `tolerance` | **`arithmetic`** |
| `CIRCLE_PACKING` | before | 344/1020 | 808/1020 | 3.167440222876345 | 5.297068645513404e-16 | `shape` | `algorithm` |
| | **after** | 344/1020 | 808/1020 | 2.6994058996143533 | 5.297068645513404e-16 | `shape` | `algorithm` |

`SPRING_3D`'s `f64` residue of 24 is one coordinate per fixture, and `CIRCLE_PACKING`'s totals do
not move because **the fallback is only reachable from a non-planar graph and 22 of the 24
fixtures are planar** — see below.

### `SPRING` is 866, not 1020, and 153 of the missing 154 are on one fixture

| fixture | coordinates | f32 identical | max gap | Procrustes |
|---|--:|--:|--:|--:|
| `lesmis` (77 nodes) | 231 | **78** | 2.3736557427289084e-03 | 1.663916971293011e-08 |
| `gate-16` (18 nodes) | 54 | **53** | — | — |
| `tree-balanced` | 45 | 45 | 9.054289717980168e-08 | 2.3562483399955027e-16 |
| `dag-diamond` | 12 | 12 | 1.4206554421747342e-07 | 2.954715407083551e-16 |
| `bipartite` | 42 | 42 | 2.2464913485009674e-07 | 6.87290889271226e-16 |
| the other 19 `gate-*` | 639 | 639 | — | — |

**21 of the 23 measured fixtures are `f32`-identical to the last bit, and `gate-16` is off by one
coordinate out of 54.** `lesmis` carries the other 153. Its **Procrustes disparity is 1.7e-08** —
the drawing is the reference's *shape*; only the last bits of the coordinates are not. That is what
one ulp of arithmetic looks like after 50 chaotic iterations, and the fact that 21 fixtures land
on the last bit at all is the seed being right rather than merely close.

### The one reduction that names it, measured on the smallest graph that has it

`gate-00` is two nodes and one edge: 50 iterations, one antisymmetric pair, no chaos, every
rounding decision visible. Both variants run on the same
`RandomState(981798123).rand(2, 2)` start in IEEE f64:

- **the reference** (`layout.py:703-705`) forms **one factor per pair**, `k*k/d**2 - A*d/k`, and
  sums `delta * factor` once over `j` in an `einsum`;
- **graph-core** (`forces.rs:118-139`) sums repulsion over every `j`, then subtracts attraction
  over `i`'s own CSR row — the same sum, a different rounding, and the price of not materialising
  an `n x n` matrix at `n = 20_000`.

```
networkx  (fused, one sum over j): [5.0, 3.461936712619982, -5.000000000000001, -3.461936712619982]
graph-core(two loops, CSR row)   : [5.0, 3.4619367126199827, -4.999999999999999, -3.4619367126199827]
max |dx| after rescale: 1.776357e-15
bitwise identical: False
```

1.78e-15, i.e. about 1.6 ulp of `f64` at a coordinate of 5. Fifty chaotic iterations on the
77-node 2D fixture turn that into 2.37e-03. **The identical kernel at `D = 3` turns it into
2.36e-07**, which is why `SPRING_3D` reaches 1020/1020 `f32` and `SPRING` does not: at three axes
the reference's own `rescale_layout` divides by a larger `lim`, and the third column carries the
residue where the `f32` rounding swallows it.

### `SPRING`'s recorded cause is wrong, and it is pinned anyway

`sc_propose.py` reaches `arithmetic` only when the shape agrees (`disparity <= 1e-3`) **and** the
raw gap is `<= 1e-6` (`ARITHMETIC_GAP`), and both halves are deliberate — `GRID` has a disparity
of 5e-32 and a gap of 4.0. `SPRING` now measures a disparity of 3.77e-16 and a gap of 2.37e-03, so
it falls through to `convention`. **No scale, centre or axis order accounts for 2.37e-03 and
undoing one would move nothing**, so the label points the next repair at the wrong thing.

The row is pinned as the classifier computed it rather than hand-edited: `baseline/table/basic.rs`
is a transcription of `conformance-baseline-proposed.rs`, and a hand-written cause would be a
record no measurement supports. Closing the discrepancy is one of two things, both outside this
job's paths:

1. **fuse the reduction** — sum `delta * (k*k/d**2 - A*d/k)` once over `j`, walking `i`'s CSR row
   with a cursor instead of a second loop. It is the port's own documented reason for the split
   (`forces.rs:12-19`) that it avoids the dense matrix, and it changes the bytes of two
   *registered* layouts: their goldens in `spring/tests/golden.rs` and `spring3d/snapshots.rs`,
   the 1000-seed `oracle-spring` differential, and a `--past-ceiling` benchmark.
2. **widen the classifier** — a rule that counts how many fixtures carry the residual, so a row
   that is exact on 22 of 23 and off by 2.4e-03 on the twenty-third reads as arithmetic rather
   than convention. This is a change to `harness/scigraphs-conformance/sc_propose.py`, which
   every sg job shares.

Recommendation: **(2) first**, because it is a measurement rule and costs no layout bytes, and
(1) only if a job is willing to re-pin two registered layouts on purpose.

### `CIRCLE_PACKING`: the fallback is seeded, and what is left is arithmetic

`seed_positions` is now `start_positions(n, seed)`: `Some(s)` draws the reference's
`RandomState(s).rand(nnodes, 2)` row-major at `f64`, `None` is the golden-angle spiral unchanged.
The parameters are already the reference's and were not touched — `frame = scale * 0.45` and
`seed_iterations = max(10, min(50, 20000 // n))` (`circle_packing.py:424-429`).

**Which fixtures this can move at all, measured rather than assumed.** The fallback is taken
exactly when `_planar_triangulation` returns `None` (`circle_packing.py:307-311`), which for
every fixture here means the graph is not planar, so `networkx.check_planarity` over the 24
emitted fixtures decides it:

```
lesmis           n=77   m=254  planar=False
tree-balanced    n=15   m=15   planar=True
dag-diamond      n=4    m=4    planar=True
bipartite        n=14   m=48   planar=False
--- non-planar: 2 of 24 -> ['lesmis', 'bipartite']
```

So **22 of 24 fixtures never read the seed at all** — the 20 gate models, plus `tree-balanced`
(a 15-node tree) and `dag-diamond`. That is why the row's `f32` (808/1020) and `f64` (344/1020)
totals are unchanged to the digit, and it corrects a claim this file made in its first draft:
`tree-balanced`'s Procrustes 0.4176 is the **exact** path's residual, it predates this repair, and
the seed cannot have moved it.

Of the two that do reach the fallback:

| fixture | Procrustes before | after | max gap after |
|---|--:|--:|--:|
| `lesmis` | **0.517** | **0.08949617241773522** | 0.8547108458144601 |
| `bipartite` | (not separately recorded before) | 0.8628699456601677 | 2.60200735558757 |

`lesmis` is the measurement that matters: **0.517 was the seed, 0.0895 is the arithmetic.**
Neither is exact, because the fallback's own reduction differs from the reference's in three named
ways:

- `FrField::displacement` fuses nothing where `layout.py:703-705` forms one factor per pair;
- it measures distance with `libm::hypot` where `np.linalg.norm` is `sqrt(x*x + y*y)`;
- it scales by `d * t / len` where numpy computes `d * (t / len)` (`layout.py:711`).

The row keeps `shape`/`algorithm` because `sc_propose.py` reads the Procrustes **worst** (0.8629 on
`bipartite`) and a shape a similarity does not explain is `algorithm` by that rule — which is at
least the right neighbourhood, and unlike `SPRING`'s it is not claiming a repair that would move
nothing.

### The sparse fork, recorded where the seed is drawn

Both seeded entry points carry a `Caveat:` line, because the port reproduces the **dense** start
only. At `n >= 500` `spring_layout` builds `A` with `dtype="f"` (`layout.py:629`) and casts `pos`
to it (`layout.py:672`), so the reference's own start is **float32** there and half its bits are
gone before the first force — a different answer that no seed value fixes, and one the port does
not reproduce at all. Every conformance fixture is under 500 nodes, so nothing here is measured
against that fork and nothing here claims it.

## Commands

```
scripts/orch/gr cargo build --release -p graph-cli                        -> 0
scripts/scigraphs-conformance.sh                        (untouched tree)   -> 0
scripts/orch/gr cargo test -p graph-core --lib layout::force::spring     -> RED (E0061, E0560)
scripts/orch/gr cargo test -p graph-core --lib layout::circle_packing    -> RED (E0061, E0560)
scripts/orch/gr cargo test -p graph-core --lib layout::force::spring     -> 0 (37 passed)
scripts/orch/gr cargo test -p graph-core --lib layout::circle_packing    -> 0 (137 passed)
scripts/scigraphs-conformance.sh                        (after)            -> 1, names SPRING, SPRING_3D, CIRCLE_PACKING
scripts/scigraphs-conformance.sh                        (after re-pin)     -> 0 PASS
scripts/scigraphs-conformance.sh --break                                  -> 1 (SPRING_3D bytes)
scripts/orch/gr cargo fmt --check                                          -> 0
scripts/orch/gr cargo clippy --release --workspace --all-targets -- -D warnings -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                      -> 0
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-spring-fixtures --seeds 20 --out target/spring-fixtures-check20
docker run ... ge-python-oracle python3 harness/oracle-spring.py target/spring-fixtures-check20
scripts/orch/gr cargo run -q --release -p graph-cli -- oracle-spring --dir target/spring-fixtures-check20 -> 0
scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8  -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q --release -p graph-cli -- hashgate --seeds 8 -> 1
docker run ... ge-python-oracle python3 /probe/planarity.py   (check_planarity over the 24 fixtures) -> 2 of 24 non-planar
```

The spring differential above is **20 seeds, not the 1000 the gate runs**: the job brief forbids
running a timed gate, and a full `emit-spring-fixtures --seeds 1000` chain is one. It reads
`layout.force.spring: 19/20 cases correlated (1 no correlation exists), median deficit 2.368e-5
at ceiling 1e-1: ok [worst 1.861e-1 at seed 17, p90 1.635e-1]`. **A 20-seed median is not the
1000-seed median the module doc records (7.288e-3), so this is a green light, not a re-measurement
of that number.** It is green for a structural reason as much as a numerical one: the differential
runs `layout.force.spring` at its registered default, `seed: None`, which is pinned bit for bit by
`the_unseeded_start_is_still_the_crates_own_mulberry32_stream`.

## Gaps closed

`G_SPRING_SEED` is deleted from `gaps.rs` and dropped from `SPRING` and `SPRING_3D` in `rows.rs`.
No row names it any more. Both rows are now `convention`-free in the JSON the harness writes, so
`sc_propose.py`'s `_has_seed_gap` is false for them for the first time — which is what moved
`SPRING_3D` to `arithmetic` (the arithmetic branch is above the seed branch) and is what moved
`SPRING` to `convention` (see above).

## Deviations

- **`crates/graph-core/src/layout/circle_packing.rs`** is not in the job's listed paths. The seed
  cannot reach `seed_positions` without it: `CirclePackingParams` is the only value `pack()` and
  `fallback::pack()` already share, and `fallback::pack` is at its 4-parameter cap. Adding the
  field there is the smallest edit that leaves the registered default at `None`.
- **`crates/graph-core/src/layout/circle_packing/tests/{edges,small}.rs`**,
  **`circle_packing/fallback/{tests,seed/tests}.rs`** and
  **`conformance/motor/tests.rs`** are constructor sites for `CirclePackingParams`, and CLAUDE.md's
  "a new struct field needs every constructor" binds them; each got `..Default::default()` or the
  explicit `seed: None`.
- **`harness/scigraphs-conformance/sc_propose.py`** was read and deliberately **not** edited: it
  is shared by every sg job, and its `ARITHMETIC_GAP` is what mislabels `SPRING`.
