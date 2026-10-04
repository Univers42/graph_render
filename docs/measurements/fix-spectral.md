# fix-spectral — LF-09, LF-10, LF-11, LF-25, LF-26, verified

Source: `docs/reviews/review-layout-force.md`, ids `LF-09`, `LF-10`, `LF-11`, `LF-25`, `LF-26`
(pivot half). Brief: `target/wf/fix-spectral.md` and its addendum. Worktree `fix-spectral`,
verified on 2026-10-04 at `d1522949` (two commits from outside this job follow it, carrying the
same tree; see "Restore" below). Every number below came from a run in this worktree in this
session.

**The one red row is `MDS_3D`, and LF-11 alone causes it.** `layout.mds.pivot3d` moved its
bytes; nothing else moved. Task 3 below is the experiment that isolates it. Nothing was
re-pinned.

## Findings — Task 0 of the addendum

The addendum asked for the failing gate, the component size, the peak residual at `n = 300`
and why `dims = 3` fails where `dims = 2` passes. Measured on the seeded gate model
(`seeded_model(7, n, REFERENCE_DEGREE)`, the generator `graph-cli`'s caps ladder grows — called,
never copied), one connected component, release build, through the module's own ungated
`lobpcg_block` seam:

| n | arm | component size | block `b` | `dims_eff` | peak residual | residual limit | residual gate | `‖VᵀV − I‖_max` | orthonormality gate |
|---|---|---|---|---|---|---|---|---|---|
| 256 | 3D | 256 | 5 | 3 | `9.869e-7` | `2.147e-3` | pass | `1.000e0` | **fail** |
| 256 | 2D | 256 | 4 | 2 | `9.713e-7` | `2.147e-3` | pass | `5.551e-16` | pass |
| **300** | **3D** | **300** | **5** | **3** | **`4.183e-5`** | **`2.205e-3`** | **pass** | **`1.000e0`** | **fail** |
| 300 | 2D | 300 | 4 | 2 | `8.454e-7` | `2.205e-3` | pass | `1.665e-15` | pass |
| 512 | 3D | 512 | 5 | 3 | `4.642e-5` | `2.066e-3` | pass | `1.000e0` | **fail** |
| 700 | 3D | 700 | 5 | 3 | `8.258e-4` | `1.997e-3` | pass | `1.000e0` | **fail** |

- **The failing gate is orthonormality, not the residual.** At `n = 300` the peak residual is
  `4.183e-5` against a limit of `2.205e-3` — inside the gate by a factor of 53, and the review
  named the wrong one. The orthonormality gate fails at exactly `1.0`, against a limit of
  `1e-6`.
- **Component size: 300**, one component, at the addendum's `n = 300`. The seeded gate model is
  connected, so "the component" and "the graph" are the same set here; `reports.len() == 1` is
  asserted by `the_seeded_gate_model_solves_in_three_dimensions_up_to_the_registry_ceiling`.
- **Why `dims = 3` fails where `dims = 2` passes.** The block is `b = dims + 2`, so 5 against 4.
  At `b = 5` the LOBPCG block's **first column is the zero vector carrying the trivial
  eigenvalue `λ = 0`**: measured column norms are `["0.0000e0", "1.0000e0", "1.0000e0",
  "1.0000e0", "1.0000e0"]` and values `["0.0000e0", "1.4976e-1", "1.9292e-1", "2.1066e-1",
  "2.2053e-1"]`. A zero column has a residual of exactly zero — which is why the residual gate
  is blind to it — and `|0 − 1| = 1` on the diagonal, which is why the orthonormality gate is
  not. One column narrower (`b = 4`, `dims = 2`) admits nothing numeric and the same graph
  solves, at `‖VᵀV − I‖_max = 1.665e-15`. So it is the **block width, not the spectrum, not the
  seed and not the node count**: `b = 5` collapses a column at 256, 300, 512 and 700 alike.
- **`n = 256` passed the addendum's ladder for a different reason than 300 did.** It is below
  `DENSE_EIG_LIMIT = 256`, so it never reaches LOBPCG at all. The 3D arm's first LOBPCG size is
  300, which is exactly where the addendum saw it start failing.

After the fix, on the same model (`run_3d` / `run`, `DEFAULT_SEED`):

