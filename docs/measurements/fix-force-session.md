# fix-force-session — LF-13 (the `Tier` fold and the file-length cap), LF-18 (the cross-field relations)

Source: `docs/reviews/review-layout-force.md`, ids `LF-13` and `LF-18`, plus the review's
unverified item on `live_params.rs` `validate_finite`. Brief:
`prompts/jobs/fix-force-session.md` over `prompts/jobs/fix-common.md`. Reviewed at `74f8994`;
the tree measured here is `b83d3d9` plus this branch. Branch `fix-force-session`, 2026-10-03.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| LF-13 | MAJOR | fixed (the parameter half; the file-length half was already satisfied — see below) | `layout::force::session::tests::m1a::the_default_session_reproduces_the_frozen_layout_on_every_seed`, `tests::m1b::the_tick_count_is_the_only_thing_that_depends_on_how_the_ticks_are_chunked`, `tests::frozen::*` (59 → 66 tests in the module) | `layout/force/session.rs:171-186` (`step_under`), `:277-285` (`tier`), `layout/force/barnes_hut.rs:176-181`, `layout/force/particle_mesh.rs:96-101` |
| LF-18 | MINOR | fixed | `layout::force::session::live_params::tests::a_distance_shell_inside_out_is_refused`, `::a_session_born_below_its_own_settle_threshold_is_refused`, `::a_field_outside_its_range_is_named_before_its_pair_is`, `::the_distance_relation_is_strict_and_the_alpha_relation_is_not` | `layout/force/session/live_params.rs:110-133` (`relations`), `:83-89` (`validate`) |
| unverified: does a live setter reach `validate_finite`? | — | **false** — the frozen path is unreachable from any live setter; the test stays and pins it | `layout::force::session::tests::frozen_path::no_live_setter_inherits_the_frozen_paths_weaker_acceptance`, `::the_frozen_path_takes_what_the_live_ranges_refuse`, `::the_frozen_parameter_set_is_in_range_for_both_relations` | `layout/force/session/live_params.rs:135-143` (`validate_finite`), `session.rs:124-128` (`from_frozen`) |

## Notes per row

- **LF-13, first half — the file-length claim is stale, the parameter claim was real.**
  `wc -l layout/force/session.rs` is **273** on this tree, not 316: `perf-p4-carry` split
  `carry.rs` out of it, and the module doc's own `mod` list (`session.rs:46-54`) already names
  six child files. The ≤300 half of the finding was therefore **already satisfied** and no
  change was owed there. The second half was not: `step_under` took
  `(&mut self, runner, workers, split, ticks)` — 5, against the ≤4 cap — and the reviewer's
  proposed fix (fold `workers`+`split` into one `Tier`) had not been applied.

  `step_under` is now `(&mut self, tier: Tier<'_, R>, ticks: u32)` — **3** (was 5). It reuses
  `barnes_hut::settle::Tier` (`barnes_hut/settle.rs:28-35`, re-exported `barnes_hut.rs:95`)
  rather than declaring a second identical struct: its doc already says it is "`sim::How`'s
  three fields … lifted to the level of a whole settle", its `Clone`/`Copy` are hand-written
  so no `R: Clone` bound leaks onto it, and `yifan_hu.rs:97` already builds one. A new type
  here would have been a fourth spelling of the same value.

  `step_with` keeps its `(runner, workers, ticks)` signature — it is `pub`, called from
  `graph-wasm/src/session.rs:130` and `graph-cli/src/bench/tick.rs:151`, and the wire and the
  SDK do not move. Its control-free half, and `step`'s, both go through a private
  `tier(runner, workers)` helper (`session.rs:277-285`) so "`Split::None`" is one value in one
  place instead of typed at each of the two call sites.

  **Two files outside this job's listed paths were touched**, both a four-line binding plus
  one import: `layout/force/barnes_hut.rs:176-181` and `layout/force/particle_mesh.rs:96-101`.
  Folding two parameters into one value cannot be done without changing the two call sites of
  the function being folded, and there is no in-path way to reach them. No behaviour changed:
  both still pass their own `(runner, workers, split)` into the same three values, in the same
  order, into the same `How`. This is a deviation to review, not a silent edit.

  Behaviour-preserving, and measured as such: the module's own suite before and after, the
  65 golden digests (`session/tests/golden.rs`) and the 4-way hash gate are unchanged. The
  session suite went **59 → 66** tests, every one of the original 59 still present and green;
  the seven added are the pin/release cases moved out of `verbs.rs` into `pins.rs` (no
  assertion changed) and LF-18's new coverage.

  ```
  before: test layout::force::session::tests::m1a::the_default_session_reproduces_the_frozen_layout_on_every_seed ... ok
          test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 1203 filtered out; finished in 1.17s
  after:  test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 1203 filtered out; finished in 1.11s
  ```

