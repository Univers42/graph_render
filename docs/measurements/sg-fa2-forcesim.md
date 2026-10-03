# sg-fa2-forcesim — FORCEATLAS2: the reference arm runs ForceSim, so graph-core runs ForceSim

**Status: done.** The row moved from `2.406e-1` to a `1.402e-13` Procrustes median — seventeen
orders of magnitude, and past the job's `1e-3` target by ten. A second layout,
`layout.forceatlas2.forcesim`, is the port of what the reference actually executes, and
`layout.forceatlas2` (the networkx port) is untouched and still green against `oracle-fa2`.

| | tier | cause | median | max | f32 bitwise | f64 bitwise | coords |
|---|---|---|---|---|---|---|---|
| before | `bitwise` | `rng` | 2.406e-1 | 9.27e-1 | 0 | 0 | 1020 |
| after | `bitwise` | `convention` | 1.402e-13 | 1.307e-9 | 72 | 23 | 1020 |

`target/gates/scigraphs-conformance.json`, `.functions.FORCEATLAS2`, and
`target/scigraphs-conformance/metrics.json`, `.rows.FORCEATLAS2`. Ceiling `1e-12`, so the row
passes. The max gap is `6.23e-4` — one coordinate of `lesmis`, which is the worst of the 24.

**No other row moved.** The judge named only `FORCEATLAS2` on the run that produced the
proposed baseline, and every other line of the summary is byte-identical to the run before this
job.

## What the previous branch established, and what this one does about it

`docs/measurements/sg-fa2-seed.md` proved, three independent ways, that `forceatlas.py:167`
always takes `_forceatlas2_forcesim` and that the networkx branch below it is dead code in
`ge-python-oracle`. Its recommendation was a separate ForceSim port, and named four things it
would need. All four are here:

1. a PCG64 generator beside `Mt19937` (§1);
2. a `model='FA2'` kernel — `f32` state, the `k = scale / cbrt(n)` move cap, gravity `× 0.1`,
   `sim.step(50)` (§2);
3. `_fa2_rescale` inside the layout, because the reference returns rescaled points (§2);
4. and, which the recommendation did not foresee, **the row's `motor` id had to change** —
   `layout.forceatlas2` is the networkx port and is gated against networkx 3.6 by
   `oracle-fa2`, so re-pointing it here would have broken its own oracle. Hence the new id.

## §1 — PCG64, `crates/graph-core/src/rng/pcg64.rs`

`default_rng(seed)` is `PCG64(SeedSequence(seed))`. Three things had to be right and each is
pinned separately, because a port that gets one wrong produces a stream that is plausible and
is not the reference's:

| what | where | pinned by |
|---|---|---|
| `SeedSequence(entropy).pool` — `mix_entropy`'s four-word pool, `hashmix` with its running multiplier, then the `4 × 4` `mix` sweep | `rng/pcg64/seed_sequence.rs` | `seedsequence_pool_is_the_mixed_entropy_pool` |
| `generate_state(4, np.uint64)` — eight `u32` over the cycled pool, read as four little-endian `u64` | same | `seedsequence_generate_state_is_the_four_u64_words_pcg64_is_seeded_with` |
| `pcg64_set_seed` → `pcg_setseq_128_srandom_r`: `state = 0`, `inc = (seq << 1) \| 1`, step, `+= initstate`, step | `rng/pcg64.rs` | `pcg64_seeding_reaches_the_state_and_inc_numpy_reports` |
| `pcg64_next64`: step, **then** `rotr64(state.high ^ state.low, state.high >> 58)` | same | `pcg64_next_u64_is_numpys_raw_output_at_the_layout_seed` |
| `uint64_to_double`: `(next >> 11) * 2^-53` | same | `pcg64_next_f64_is_numpys_default_rng_at_both_seeds` |

Measured in `ge-python-oracle` (numpy 2.3.3) at `seed = 1767573729`:

```
state 60223290111031304053904000740221457340   inc 232180640892196755669630849829386553819
random(6) 0x3FE96B8FA5E53C4A 0x3FE5525D058739CE 0x3FE3A71AE513D623
         0x3FC1C7A8FC6458F4 0x3FEB5006A7EEBDE4 0x3FE9C989DD74267F
```