| n | 3D tier | 3D peak residual | 3D solved | 2D tier | 2D iterations | 2D solved |
|---|---|---|---|---|---|---|
| 300 | `ShiftInvert` | `8.727e-7` | yes | `Lobpcg` | 112 | yes |
| 400 | `ShiftInvert` | `4.080e-7` | yes | `Lobpcg` | 95 | yes |
| 512 | `ShiftInvert` | `5.891e-7` | yes | `Lobpcg` | 113 | yes |
| 700 | `ShiftInvert` | `3.207e-7` | yes | `Lobpcg` | 341 | yes |

## Task 2 — the addendum's GREEN at the ceiling

`cap-probe` **does not exist in this tree or on `origin/develop`** (`fd3ef725` added
`crates/graph-cli/src/bench/cap_probe.rs` and `scripts/caps-ladder.sh`; neither file is here,
and `graph-cli --help` lists no such subcommand — it exits 2 with `unrecognized subcommand`).
The in-tree command that answers the same question is `snapshot`, which runs the same
`seeded_model(seed, nodes, REFERENCE_DEGREE)` generator through the same registry entry and
exits 2 on a `StageError`, so "does `layout.spectral3d` refuse this size" is the same
measurement:

```
scripts/orch/gr cargo build -q --release -p graph-cli                          -> rc 0
scripts/orch/gr bash -c 'for n in 256 300 512 700; do
  target/release/graph-cli snapshot --seed 0 --nodes $n --layout spectral3d --out-bin /dev/null;
  echo "n=$n rc=$?"; done'
  n=256 rc=0   (16472 bytes, sha256 226889d78ab225f9…)
  n=300 rc=0   (19312 bytes, sha256 be28debd585120ca…)
  n=512 rc=0   (33400 bytes, sha256 b7ec01f34bad1467…)
  n=700 rc=0   (45960 bytes, sha256 21493f874dafc283…)
```

All four are 0, so the addendum's GREEN (`n = 700` exits 0) holds. The in-tree graph-core test
over the same seeded model is
`the_seeded_gate_model_solves_in_three_dimensions_up_to_the_registry_ceiling`
(`spectral/tests/shift_invert.rs:107`), which asserts 300, 400, 512 and 700 — the collapse is
a function of the block width, so a fix reaching only the largest size would be a coincidence.

## Task 1 — the bytes of the four ids

A base tree was built with `git archive origin/develop | tar -x -C target/base`, with
`crates/graph-core/src/layout/tests/digest.rs` copied in and registered the same way this tree
registers it (`tests.rs` → `tests/mod.rs` on develop, which has no `tests/` directory there, and
`mod digest;` appended). Both trees were run with
`cargo test -q -p graph-core --release digest_print -- --ignored --nocapture` (the base through
`scripts/orch/gr bash -c 'cd target/base && …'`, because `gr` mounts the git top-level).

Logs: `target/wf/digest-base.txt`, `target/wf/digest-branch.txt`. The diff of the two files is
two lines, and neither is a digest:

```
36c36
< test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1494 filtered out; finished in 0.00s
---
> test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1467 filtered out; finished in 0.00s
46c46
< test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 53 filtered out; finished in 0.00s
---
> test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 48 filtered out; finished in 0.00s
```

