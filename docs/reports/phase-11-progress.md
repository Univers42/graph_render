# Phase 11 progress — compute tiers

Branch `p11`. This is the **reconcile** slice: the work two agents raced into this worktree
(left uncommitted) was reviewed hunk by hunk, fixed, dropped or kept, and **every number in
the phase's documents was re-derived by a command run in this slice** or removed. It is a
progress report, not the phase's §12 report.

## What this slice found, per file

| file | verdict | what |
|---|---|---|
| `barnes_hut/{link,collide}.rs` | **kept, fixed** | the gather ports are right; `halves` called `displaced` twice per edge (same value, twice the work) — now once |
| `barnes_hut/step.rs` | kept | three `StepRange` kernels, one per gathered pass |
| `barnes_hut.rs` | **fixed** | `THREADED_PASSES` claimed "the tick's own order" while listing `charge, link, collide`; the tick runs link first, so the list is now `link, charge, collide` and the doc says what is tested (the count) and what is stated (the order) |
| `barnes_hut/sim.rs` | kept | `dvx`/`dvy` gone; one scratch buffer for all three passes |
| `barnes_hut/link.rs` | **fixed** | see `halves` above; `scatter` kept as the serial reference the kernel is compared against |
| `layout/force/quadtree.rs` | kept | `visit` (the `&mut self` walk) deleted once nothing used it; `visit_in` is the only walk, and it is `&self` |
| `exec/select.rs` + tests | kept, with the numbers re-derived | `threads_nodes = 10_000`, `threads_max = 7` rest on the sweep below |
| `graph-cli/src/bench/tiers*` | kept | the sweep, its report half and its tests |
| `graph-cli/src/hashgate/knob.rs` | kept, extended | `GM_MUTATE_SPLIT_SUM` now names the pass, and the gate **refuses** a run the control cannot move |
| `docs/measurements/phase11-threads.md` | **regenerated** | the sweep writes it; nothing in it is hand-typed |
| `docs/decisions/tier-thresholds.md` | **rewritten** | every number re-derived below; the four-run and charge-only claims are gone |
| `docs/decisions/link-gather.md` | kept, corrected | one claim updated: the collide control now *refuses* below five seeds rather than passing vacuously |
| `docs/reports/phase-11-progress.md` | **rewritten** | this file |

## The measurement, and the row it justifies

<!--BENCH-TABLE-->
| n | scalar (ms) | threads 2 | threads 4 | threads 7 |
|---:|---:|---:|---:|---:|
| 220 | 17.85 | 0.75x | 0.66x | 0.52x |
| 10 000 | 2 272.25 | 1.34x | 2.14x | 2.97x |
| 100 000 | 50 866.71 | 1.71x | 2.92x | 4.29x |

(16 cores, load 4.83 → 7.10 across the run; medians of 5; the full table with every
individual run is `docs/measurements/phase11-threads.md`.)

The sweep times the **whole Barnes-Hut stage**, because a threshold selects a tier for a
layout run and a per-pass number would measure something no host runs. Every arm is compared
with **the serial arm's bytes from the same run**, so "faster" and "equal" are one check; the
run above reports `equal to scalar: true` for every arm, and the comparison is a **second
pass, after every arm has been timed**, so the order the tiers were asked in cannot decide it
(`the_order_the_tiers_were_asked_in_cannot_decide_whether_a_cell_is_compared`).

**Threads lose at n = 220 (0.52x–0.75x) and win from n = 10 000**, so the crossover is
*bracketed* between the two measured sizes rather than located, and `tier-thresholds.md`
carries `threads_nodes = 10_000` — the first measured winning size, the only claim the data
supports.

**Claims this slice removed, because nothing in this slice reproduced them.** The previous
draft of the report and the thresholds file carried: a *four-run* sample quoting 3.82x, 4.12x
and 5.19x with serial medians between 37.1 s and 55.9 s; a *charge-only* "Amdahl" table
(1.00x/1.05x/0.92x at n = 220, 1.80x at n = 100 000) taken before `collide` and `link` were
kernels; and per-run load averages. Those are measurements of a **code state that no longer
exists** — the charge-only arm was measured before two of the three passes were threaded, and
no command in this slice can reproduce it. Keeping them would be keeping a number nobody ran.

**What two runs of one command in this slice do say.** The sweep was run twice: the run
committed above (load 4.83 → 7.10) and a second one in this slice (load 1.55 → 3.78; serial
median at n = 100 000 of 44.9 s, seven-worker arm 4.54x). The ratio moved and the serial
median moved; the *sign* at every measured size did not move at all. That is the surviving
claim, and it is a claim about a threshold rather than a speedup.