The one that bites: **`PCG_DEFAULT_MULTIPLIER_128` is `(2549297995355413924 << 64) |
4865540595714422341`** — the constant's `high` half goes in the high position, because
`PCG_128BIT_CONSTANT(high, low)` is `(high << 64) + low`. Swapped, the generator still returns
numbers in `[0, 1)` and is wrong from the first draw.

The whole state is integer-only — xor, shift, `wrapping_mul`, one `u128` add — so it is
bit-identical native and wasm32 (D1, D2, D10); `u128` arithmetic is software on wasm32, not a
widening float.

**RED was observed**, but not in the usual order: the tests and the implementation were
written in one pass, so the first run was green. The suite was then broken deliberately
(`SeedSequence::pool(seed + 1)`) and four of the eight failed —
`pcg64_next_f64_is_numpys_default_rng_at_both_seeds`,
`pcg64_next_u64_is_numpys_raw_output_at_the_layout_seed`,
`pcg64_seeding_reaches_the_state_and_inc_numpy_reports` and the negative control — which is
what says the vectors bite.

## §2 — The kernel, `crates/graph-core/src/layout/force/forcesim/`

`forceatlas.py:92-147` into `simulation.py:230-1094`, `model='FA2'`, `repulsion_mode` direct.
Only the path the fixtures take: every fixture is under `DIRECT_MAX = 400`, where
`_force_field` (`simulation.py:1003-1004`) takes the direct branch whatever `repulsion_mode`
says. Modules: `seed.rs` (the start), `state.rs` (`_renormalize`), `reduce.rs` (the
reductions), `pair_force.rs`, `attraction.rs`, `gravity.rs`, `integrate.rs`, `rescale.rs`.

### The dtype trace

Under NEP 50 a Python `float` is **weak** — it does not promote an `f32` array — and an
`np.float64` **scalar is not weak** and promotes the whole expression to `f64`. Three scalars
here are `np.float64` and all three come from `np.cbrt`:

| expression | dtype | why |
|---|---|---|
| `self.k = scale / max(np.cbrt(max(n,1)), 1.0)` | `np.float64` | `np.cbrt` returns one |
| `self.repulsion = raw * (k**2 / m3)` | `f64` | `repulsion` is `np.float64`, so `coeff` in `_pair_force` is an `f64` `(chunk, n)` block |
| `self.gravity = raw * (0.38*2.0*scale**2 / (max(k,1e-9)*m2))` | `f64` | same: all of `_gravity` is `f64` until `.astype(DTYPE)` |
| `self.speed = float(np.clip(...))` | **`f32`** | a Python `float` is weak, so `factor = speed / (1.0 + speed * np.sqrt(swing))` stays `f32`, and `disp` and `norm` with it |
| `total_swing = float(np.dot(mass, swing))` | `f32` reduction, `f64` re-wrap | `np.dot` on `f32` returns `f32` |
| `np.bincount(src, weights=pull[:, ax])` | `f64` | numpy's `bincount` declares `double*` weights, whatever the weights' dtype |
| `np.einsum("ij,ij->i", a, a)` | `f32` | `einsum` accumulates in the array's own dtype |
| `force += _attraction()`, `force += _gravity()` | `f32` | both operands already narrowed |
| `self.pos -= (self.pos.mean(axis=0) - center_was)` | `f32` | both means are `f32` |
| `_fa2_rescale` | `f64` throughout | it is handed `np.asarray(positions, dtype=np.float64)` |

**The `speed` row is the one that cost the most to find, and it is worth writing down.** The
first port read `self.speed` as `f64` — it is stored in a Python `float` and every other
constant in the file is an `np.float64` — and got a layout that agreed with the reference to
about `1e-6` and no closer. One `f32` ULP per move, `50` moves, and the two arms had drifted
by a ULP on 12 of 45 coordinates of `tree-balanced` by iteration 5. `self.pos += disp` with an
`f64` `disp` is an in-place same-kind cast; with an `f32` `disp` there is no narrowing at all.

