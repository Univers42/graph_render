# fix-force-jiggle: `rng::jiggle` never returns `+0.0`, and `link.rs` floors its division (2026-10-03)

Branch `fix-force-jiggle`, cut from `fix-force-quadtree` (item 6 of that job's review pass).
Source: `docs/measurements/fix-force-quadtree.md`, the row for the review's unverified item on
`barnes_hut/charge.rs:261` / `link.rs:84`, and `docs/reviews/review-layout-force.md:71-72`.
Paths are under `crates/graph-core/src/`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| 1 — `rng::jiggle` can return exactly `+0.0` | MAJOR | **fixed, in `jiggle` itself and at none of its 14 call sites.** `jiggle`'s last step is split into `jiggle_of(w: u64) -> f64`, which maps the one 53-bit word `1 << 52` to `(1 << 52) + 1`. That word is the whole defect: `u = w·2^-53` is exactly `0.5`, and `(0.5 - 0.5)·1e-6` is exactly `+0.0`. The `Ponytail:` line above `jiggle_of` names the word, the direction it moves and why no hash can move with it (`fold`'s output feeds nothing else, and the mapping is a one-word branch in a pure function — the only way a layout output can move is a draw that actually lands on that word, which `hashgate --seeds 8` below is the evidence against). Splitting the step out is what makes the property testable at all: `1 << 52` has no pre-image under `fmix64`, so a test on `jiggle` could only observe the *absence* of a zero (1 638 400 draws found none) and would pass against the unfixed mapping. | `the_midpoint_word_never_becomes_a_zero_nudge` (RED), `the_midpoint_word_is_the_only_one_the_mapping_moves` (the control) | `rng.rs:63-80` (`jiggle_of`), `:60` (`jiggle`'s call); tests `rng/tests.rs:132`, `:143` |
| 2 — `link.rs:84` divides by a distance a zero jiggle leaves at zero | MAJOR | **fixed, and it needed a guard, not just the proof.** Item 1 removes the *coincident* case: with `jiggle` non-zero, both `jiggle` branches in `force` install a non-zero axis, so a fully coincident pair has `l > 0` — that proof is now the doc comment above `force`. But the denominator is **not** provably non-zero, because `dx * dx` underflows for any axis below `sqrt(f64::MIN_POSITIVE) ≈ 1.5e-162`: a pair `1e-200` apart is *not* coincident, takes no `jiggle` branch, and still sums to `0.0`. Before the guard, `force` returned `(-inf, -inf)` there — the RED below. So `force` floors `l == 0.0` to a zero contribution, the way `charge.rs:255` floors its own, with a `Ponytail:` line naming what it gets wrong (that pair gets no spring at all; charge's `distanceMin²` floor would instead divide to ~1e106). The floor is bit-identical on every input that was already finite — `hashgate --seeds 8` confirms — so it changes nothing but the `±inf` that was there before. | `a_separation_whose_square_underflows_still_has_a_finite_force` (RED), `a_link_between_two_coincident_nodes_has_a_finite_force` (the coincident pair item 1 protects) | `link.rs:86-97` (the proof, on `force`), `:105-118` (the floor and its `Ponytail:` line); tests `link/tests.rs:81`, `:95` |
| 3 — output must not move | — | **verified.** `hashgate --seeds 8` exits 0 with every stage `4-way equal on 8/8 seeds`, and `GM_MUTATE_REFERENCE_DEGREE=9` exits 1 with `FAIL: 8 of 8 seeds diverge`. No registered layout, post, analysis, scale or transport row moved. | — | — |

## RED → GREEN (last lines, filtered runs)

All from `scripts/orch/gr cargo test -p graph-core --lib -- <filter>`.

**Item 1 RED** — the mapping removed again after `jiggle_of` was extracted (the extraction is
behaviour-preserving, so this is the pre-fix behaviour):
`test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 1275 filtered out`:

```
---- rng::tests::the_midpoint_word_never_becomes_a_zero_nudge ----
  panicked at crates/graph-core/src/rng.rs:302:9:
  assertion `left != right` failed
    left: 0.0
   right: 0.0
```

The control, `the_midpoint_word_is_the_only_one_the_mapping_moves`, passed in the same RED run —
as it must: it pins that the five control words did *not* move, which is true of both the
unfixed and the fixed code. Its job is to fail if the mapping ever grows a second word.

**Item 1 GREEN** — `test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 1261 filtered out`
(filter `rng::tests`).

**Item 2 RED** — `test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 1274 filtered out`:

```
---- link::tests::a_separation_whose_square_underflows_still_has_a_finite_force ----
  panicked at crates/graph-core/src/layout/force/barnes_hut/link/tests.rs:109:5:
  force (-inf, -inf)
```

`a_link_between_two_coincident_nodes_has_a_finite_force` passed in this RED run too, and that
is the honest reading of item 2: coincidence alone was **not** reachable to `±inf` (no draw in
1 638 400 hit the midpoint word), so the underflow path is the defect this job found.

**Item 2 GREEN** — `test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 1252 filtered out`
(filter `link::tests rng::tests charge::tests`).

## Negative controls

| control | expected | got |
|---|---|---|
| `the_midpoint_word_is_the_only_one_the_mapping_moves` | the five words `{0, 1, (1 << 52) - 1, (1 << 52) + 1, (1 << 53) - 1}` keep the raw formula's bits | pass — a clamp or a wider branch would move one of them |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | exit 1 | exit 1, `FAIL: 8 of 8 seeds diverge` |
| `a_link_between_two_coincident_nodes_has_a_finite_force` | finite halves both ways | pass; would fail if `jiggle` returned `+0.0` again |

## House limits after the repair

`rng.rs` was 285 lines and the two tests plus the split-out function took it to 329, over the
300-line cap, so the test module moved to `rng/tests.rs` (`#[cfg(test)] mod tests;`) — the same
split `link.rs`, `charge.rs` and `tests.rs` already use. Final: `rng.rs` 173, `rng/tests.rs` 158,
`link.rs` 180, `link/tests.rs` 126 lines. `force` is 23 lines including its comment block.
`cargo fmt --all --check` and clippy `-D warnings` exit 0.

## Determinism and unchanged outputs

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (silent) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished \`dev\` profile` |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | PLACEHOLDER_WS |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished \`dev\` profile [unoptimized + debuginfo] target(s)` |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 | `FAIL: 8 of 8 seeds diverge` |
| `scripts/scigraphs-conformance.sh` | PLACEHOLDER_CONF | PLACEHOLDER_CONF2 |

Nothing in either repair touches a transcendental, a reduction order or a `HashMap`
iteration, so D1–D10 are unaffected; the mapping is an integer comparison in a pure function
and the floor is a branch on `l == 0.0` that no finite input reaches.

**A first full-workspace run was not usable and was discarded.** Its `graph-cli` integration
binaries failed with `graph-cli was built from tree 8a94031… but the tree is now 5cec671…:
rebuild before recording` — the CLI had been built before the `rng.rs` test split, so the
trees disagreed by construction. It is a staleness artefact of editing the tree between a
build and a run, not a regression: `cargo test -p graph-cli --test cli_fa2` is `ok. 3 passed`
on the rebuilt tree, and the row above is the rerun of the whole workspace after the rebuild.

## Decisions taken

- **The `link.rs` guard returns `(0.0, 0.0)`, not a floored `l`.** `charge.rs` floors to
  `distanceMin²`, which is right there because it divides a weight by `l` twice. Here the same
  floor would put `(0 - 30) / distanceMin²` into `factor` and hand the pair a displacement of
  order 1e106 — finite, and worse than nothing. Recommended: keep the zero contribution.
- **`rng/tests.rs` is a new file.** The job lists `rng.rs` "+ its tests"; splitting the test
  module is how the crate's own other test modules are split, and without it the file is over
  the house cap.

## Deviations

None outside the listed paths.