- **`verbs.rs` was over the cap too, and the job's done-when names every file.** The review
  measured `layout/force/session.rs` alone; `session/tests/verbs.rs` was **313** lines,
  which the done-when ("`wc -l` of every file under `layout/force/session*` is at most 300")
  forbids. It is now 148. The split is on the existing boundary and not a compression: the
  pin/release family moved to `session/tests/pins.rs` (100 lines) — pins place a row rather
  than arming a force, so they are read against `sim.rs`'s integrate tail, not against a
  parameter — and the two-run `Twin` harness moved to `support.rs:53-127`, which is the file
  the module already documents as "what they share". No assertion was weakened, merged or
  deleted in either move. `session/tests/carry.rs` is at exactly 300 and left alone.

  Final `wc -l`, the largest of the 25 files under `layout/force/session*`:
  `tests/carry.rs` 300, `session.rs` 283, `tests/m1e.rs` 247, `tests/live.rs` 239,
  `session/carry.rs` 216, `live_params/tests.rs` 204, `live_params.rs` 190,
  `live_params/ranges.rs` 168.

- **LF-18 — the two relations, and where they live.** `validate` is now the loop over
  `ordered()` (which it duplicated by hand before) followed by `relations()`
  (`live_params.rs:110-133`). Two points on the reviewer's proposal:

  1. **"inside `ordered()`" is taken as "on the same walk".** `ordered()` returns
     `[(&Range, f64); 13]` — a field and its range — and a relation between two fields is
     not one of those pairs, so it cannot literally live there. What the review's stated
     reason requires is preserved and is now *stronger* than before: `validate` and
     `validate_finite` both walk `ordered()`, so the "one order for both" claim at
     `:245-247` is no longer a claim about two parallel lists but about one. `relations()`
     runs **after** the per-field loop, so a value that is wrong on its own is still named as
     the field that is wrong — `a_field_outside_its_range_is_named_before_its_pair_is` pins
     exactly that ordering.

  2. **The distance relation is strict (`>=` refused), which is the review's own wording and
     slightly more than the defect needs.** The review's failing input is
     `distance_min = 1000, distance_max = 1`; its proposed fix says "`min < max`". Those
     differ on the single point `distance_min == distance_max`, a shell of **zero width** —
     and a zero-width shell is the *same* no-repulsion picture as an inside-out one, not a
     different one. `charge = 0.0` is the honest spelling, so the equality is refused too,
     with the trade recorded as a `Ponytail:` on `relations()` (failing input: a caller who
     meant "no repulsion" and wrote an empty shell; direction: over-refusing; escape hatch:
     `charge = 0`). `the_distance_relation_is_strict_and_the_alpha_relation_is_not` pins both
     sides of both lines so the choice cannot drift silently.

  The error is `SessionError::OutOfRange` — **no new variant**. The refusal names one field
  and one `&'static str` rule that already carries both field names and the direction, which
  is precisely what `OutOfRange` is for and what `StageError::Param { name, rule }` can carry
  without allocating. A new variant would have been additive on the wire but would have
  widened a public enum (`SessionError` is `pub` and `graph-wasm` folds all of it into the
  single `Code::SessionRefused = 17`, `errors.rs:70-76`) for no gain in what the caller reads.

  RED, before the fix, filtered by test name:

  ```
  test layout::force::session::live_params::tests::a_distance_shell_inside_out_is_refused ... FAILED
  thread '...' panicked at crates/graph-core/src/layout/force/session/live_params/tests.rs:103:9:
  assertion `left == right` failed: distance_min = 1000 with distance_max = 1 is a shell no pair is inside
    left: Ok(())
   right: Err(OutOfRange { field: "distance_min", rule: "distance_min: finite, 0..=1000, and < distance_max" })
  test layout::force::session::live_params::tests::a_session_born_below_its_own_settle_threshold_is_refused ... FAILED
  thread '...' panicked at .../live_params/tests.rs:128:9:
  assertion `left == right` failed: initial_alpha = 0 with alpha_min = 1 is born settled
    left: Ok(())
   right: Err(OutOfRange { field: "initial_alpha", rule: "initial_alpha: finite, 0..=1, and >= alpha_min" })
  test layout::force::session::live_params::tests::the_distance_relation_is_strict_and_the_alpha_relation_is_not ... FAILED
  panicked at .../live_params/tests.rs:192:5: zero wide is refused
  test result: FAILED. 4 passed; 3 failed; 0 ignored; 0 measured; 1259 filtered out
  ```

  GREEN:

  ```
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 1259 filtered out; finished in 0.00s
  ```

  **One existing test had to change, and it is the defect rather than the fix.**
  `m1e::the_ends_of_every_range_are_in_range` asserted that `distance_min = 1000.0` — the end
  of its own range — is accepted beside the default `distance_max = 520.0`. That *is* LF-18:
  the test was pinning the defect as though it were the contract. The per-field ends are still
  pinned, and now with the coupled field's partner moved to the far end of its own range
  (`m1e.rs:198-221`, `ends`), because `distance_min`'s range ends at 1000 and
  `distance_max`'s begins at 1 — the ends genuinely overlap and only a compatible partner
  reaches them. The weaker assertion (a field's end is in range) was kept and the
  contradictory one corrected; no case was deleted.

- **`live_params.rs` had to be split to take the fix.** It was 293 lines and `relations()` is
  24 lines with its docs, so the finding could not be fixed in place without breaching the cap the
  same job enforces. The split is on the same boundary the review names as the precedent
  (`session/live_params.rs`'s own `tests` child): the `Range` type and the 16 range constants
  moved to `live_params/ranges.rs` (168 lines), leaving the struct, the relations and the
  validator in `live_params.rs` (190). `ALPHA`/`ALPHA_TARGET` are `pub(in
  crate::layout::force::session)` and re-exported, so `session.rs:60` and the two cooling
  setters are untouched. No published path moved.

- **The unverified item — `false`, with the test kept.** The review could not determine
  whether a live setter routes through `validate_finite` (`live_params.rs:238` at review time,
  `:135-143` now), which would break the "refuse, never clamp" promise at the module header.
  **It does not.** `validate_finite` has exactly one caller in the tree,
  `ForceSession::from_frozen` (`session.rs:126`); `from_frozen`'s only in-tree feeders are
  `BarnesHut::run_under`/`ParticleMesh::run_under` and the batch arms, which pass
  `ForceParams::default()`. Every live entry point goes through `validate` instead:
  `new`/`set_params` (`session.rs:115`, `:210`), `from_positions` (`warm.rs:19`, via `new`),
  and `reheat`/`set_alpha_target`, which hold their own single-field ranges
  (`ALPHA.check`, `ALPHA_TARGET.check_open`). `carry` (`carry.rs:73`) validates nothing
  because it reuses the already-validated `sim.params` by design.

  `tests/frozen_path.rs` is the test the unverified item was missing, and it is written to
  fail if the property ever changes rather than to describe it: it offers `theta = 2.0` — a
  value the frozen path *takes* — to `new`, `set_params`, `from_positions`, `reheat` and
  `set_alpha_target`, and requires each to refuse it by name and change nothing. Negative
  control, run and pasted: with `set_params` temporarily rewired from `validate()` to
  `validate_finite()`, the test goes red exactly where it should and nowhere else —

  ```
  test layout::force::session::tests::frozen_path::no_live_setter_inherits_the_frozen_paths_weaker_acceptance ... FAILED
  thread '...' panicked at crates/graph-core/src/layout/force/session/tests/frozen_path.rs:88:5:
  assertion `left == right` failed: set_params
    left: Ok(())
  test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 1266 filtered out
  ```

  and the other two stayed green, which is what says the failure is the setter and not the
  fixture. The file's third test also records that the frozen default set satisfies *both*
  relations (`distance_min` 1.0 < `distance_max` 520.0, `initial_alpha` 1.0 ≥ `alpha_min`
  0.001), so the acceptance split is a difference in strictness and not in kind: nothing the
  frozen layout runs is something a live session now refuses.

- **What did not move.** No layout output changed. `layout.force.barnes_hut`,
  `layout.force.particle_mesh` and `layout.force.yifan_hu` reach the tick through the same
  `Tier` values in the same order; `LiveParams::default()` is `ForceParams::default()` field
  for field and satisfies both relations, so the frozen stage builds and hashes identically.
  The wasm parameter wire is unchanged — thirteen `f64`s in declaration order, `LEN` and
  `decode` untouched (`graph-wasm/src/session/params.rs`) — and `set_params` over the ABI now
  refuses two *additional* pairs of values, all of them previously producing a wrong picture
  rather than a refusal. That is the finding.

## Commands and last lines

| command | exit | last lines |
|---|---|---|
| `scripts/orch/gr cargo test -p graph-core --lib layout::force::session` (before) | 0 | `59 passed; 0 failed; ... finished in 1.17s` |
| `scripts/orch/gr cargo test -p graph-core --lib live_params` (RED) | 101 | `4 passed; 3 failed; 0 ignored; 0 measured; 1259 filtered out` |
| `scripts/orch/gr cargo test -p graph-core --lib live_params` (GREEN) | 0 | `7 passed; 0 failed; 0 ignored; 0 measured; 1259 filtered out; finished in 0.00s` |
| `scripts/orch/gr cargo test -p graph-core --lib layout::force::session` (after) | 0 | `66 passed; 0 failed; 0 ignored; 0 measured; 1203 filtered out; finished in 1.11s` |
| `scripts/orch/gr cargo fmt --all --check` | 0 | no output |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | no `error` line |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 20 `test result: ok` suites, 0 `FAILED` |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished dev profile ... in 4.85s` |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `4-way equal on 8/8 seeds` / `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 | `FAIL: 8 of 8 seeds diverge` |
| `scripts/scigraphs-conformance.sh` | 0 | `PASS` — every row `ok`, none re-pinned: `IGRAPH_DH` 7.667e-1, `IGRAPH_GRAPHOPT` 5.571e-1, `YIFAN_HU` 8.292e-1, `SUGIYAMA` 1.259e-16, `CIRCULAR_HIERARCHY` 4.320e-16, `GRAPHVIZ_TWOPI` 2.040e-10 |

## Decisions taken

1. **`distance_min == distance_max` is refused** (`>=`, not `>`), matching the review's
   proposed "min < max" rather than its narrower failing input. A zero-width shell is the
   same no-repulsion picture as an inside-out one, `charge = 0.0` is the honest spelling, and
   the trade is recorded as a `Ponytail:` on `relations()`. The strictness is the reviewer's
   wording, and the Ponytail is the house rule for choosing the safe direction on a heuristic.
2. **`SessionError::OutOfRange` reused** rather than a new variant. The rule is a
   `&'static str` that already names both fields and the direction, so nothing is lost to
   `StageError::Param`, and a public enum does not grow for a refusal the caller reads as
   text.
3. **`Tier` reused from `barnes_hut::settle`**, per the review's own proposed fix and the
   module's existing doc, rather than a session-local struct of the same three fields.
4. **`session.rs`'s length needed no work** (273 lines on arrival, already under the cap);
   `session/tests/verbs.rs` did, and was split. Recorded so the next reader does not re-look
   for a 316-line file that is not there.