### The reductions

Each named for the expression it replaces, and each **measured** in `ge-python-oracle` rather
than assumed:

| reference expression | numpy's order | reproduced |
|---|---|---|
| `mass.mean()` | pairwise `f32`, block 8 | yes, exactly |
| `coeff.sum(axis=1)` | pairwise `f64`, block 8 | yes, exactly |
| `pos.mean(axis=0)`, `arr.mean(axis=0)` | **sequential** `f32`/`f64` per column | yes, exactly |
| `np.einsum("ij,ij->i", a, a)` | sequential `f32`, `((a+b)+c)` | yes, exactly |
| `np.bincount` | sequential, in edge index | yes, exactly |
| `np.dot(mass, swing)`, `np.dot(mass, traction)` | **BLAS `sdot`** | **no** |
| `t @ sources.T` (`k = 3`) | **BLAS `sgemm`** | **no** |
| `coeff @ sources` | **BLAS `dgemm`** | **no** |

Two asymmetries in that table are worth a line each, because both look like typos:

- **`mean()` is pairwise; `mean(axis=0)` is not.** A contiguous one-dimensional reduction
  runs `pairwise_sum_FLOAT`; a reduction along the *outer* axis of a C-contiguous `(n, 3)`
  array runs the plain `add` inner loop. Measured over `n` in {4, 15, 77, 128, 129, 200,
  1000}: sequential matches at every size, pairwise does not from `n = 77`.
- **The pairwise remainder is folded into the tree, not added to it.** numpy writes
  `res = res + a[i]` in a loop; `tree + (a[8] + a[9] + ...)` is a different `f32`. On the
  fifteen values `total_swing` is actually reduced over at `tree-balanced`'s second iteration,
  the two are one ULP apart — `0x43808672` against `0x43808671` — and one ULP of `total_swing`
  is one ULP of `speed` and from there of every position. That was the last thing standing
  between this port and a bit-exact match on the small fixtures;
  `reduce.rs::the_remainder_is_folded_into_the_tree_and_not_added_to_it` is its negative
  control.

### The residual cause: three BLAS kernels

None of the three is reproducible from a portable Rust, and each was measured before it was
accepted.

**`np.dot(mass, swing)` — BLAS `sdot` on `f32`.** Measured over `n` in {4, 7, 8, 15, 16, 77,
128, 129, 200, 1000} at two seeds: it matches numpy's pairwise `f32` order at some sizes and
not at others, matches `f64`-then-narrowed at yet others, and matches none of them at every
size. The kernel is vectorised and its blocking depends on the length. **This is the one the
row's residual rests on.** The port uses the pairwise `f32` order, which is also what
`mass.mean()` needs, so one implementation covers both.

**`t @ sources.T` — BLAS `sgemm` with an inner dimension of three.** Measured on 5929 cells of
`(n, 3) @ (3, n)`: **0** differ from `acc = fma(a_k, b_k, acc)` and **2275** differ from a
sequential `f32` dot. The reference's kernel is an FMA chain. The port uses the sequential
`f32` dot: an FMA is not a portable primitive under D1 (`mul_add`'s result is
target-dependent), and emulating one through `f64` is a double rounding dressed up as a single
one. Measured cost of the substitution: **zero** — the finished layouts are bitwise identical
either way.

**`coeff @ sources` — BLAS `dgemm` over `n`.** Replaced with a sequential `f64` dot. Measured
cost: **zero**, for the same reason — both products are rounded to `f32` on store
(`simulation.py:74`) and an `f64` ULP dies in that rounding.

### What the transliteration says, and how far the port is from the reference

Before any Rust was written, the algorithm was transliterated into Python and checked against
the real `ForceSim` in the oracle. **With the reference's own BLAS calls it reproduced
`ref/FORCEATLAS2.f64` bit for bit on all 24 fixtures, all 1020 coordinates.** That is what
says the algorithm, the seed chain, the `f32` state and the rescale are right, before any of
them were ported.