## Determinism: the N-way gate and every negative control

`hashgate --tiers all` runs **ten arms** — native ×2, wasm32 ×2, a scalar reference and one
Barnes-Hut arm per worker count in {1, 2, 3, 4, 7} — and all three gathered passes go through
the runner in each, so it covers the whole partitioned tick.

| row | result |
|---|---|
| `hashgate --seeds 8` (4-way) | **0** — 4-way equal on 8/8, all 12 stages |
| `hashgate --seeds 8 --tiers all` (10-way) | **0** — 10-way equal on 8/8, all 12 stages |

**The threads tier is bit-identical to the scalar tier**, and `layout.force.barnes_hut`'s
bytes are unchanged: the honest ten-arm digest is the same value the 4-way run reported
(`e7436052…d4a0`), and the wasm32 arms agree with it.

### Every control goes red

Each gathered pass has its **own** control, because a knob that could only corrupt one kernel
would leave the other two resting on nothing. Run at `--seeds 8 --tiers all`:

| control | result |
|---|---|
| `GM_MUTATE_SPLIT_SUM=charge` | **1** — 0/8 seeds equal |
| `GM_MUTATE_SPLIT_SUM=collide` | **1** — 5/8 seeds equal (diverged at seeds 4, 6, 7) |
| `GM_MUTATE_SPLIT_SUM=link` | **1** — 0/8 seeds equal |
| `GM_MUTATE_SPLIT_SUM=1` | **1** — 0/8 seeds equal |
| `GM_MUTATE_REFERENCE_DEGREE=9` | **1** — 8/8 seeds diverge (the pre-existing control) |

### A control that cannot bite now refuses the run

Collide's control moves nothing below five seeds — at two or three nodes link and many-body
have already pushed the pair past `2 * collideRadius`, so collide's deltas are all `0.0` and
no split of its merge can change a byte. The `5/8` above pins it directly: the first seed
that diverges is **seed 4**. A row at two seeds would corrupt nothing and exit **0**, a
*vacuous pass*, which is worse than a red one because it reads as evidence.

`Split::min_seeds()` (graph-core, the model that measured it) and
`hashgate.rs`'s `refuse_a_vacuous_control` turn that into a refusal:

| row | result |
|---|---|
| `GM_MUTATE_SPLIT_SUM=collide hashgate --seeds 2 --tiers all` | **2** — "could not run" |
| `GM_MUTATE_SPLIT_SUM=collide hashgate --seeds 4 --tiers all` | **2** — "could not run" |
| `GM_MUTATE_SPLIT_SUM=collide hashgate --seeds 5 --tiers all` | **1** — 4/5 seeds equal |
| `GM_MUTATE_SPLIT_SUM=collide hashgate --seeds 8 --tiers all` | **1** — 5/8 seeds equal |

The test was written first and seen **RED** (with the floor forced to 1, it fails at
`knob.rs:146`), then the fix, then green.

## The forbidden-constructs row, scoped to product code

The phase prompt's row was a bare `grep -rnE "mul_add|relaxed|rayon|std::thread|Instant::now"
crates/graph-core/src`, and it **fails on a clean tree** — for three reasons, none of them a
violation:

1. **doc comments**, which name the forbidden constructs *in order to forbid them*
   (`exec/partition.rs`: "the executors live with their hosts — `std::thread::scope` in
   graph-cli"; `styles.rs`: "never `std`; no `mul_add`");
2. **`#[cfg(test)]` modules** — `post/routed/measure.rs` is an `#[ignore]`d benchmark whose
   whole purpose is to call `Instant::now()`;
3. **test code** — `linalg/lobpcg/tests.rs`.

`scripts/forbidden-constructs.sh` replaces the row. It resolves every `#[cfg(test)] mod NAME`
to the file Rust would load it from, skips those plus `tests.rs` and `tests/` files, and drops
comment lines — so a violation in product code is still a violation, and the documentation
that the rule depends on can stay. **This is a deviation** (below).

**Two negative controls, both red, both run:**

- `--self-test` builds a throwaway tree holding one real `Instant::now` in a product file plus
  the three shapes the unscoped row used to match (a doc comment, a `cfg(test)` module, a
  `tests.rs`), and asserts the product one is found and the other three are not — **exit 0**.