The differing numbers are the test counts (this branch adds 27 graph-core tests and 5 doc tests
to develop's suite). **The 32 digest lines are identical** — `diff` over the `^seed=` lines
alone exits 0, which covers all four ids at all 8 seeds:

- `layout.spectral`: 8/8 identical.
- `layout.mds.pivot`: 8/8 identical. LF-11 moved nothing on the gate model.
- `layout.spectral3d`: 8/8 identical. LF-09/LF-10 moved nothing on the gate model.
- `layout.mds.pivot3d`: 8/8 identical on the **gate seeds** — which is the trap: the gate seeds
  are 2 to 9 nodes, far too small for a tie at a non-zero eigenvalue, so they cannot see the
  move Task 3 finds. See "What the digest does not cover".

## Task 3 — `MDS_3D`, and whether LF-11 alone moves it

`scripts/scigraphs-conformance.sh` re-run on this tree: **rc 1**, one red row,
`MDS_3D: FAIL — motor bytes are not the pinned ones (sha b625226d…)`
(`target/wf/conformance-run2.txt`). `IGRAPH_KK 23/24` in the same log is SciGraphs' own
reference raising `ValueError: IGRAPH_KK produced 9 non-finite coordinate(s)`, not the motor.
`MDS_3D` is `layout.mds.pivot3d` (`oracle_python/conformance/rows.rs:171`) pinned `bitwise` at
`1e-15` (`baseline/table/igraph.rs:29-37`).

**The experiment.** One line in `crates/graph-core/src/layout/pivot_mds.rs` — the
`canonicalise(&mut top);` call at `:202`, LF-11's whole effect — was replaced by
`let _ = &canonicalise;`, the script re-run, and the file restored afterwards. Nothing else was
touched.

| | motor sha | `bitwise_f64` | `bitwise_f32` | procrustes median | procrustes max | max_gap | script rc |
|---|---|---|---|---|---|---|---|
| LF-11 **on** (this branch) | `b625226d…` (**moved**) | 22 of 1020 | 881 | `3.134e-16` | `0.2556` | `9.730` | **1** |
| LF-11 **off** (develop behaviour) | matches the pin | 25 of 1020 | 903 | `3.114e-16` | `0.2556` | `9.730` | **0** |
| SciGraphs reference | pinned `603f394d…` | — | — | ceiling `1e-15` | — | — | — |

**LF-11 alone moves the row.** With the canonicalisation bypassed, `MDS_3D` returns to its
pinned bytes and the script exits 0 with every other row unchanged. No other hunk of LF-09,
LF-10, LF-25 or LF-26 touches `pivot_mds`, and `SPECTRAL_3D` (which uses `layout.spectral3d`,
not this path) is `ok` in both runs at `median 4.667e-16`.

**What the move costs and what it buys.** The shape is unaffected at the level the row gates on.
The Procrustes median against the SciGraphs reference is `3.134e-16` with LF-11 on and
`3.114e-16` with it off — both three orders of magnitude inside the `1e-15` ceiling, and the
difference between them is in the noise of which arbitrary orientation each arm drew. The
`max_gap` is bit-identical (`9.730`) in both. The fixtures where the two arms genuinely differ
are the ones carrying a tied eigenspace the reference also had to orient arbitrarily:
`gate-06` (`1.02e-5`), `gate-07` (`0.256`), `gate-08` (`0.036`), `dag-diamond` (`0.027`) and
`bipartite` (`0.231` with LF-11 on, `0.097` with it off) — the last is the one place LF-11
measurably moves *away* from the reference, and by less than a factor of 2.4 on a quantity
whose ceiling is `1e-15`. `bitwise_f64` is 22 of 1020 with LF-11 on and 25 with it off, so the
canonicalisation costs 3 `f64` coordinates of agreement with a pin and buys a stated rule in
place of `tred2`/`tql2`'s internal arithmetic.

**Restore.** `crates/graph-core/src/layout/pivot_mds.rs` is byte-identical to the brief's
`HEAD d1522949` (`git show d1522949:… | diff - …` exits 0) and the working tree is clean of it.
Two commits appeared in this tree mid-session from outside this job: `672da1e4` captured the
scratch bypass line, and `6f3f253f` captured the restored file. Nothing was re-pinned.

## Task 4 — the merge floor

| row | command | exit code |
|---|---|---|
| format | `scripts/orch/gr cargo fmt --all --check` | **0** |
| clippy | `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| graph-core tests | `scripts/orch/gr cargo test -q -p graph-core --no-fail-fast` | **0** — 1457 + 5 + 48 + 2 passed, 0 failed, 19 ignored |
| wasm32 | `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | **0** |
| hashgate | `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | **0** — `PASS`, 4-way equal on 8/8 seeds, every row |
| negative control | `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | **1** — `FAIL: 8 of 8 seeds diverge` (expected non-zero) |
| capabilities | `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | **1** — `77 rows, 36 problems` (pre-existing; see below) |
| conformance | `scripts/scigraphs-conformance.sh` | **1** — `MDS_3D` only |

`capabilities --check` is red **identically on the base tree**: the same 36 problems over the
same 17 ids (`target/wf/capcheck.txt` vs `target/wf/capcheck-base.txt`). Every one is an
evidence-ledger row — "gated, but hashgate ran 8 seeds, need 1000", "no oracle-spectral record:
run the gate" — which the 1000-seed timed gate and the oracle scripts write, and which this
session was told not to run. LF-25's own requirement, that `capabilities --check` "stay as it is
for these rows", holds: `layout.spectral` and `layout.mds.pivot` are still `gated` at the same
tier and the same `scale_ceiling`.

## Each LF id

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| LF-09 | MAJOR | fixed | `the_seeded_gate_model_solves_in_three_dimensions_up_to_the_registry_ceiling`, `the_three_dimensional_arm_reaches_the_shift_invert_tier`, `a_three_hundred_node_grid_solves_in_three_dimensions`, `the_failing_gate_is_orthonormality_not_the_residual` | `layout/spectral/shift_invert.rs:1-180` (the tier), `layout/spectral/solve.rs:90-127` (the cascade), `layout/spectral/tests/shift_invert.rs:106`, `:126`, `:142`, `:160` |
| LF-10 | MAJOR | fixed | `spectral_stage::tests` — a 1025-node path is refused, and an all-passing run is untouched byte for byte | `layout/spectral_stage.rs:5-26` (the rule), `:33-46` (the two errors), `:85-94` (`solve`), `:132` (RED), `:197-208` (the negative control) |
| LF-11 | MAJOR | fixed, **moves `MDS_3D`** | `c4_and_c8_land_on_their_hand_computed_orientation`, `a_rotated_tied_basis_gives_the_same_canonical_coordinates` | `layout/pivot_mds/tied.rs:1-66` (the rule in full), `layout/pivot_mds.rs:41-48` (`TIE_TOL`, `COLLAPSE_NORM`), `layout/pivot_mds.rs:202` (the call), `layout/pivot_mds/tests.rs:103-159`, `:183-227` |
| LF-25 | MINOR | doc-only | — | `registry/spectral.rs:10-30` (what the ceiling bounds vs the per-component spectral gap), `:32-42` (the MDS ceiling) |
| LF-26 | MINOR | fixed | `pivot_mds::tests::c4_and_c8_land_on_their_hand_computed_orientation` (the new pin; `is_deterministic_run_twice` is kept at `:235` but demoted to the plain purity statement it always was) | `layout/pivot_mds/tests.rs:19-26` (why `P_3` cannot see it), `:130-159`, `:229-238` |

## What the digest does not cover

`digest_print` compares 8 gate seeds at `n = 2..9`. `pivot_mds`'s Gram matrix is
`k = min(100, n)` wide, so a tie at a **non-zero** eigenvalue needs at least the 4 nodes of
`C4` — and a tie big enough to move a 3-column selection needs the shape the conformance
fixtures have. The digest is therefore silent on exactly the defect LF-11 fixes, and on the row
it moves. The row that would have caught it is `scigraphs-conformance.sh`'s `MDS_3D`, and it
did.

## Decisions needed

1. **`MDS_3D`: re-pin or narrow LF-11.** Recommendation: **re-pin `MDS_3D` at its new bytes.**
   The brief says a moved row is "reported, never re-pinned", but it also says LF-11's
   determinism fix is required, and the two cannot both hold: the tie's orientation is
   arbitrary, so *some* pinned sha has to give. The evidence says LF-11's answer is at least as
   close to the SciGraphs reference (`3.114e-16` against `3.134e-16`, both inside `1e-15`), it
   is stated as a rule rather than inherited from `tred2`/`tql2`'s arithmetic, and it is what
   makes `C4` and `C8` land on a hand-computed orientation. Narrowing LF-11 to the 2D arm would
   keep the pin but leave `layout.mds.pivot3d` — the row that moved — still solver-rotated,
   which is the defect LF-11 exists to remove.
2. **`docs/decisions/eigensolver.md`** rejected the shift-invert tier ("needs a sparse direct
   factorisation"). That record's reason is what has gone: the tier builds a **dense** Cholesky
   of `L + 1e-3 I`, which is what `DENSE_EIG_LIMIT`'s own dense tier already builds. The file is
   outside this job's paths and was not edited. It should record the substitution.
3. **`StageError` cannot carry the measured number.** LF-10 names the condition in
   `StageError::Param`'s `rule` and points at `ComponentReport::peak_residual` for the numbers,
   because `StageError` is `Copy` over `&'static str` and carrying a per-component `f64` would
   change every stage's error type. Making `graph-cli` print the report is a one-line step for
   whoever owns that path.
4. **`cap-probe` is in the brief but not in the tree.** It arrived in `fd3ef725` on an unmerged
   branch. Either that commit lands before the next job needs it, or the brief should name
   `snapshot` for size-ladder questions.