With the three BLAS kernels replaced by fixed orders, the same transliteration sits at a
**median mean absolute `1.02e-6`** over the 24 fixtures, worst single coordinate `6.23e-4`, on
coordinates of magnitude about `2.5`. The conformance run reports the same worst gap,
`6.2267e-4`, and a Procrustes median of `1.402e-13` — the disparity measure is scale- and
rotation-invariant, so a relative error of `2.5e-4` on one coordinate is a median of `1e-13`
across the cloud.

## §3 — Registered, and what it costs to register

`layout.forceatlas2.forcesim` (`registry.rs`, metadata in
`registry/forceatlas2_forcesim.rs`), every `Metadata` field filled, `capabilities --check`
green, `codegen --check` unaffected (no layout id reaches any generated artifact).

`FA2_FORCESIM_CEILING = 2_000`, `Ponytail (scale_ceiling)`: extrapolated from
`layout.forceatlas2`'s measured 14 000 ceiling and this port's cost per pair, **not measured
at 2 000 nodes**. The direct repulsion is `n²` per iteration over 50 iterations, and the
transient coefficient block is `CHUNK * n * 8` bytes.

Four `Ponytail` markers on the layout itself, each with a failing input, a direction and an
escape hatch: the **repulsion mode** (`DIRECT_MAX = 400` and the reference's own TREE / near-far
split above it, unported), the **planar start** (the reference's `standard_normal` ziggurat
nudge, which cannot fire on a uniform random start), the **parameters** fixed at their
`forceatlas.py` defaults, and the **early exit** that ForceSim does not have.

## §4 — The conformance arm

- `rows.rs`: the row's `motor` is `layout.forceatlas2.forcesim`, with the reason in a comment.
- `motor/overrides.rs`: `fa2_forcesim` passes `iterations = 50`, `scale = 5.0` and
  **`seed = FORCESIM_SEED = 1_767_573_729`** — *not* `LAYOUT_SEED`. The reference draws one
  seed from the layout seed (`forceatlas.py:122`) and hands that to `default_rng`; passing
  `981798123` instead is the bug `sg-fa2-seed` documented.
- `gaps.rs`: **`G_FORCEATLAS2_SEED` is deleted**, const and comment, and dropped from the
  row's `gaps`. A gap left behind after the fix is a false record. `G_SNAPSHOT_SCALE` stays —
  it is about `scale`, not about the seed.
- `conformance.rs`: `FORCESIM_SEED` is a named constant with the derivation, so the number in
  the arm is not a bare literal.

## §5 — The transcript

`harness/scigraphs-conformance/sc_reference.py` captured the reference's stdout at `:48,51`
and threw it away. That is how a branch compared the motor's networkx port against a
reference that was running ForceSim: the line that says which tier ran —
`Computing ForceAtlas2 (3D, ForceSim)` against `(3D, networkx)` — was in the capture and in
no file a reader could open.

`ref/<NAME>.json` now carries **`transcript`**: the reference's own stdout as a list of lines,
in fixture order, for every one of the 23 names. `FORCEATLAS2.json` reads:

```
Computing ForceAtlas2 (3D, ForceSim) for 77 nodes...
  Barnes-Hut not used below 400 nodes (n=77): the exact all-pairs repulsion is cheaper there, and exact
  Still moving 0.10k per node after 50 iterations, so this layout is not settled; raise Iterations
  ForceAtlas2 completed in 0.01s
```

which is `_forceatlas2_forcesim` (`forceatlas.py:124-147`) saying so itself.

## Commands

