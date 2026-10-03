# fix-force-quadtree: review repairs in `layout/force/quadtree*` and `barnes_hut/charge.rs` (2026-10-03)

Branch `fix-force-quadtree`. Source: `docs/reviews/review-layout-force.md`, ids `LF-02`, `LF-14`,
`LF-17`, `LF-22`, the quadtree half of `LF-26`, and the review's unverified item on
`barnes_hut/charge.rs:261` / `link.rs:84`. Paths are under `crates/graph-core/src/layout/force/`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| LF-02 | BLOCKER | fixed: `±inf` refused where `NaN` was (`bounds_of`, `Builder::add`, `flatten`), a "bounds stopped growing" bail in `cover`, and a "bounds stopped narrowing" bail in `insert_leaf` that chains the pair instead of splitting for ever. The build returns; the caller still gets `StageError` from `force::planar_points` / the post-run check, never a hang. Each bail carries a `Ponytail:` line. | `a_non_finite_coordinate_is_refused_and_the_build_returns` (parent) + `probe::a_non_finite_coordinate_makes_the_build_return` (child, run under `ulimit -v`) | `quadtree.rs:46` (`bounds_of`), `:124` (`build`), `:191` (`cover` bail); `quadtree/build.rs:81` (`insert_leaf` bail), `:28` (`add`), `:103` (`chain`); `quadtree/preorder.rs:56` (`flatten`); tests `quadtree/tests.rs:14`, `quadtree/tests/probe.rs:81` |
| LF-14 | MAJOR | fixed, doc half: `key(k)` returns the **shape-arena slot** the preorder was flattened from, not an insertion-order point id. Chose the rename (`node_id`) over storing the chain head, because storing the head would change the jiggle key `approx` hashes and therefore move every Barnes-Hut output — the one option the job forbids. The doc at `quadtree.rs:81` now says what the value is, and a test pins both halves: every id is a shape slot, and at least one is *not* a valid point index. | `a_cell_s_node_id_is_a_shape_slot_and_never_a_point_index` | `quadtree.rs:81-88` (`node_id`), `:64` (field `node_id`), `charge.rs:201` (`approx`'s only caller); test `quadtree/tests.rs:189` |
| LF-17 | MINOR | fixed: `open` is now `opening_threshold(w, theta, theta2)` — `theta == 0.0` returns `+inf` (open nothing, the documented exact-N² path), the squared ratio `(w/θ)²` is formed only where `w*w/θ²` came back `NaN`, and the value is bit-identical to the old form everywhere the old form was already finite. A non-finite `θ` opens nothing. The `centre` division named in the same row is **not** a defect: `m` is the sum of the children's `count` and an internal cell always holds a point, which `insert_leaf`'s bail preserves; a test now pins that invariant rather than a branch in the hot loop. | `an_opening_threshold_is_never_a_nan`, `the_opening_threshold_is_the_one_the_old_form_already_produced`, `a_zero_theta_opens_nothing_and_the_walk_stays_exact`, `a_cell_at_the_extreme_magnitude_still_gets_a_usable_threshold`, `every_internal_cell_has_a_child_so_the_centre_never_divides_by_zero` | `charge/threshold.rs:19` (`opening_threshold`), `:48` (`centre`); `charge.rs:102` (the call); tests `charge/tests.rs:53`, `:73`, `:89`, `:124`, `quadtree/tests.rs:224` |
| LF-22 | MINOR | fixed: `build` refuses `xs.len() != ys.len()` and a count past `u32::MAX` (`u32::try_from`), leaving an empty arena rather than indexing past `ys` or wrapping the index — the previous behaviour was `index out of bounds: the len is 1 but the index is 1`. The doc half: the claim that `quadtree.rs:219` and `bounds.rs:47` were under the parameter cap was **false as cited** — the doc named no lines, and both functions take 4 parameters *besides* the receiver. `split_step` is left at 4 and the two doc comments now state the counting rule instead of implying a number. | `build_refuses_a_length_mismatch_instead_of_indexing_past_ys` | `quadtree.rs:26` (`Points` doc), `:124` (the `try_from` refusal), `quadtree/build.rs:7` (`Builder` doc); test `quadtree/tests.rs:174` |
| LF-26 (quadtree half) | MINOR | fixed: `cover_grows_a_square_that_contains_every_point` now runs four point sets — the original small one, `±1e17`, `±1e300`, and a mixed set spanning eleven orders — and asserts the square is finite and that `order` is a permutation. `1e17` and `1e300` are what make it able to fail for LF-02's `cover` bail (the old set never overflowed `z`); the `inf` cases live in the probe test instead, because on the unrepaired tree they do not return. The other three LF-26 controls named in the row are not in this job's paths. | `cover_grows_a_square_that_contains_every_point` | `quadtree/tests.rs:154`, cases at `:144` |
| review "unverified" (`charge.rs:261`, `link.rs:84`) | unverified | **true, and it is a defect — but not one this job's paths can close.** `rng.rs:54-60` is `(h >> 11) as f64 * 2^-53 - 0.5) * 1e-6`, exactly representable throughout, so `jiggle` returns exactly `+0.0` iff `h >> 11 == 1 << 52` — one word in 2^53 per draw, reachable because `fmix64` is a bijection over `u64`. A brute-force scan of 1 638 400 draws over `seed < 400, tick < 8, pass < 8, ij < 64` found **0** zeros, so no witness key could be produced without inverting `fmix64`. Closed on this side instead: `settle` floors a zero `l` to `distanceMin²`, so a fully coincident pair contributes nothing rather than `±inf` then `NaN`. **`link.rs:84` is unchanged and still divides by an unguarded `l`** — `link.rs` is not in this job's paths; see *decisions needed*. | `a_fully_coincident_pair_at_zero_distance_yields_a_finite_zero_delta` | `charge.rs:262` (`settle`'s floor); test `charge/tests.rs:163`. Open half: `link.rs:84-87` |

## RED → GREEN (last lines, filtered runs)

All from `scripts/orch/gr cargo test -p graph-core --lib -- quadtree::tests charge::tests`.

**RED, before any fix** — `test result: FAILED. 18 passed; 5 failed; 1 ignored; 0 measured; 1229 filtered out; finished in 60.03s`:

```
---- charge::tests::a_cell_at_the_extreme_magnitude_still_gets_a_usable_threshold ----
  panicked at charge/tests.rs:133: cell 0: open is NaN
---- charge::tests::an_opening_threshold_is_never_a_nan ----
  panicked at charge/tests.rs:66: w = 0, theta = 0: open is NaN
---- quadtree::tests::a_cell_s_node_id_is_a_shape_slot_and_never_a_point_index ----
  panicked at quadtree/tests.rs:189: quadtree.rs documents key() as an insertion-order node id: [1, 2, 0, 3, 5, 4, 6]
---- quadtree::tests::a_non_finite_coordinate_is_refused_and_the_build_returns ----
  panicked at quadtree/tests.rs:217: ...a_non_finite_coordinate_makes_the_build_return did not return within 60 s
---- quadtree::tests::build_refuses_a_length_mismatch_instead_of_indexing_past_ys ----
  panicked at quadtree.rs:36:36: index out of bounds: the len is 1 but the index is 1
```

The `+inf` case is the BLOCKER's RED: the child process the probe spawns did not come back in
60 s and had to be killed. Two further REDs were observed while shaping the bails and are
recorded here because both were **my** error, not the review's: `cover`'s first bail
(`!(span > before)`) failed `x -1e17 in [-1e17, -1e17)` — a coordinate past 2^53 makes
`b.x0 + z` round back to `b.x0`, so the span stays 0 for the first ~57 doublings and the bail
fired on a `cover` that was still converging; and `opening_threshold`'s first form returned
`NaN` for `w = inf, theta = inf`, because `(w/θ)²` is `NaN` there too.

**GREEN** — `test result: ok. 24 passed; 0 failed; 1 ignored; 0 measured; 1229 filtered out; finished in 0.05s`.
The 1 ignored is the probe body, which the parent runs in a child by design.

## Negative controls

| control | expected | got |
|---|---|---|
| `charge::tests::the_opening_threshold_is_the_one_the_old_form_already_produced` | the new form is bit-identical to `w*w/θ²` on 5 θ values × 6 widths | passes; without it the LF-17 fix would move every Barnes-Hut output |
| `quadtree::tests::probe::a_non_finite_coordinate_makes_the_build_return` ran as a plain test on the unrepaired tree | must not hang the suite | the parent killed it at the 60 s deadline and failed — the row's own RED |
| LF-02's `inf` case under `ulimit -v 4000000` in a child | bounds the damage if a future bail regresses | child killed on the deadline, exit 101 surfaced in the parent's assertion |

## House limits after the repair

`quadtree.rs` 222, `quadtree/bounds.rs` 87, `quadtree/build.rs` 107, `quadtree/preorder.rs` 100,
`quadtree/tests.rs` 254, `quadtree/tests/probe.rs` 121, `charge.rs` 268, `charge/threshold.rs` 60,
`charge/tests.rs` 201 — all under 300. `insert_leaf` is 39 lines with 4 parameters besides the
receiver; `opening_threshold` 4, `centre` 3. Three new files were needed for the caps
(`quadtree/build.rs`, `quadtree/tests/probe.rs`, `charge/threshold.rs`), each following a split the
module already used; reported as a deviation.

## Determinism and unchanged outputs

Barnes-Hut and FA2 output does not move for finite input, and this is enforced rather than
asserted: `opening_threshold` returns the *same bits* as `w * w / θ²` unless that came back `NaN`
(`charge/tests.rs:73` pins it over 5 θ × 6 widths), the `bounds_of` filter change is `is_nan` →
`is_finite`, which excludes exactly the inputs that used to hang, and `node_id` is a rename — the
value the jiggle hashes is untouched, so `approx` still hashes the same key.

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.04s`` |
| `scripts/orch/gr cargo test -p graph-core --lib -- quadtree::tests charge::tests` | 0 | `24 passed; 0 failed; 1 ignored; 1229 filtered out` |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.44s`` |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `PASS` (`4-way equal on 8/8 seeds`) |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (expected non-zero) | `FAIL: 8 of 8 seeds diverge` |

`scripts/orch/gr cargo test --workspace --no-fail-fast` → **1**, and both failures are outside this
job's paths and pre-existing:

1. `layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them`
   — `oracle_probe.rs:58` panics because `target/probe/rank1000.txt` is absent. The module's own
   doc says a missing measurement file should skip, but the `panic!` at `:58` is a hard abort. The
   file is a probe artefact, not in git. 1244 passed, 1 failed.
2. Two `self-check FAILED: the settle is 112 ticks` lines printed by the `force` self-check inside a
   test binary that still reported `ok` — a self-check that prints and does not gate.

## `scripts/scigraphs-conformance.sh` → exit 1, and it is not this repair

30 of 32 rows pass, every row a repair could have moved included (`FORCEATLAS2`,
`IGRAPH_DRL`, `IGRAPH_DRL_2D`, `IGRAPH_LGL`, `IGRAPH_GRAPHOPT`, `IGRAPH_DH`, `IGRAPH_FR`,
`YIFAN_HU`'s motor half). The two failures are `YIFAN_HU` and `GRAPHVIZ_SFDP`, and both say the
same thing:

```
YIFAN_HU:      FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72)
GRAPHVIZ_SFDP: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72)
```

That is the **`ref/` half**, hashed by `verdict.rs:156` against `baseline/table/graphviz.rs:51`,
whose pin is `4c90e6c3…` — a different digest for the *reference* coordinates, produced by the
Graphviz arm inside `ge-graphviz-oracle`. No file in this job's diff is read by that arm, and the
two rows share one `sfdp` reference by construction (`scigraphs-conformance.md:203`), which is why
they report the same sha. The proposed baseline
(`target/scigraphs-conformance/conformance-baseline-proposed.rs`) shows the **motor** shas for both
rows unchanged from the pin (`GRAPHVIZ_SFDP` motor `61d65b07…` in both), so the motor half did not
move. Per `fix-common.md`, a moving row is a regression to report, never a row to re-pin from a fix
job: **left for the orchestrator**, re-pin or fix in the oracle arm, not here.

## Decisions taken

- **`node_id` renamed rather than the chain head stored** (LF-14). The head is the review's first
  suggestion, but `approx` hashes `key(k)` as the jiggle pair with the query index, so storing a
  point id would re-key every Barnes-Hut nudge and move every output — forbidden by this job. The
  rename has exactly one caller, in this job's paths.
- **`build` refuses by returning, not by erroring** (LF-22, LF-02). `build` returns `()` and has
  three callers across two other modules (`barnes_hut/collide.rs:59`,
  `forceatlas2/state/barnes_hut.rs:95`), neither in this job's paths; changing the signature would
  be a breaking change to a `pub(crate)` API other modules use. An empty arena is a tree where
  every walk contributes nothing — bounded and silent, against a panic or a wrapped index.
- **`rng.rs` left untouched.** The job lists it read-only, so the `jiggle`-can-be-zero finding is
  closed on the `charge.rs` side (`settle`'s floor) rather than at its source.
- **The `centre` division left as it is** (LF-17's second half). Adding a guard would put a branch
  in the innermost loop of every tick for an invariant the tree already guarantees; the invariant
  is now pinned by a test instead.

## Deviations

Three files created outside the paths the body listed, each required by the 300-line cap and each
following a split the same module already used: `quadtree/build.rs` (the `Builder` half, as
`bounds.rs`/`preorder.rs` already are), `quadtree/tests/probe.rs` (the capped-child probe, as
`charge/tests.rs` already is), `barnes_hut/charge/threshold.rs` (the two numeric helpers, as
`barnes_hut/step.rs` already is). No other path was touched.