- a temporary `Instant::now` in a real product file (`layout/force/quadtree.rs`) turns the row
  **red, exit 1**, naming the file and line; a temporary `mul_add` does the same. Both
  reverted; the clean tree is exit 0.

No other gate row was deleted or weakened.

## File splits (the 300-line cap)

Three files this branch touched were over the cap and are now child modules, each split by
claim rather than by line count:

- `exec/partition/tests.rs` (320) → `tests/{mod,rule,contract,runner}.rs` — the partition
  rule, the `StepRange` contract with its negative control, and `Serial`.
- `bench/tests.rs` (383) → `tests/{mod,plan,harness,ladder,scale}.rs` — the plan's vocabulary,
  the two JS arms' self-checks with their negative controls, the crossover table, the fixtures.
- `hashgate/tests/mod.rs` (382) → `tests/{mod,compare,pipeline,arm_lines}.rs` beside the
  existing `knob`/`report`/`stages` — the comparison, the pipeline's bytes, what an arm prints.

The split was mechanical: no test was dropped, merged or weakened, and the count is unchanged
(191 graph-cli unit tests before and after).

## Deviations

1. **`scripts/forbidden-constructs.sh` is a new file outside the phase's CREATE list**, and
   `prompts/phase-11-compute-tiers.md`'s gate row now calls it instead of the bare `grep`. The
   row as written could not pass on a clean tree (it matches the crate's own doc comments), so
   scoping it was necessary rather than optional; the alternative — deleting the documentation
   of the rule — would have been the worse change.
2. **`docs/decisions/link-gather.md`** is a new file outside the CREATE list (carried over
   from the previous slice, which the orchestrator asked for).
3. **Child modules** under `exec/partition/tests/`, `bench/tests/` and `hashgate/tests/` — the
   300-line cap forces them, and each is the implementation or the tests of a file the
   envelope names.

## Gates run in this slice

Every row below was run in this slice, on this host.

| check | result |
|---|---|
| `gr cargo fmt --all --check` | **0** |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| `gr cargo test --workspace --no-fail-fast` | **0** — 974 passed, 0 failed |
| `gr cargo build -p graph-core --target wasm32-unknown-unknown` | **0** |
| `hashgate --seeds 8` (4-way) | **0** — 4-way equal on 8/8, all 12 stages |
| `hashgate --seeds 8 --tiers all` (10-way) | **0** — 10-way equal on 8/8, all 12 stages |
| `GM_MUTATE_SPLIT_SUM=charge` `--seeds 8 --tiers all` | **1** — 0/8 equal |
| `GM_MUTATE_SPLIT_SUM=collide` `--seeds 8 --tiers all` | **1** — 5/8 equal |
| `GM_MUTATE_SPLIT_SUM=link` `--seeds 8 --tiers all` | **1** — 0/8 equal |
| `GM_MUTATE_SPLIT_SUM=1` `--seeds 8 --tiers all` | **1** — 0/8 equal |
| `GM_MUTATE_SPLIT_SUM=collide` `--seeds 2` / `--seeds 4` | **2** — refused, "could not run" |
| `GM_MUTATE_SPLIT_SUM=collide` `--seeds 5` | **1** — 4/5 equal |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | **1** — 8/8 diverge |
| `bash scripts/forbidden-constructs.sh crates/graph-core/src` | **0** |
| `bash scripts/forbidden-constructs.sh --self-test` | **0** |
| the same row, with a planted `Instant::now` in a product file | **1** — the negative control |
| the same row, with a planted `mul_add` in a product file | **1** — the negative control |
| `bench --n 220,10000,100000 --tiers scalar,threads --repeat 5` | **0** — every arm `equal to scalar: true` |

## Not run, and therefore not claimed

The **1000-seed** hashgate, the full `gate.sh`, `cargo mutants` and the `+simd128` wasm build
row are the orchestrator's, under the host-wide lock, and none was run here. The 8-seed runs
above are exploration, not gate rows. `capabilities --check` is not run and not claimed.

## Next slices, in order

1. **`exec.threads.native` in the ledger**, if the orchestrator takes the row: the speedup is
   measured and the equality holds at 8 seeds, and what is left is the 1000-seed run that
   `capabilities --check` reads a `gated` row from.
2. **SIMD**, autovectorisable SoA first, in the three `step_range` bodies, checked with the
   ten-arm gate.
3. **Browser workers**, after stop-and-ask 1.
4. **GPU**, only after stop-and-ask 2 and only with the crossover numbers in hand.