```
scripts/orch/gr cargo build --release -p graph-cli                                   -> 0
scripts/scigraphs-conformance.sh                       (before)                         -> 0
scripts/scigraphs-conformance.sh                       (re-pin; names FORCEATLAS2)      -> 1
scripts/scigraphs-conformance.sh                       (after the re-pin)              -> 0
scripts/scigraphs-conformance.sh --break                                               -> 1
scripts/orch/gr cargo test -p graph-core --lib forcesim                                -> 0  (34 passed)
scripts/orch/gr cargo test -p graph-core --lib rng::pcg64                              -> 0  (13 passed)
scripts/orch/gr cargo test --workspace --no-fail-fast                                  -> 0  (0 failed)
scripts/orch/gr cargo fmt --check                                                       -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings                  -> 0
scripts/orch/gr cargo run -q --release -p graph-cli -- capabilities --check             -> 0  (74 rows; no problem names forcesim)
scripts/orch/gr cargo run -q --release -p graph-cli -- codegen --check                  -> 0  (4 artefacts up to date)
scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8               -> 0  (PASS, 8/8 seeds)
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 … hashgate --seeds 8                    -> 1  (FAIL: 8 of 8 seeds diverge)
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-fa2-fixtures --seeds 1000  -> 0
docker run … ge-python-oracle python3 harness/oracle-fa2.py target/fa2-fixtures         -> 0  (1000 cases, worst 3.156e-8)
scripts/orch/gr cargo run -q --release -p graph-cli -- oracle-fa2                       -> 0  (worst 3.156e-8, ceiling 1e-7: ok)
```

**`oracle-fa2` did not move.** Its worst case is `3.1559272452132634e-8`, the figure
`sg-fa2-seed` recorded, and it is measured on `layout.forceatlas2` — which this job did not
touch. That is the whole reason the ForceSim port is a second layout id rather than a
re-pointing of the first.

## Deviations from the job's file list

Recorded because the list was optimistic about what a new registry entry costs. Every one of
them is mechanical and every one was forced by a test that counts registered layouts.

| file | why |
|---|---|
| `crates/graph-core/src/layout/force/mod.rs` | the `pub mod forcesim` + `pub use` the module needs |
| `crates/graph-core/src/rng.rs` | `mod pcg64;` and the re-export (the body allowed `rng/pcg64.rs` "if it would pass 300 lines"; `rng.rs` is 285 and PCG64 does not) |
| `crates/graph-cli/src/oracle_python/conformance.rs` | `FORCESIM_SEED`, which `overrides.rs` imports |
| `crates/graph-cli/src/capabilities/registry/unproven.rs` | `capabilities --check` refuses a `Gated` row with no `hash_4way` and no `oracle_diff` |
| `crates/graph-cli/src/capabilities/tests/registry/{routing,force}.rs` | the routing test restates `unproven.rs` per row |
| `crates/graph-cli/src/hashgate/tests/knob/coverage.rs` | every registered layout needs a per-stage control or a stated `Gap` |
| `crates/graph-cli/src/hashgate/tests/report.rs` | a golden JSON string keyed by every registered stage |
| `crates/graph-cli/src/snapshot_cmd/tests.rs` | `layout_names()` is asserted against a hand-written array in registry order |
| `crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs` | a golden `"snapshots": 200`, one short per registered layout |
| `docs/measurements/scigraphs-conformance.md` | sg-common step 6: this row's matrix line and its override count |

## Decisions taken

- **`params` is six fields, not the ten `_forceatlas2_forcesim` takes.** `strong_gravity`,
  `lin_log_mode`, `barnes_hut_optimize`, `barnes_hut_theta` and `edge_weight_influence` are
  fixed at their `forceatlas.py` defaults and documented instead of exposed: the last is inert
  without edge weights (`_build_edge_w` returns `None` outright, `simulation.py:278-284`) and
  the others select force laws this port does not implement. Exposing a field the layout
  ignores is the same defect as a field it half-implements. Reason: it keeps the layout inside
  the house line cap and inside the conformance row's question. Reversible by adding a force
  law, not by adding a field.
- **The scale ceiling is 2 000 and is marked as extrapolated.** A wrong-by-a-lot ceiling
  costs one slow run; a wrong-by-a-lot *unmarked* ceiling costs a reader's trust.
- **The `f32` position vectors in `forcesim/tests.rs` come from the transliteration, not from
  `ref/FORCEATLAS2.f64`,** and the test file says so at the top. Everything *upstream* of them
  — the start row, `k`, `repulsion`, `gravity` — compares equal to the reference exactly, and
  `G_FORCEATLAS2_SEED` is closed on that basis.
