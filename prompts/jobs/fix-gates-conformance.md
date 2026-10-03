# Job fix-gates-conformance (agent build, review repairs: the SciGraphs conformance gate and its neighbours)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-gates.md` (ids `RG-NN`).
Ids: RG-13, RG-14, RG-15, RG-16, RG-25 (MAJOR); RG-43, RG-44, RG-47, RG-49, RG-52, RG-54, RG-55
(MINOR). The other RG ids are owned by `fix-gates-hashgate`, `fix-gates-evidence` and
`fix-gates-oracles`.

These are the gate's mechanics, not its rows. The sg-* jobs own the row entries (`rows.rs`,
`gaps.rs`, the `row(...)` blocks under `baseline/table/`, the row dispatch in `motor.rs`); you own
the code that emits, slices, judges and refuses. Every repair ships a negative control that fails
on the current code. `scripts/scigraphs-conformance.sh` exits 0 before and after with no row moved
(paste the row lines from `target/scigraphs-conformance/metrics.json`, before and after).

Judgement notes:
- RG-13 (`raw_f64` flattens `Err` runs): first read how `motor.rs` builds `skipped` and whether the
  Python arm (`harness/scigraphs-conformance/sc_reference.py`, `sc_metrics.py`) honours it when it
  slices `motor/<NAME>.f64`. If a refused fixture is named in the manifest and both arms skip it,
  the finding is `false` (the test that proves it stays). Otherwise make the skip explicit on both
  sides; never shift a later fixture's bytes. Do not turn a row that runs today into exit 2: if a
  layout refuses a fixture on this tree, that refusal is recorded, not promoted to a failure.
- RG-14: a `--break` / `GM_MUTATE_SCIGRAPHS_CONFORMANCE` name that is not in `ROWS`, or names a row
  with `motor: None`, is exit 2; `broken: true` is published only when the bit flip happened.
- RG-15: refuse an empty fixture set and an empty row set on both sides (emit and harness);
  `render.py` returns 2 on an empty `shapes/`.
- RG-16: the reviewer's fix (the emit records a digest of `spring-theirs.jsonl`) is out of order:
  the emit runs before the arm writes that file. Make the reference provably this run's: the arm
  (`harness/oracle-spring.py`) echoes the manifest's fixture digest into its output and the ingest
  refuses a mismatch; correct the `ingest.rs` comment. Add the floor: `cases` equals the manifest's
  seed count and `scored >= 0.9 * cases`, with a `Ponytail:` on the 0.9.
- RG-25: compare `procrustes_max` (and `max_gap` where the row records it) against a ceiling in
  `Baseline`, or stop publishing an uncompared number as evidence. A new ceiling is a measurement:
  set it from this tree's run times a stated margin and paste the run. `verdict.rs` is 290 lines:
  split it along a seam before it passes 300.
- RG-43: keep the 2-iteration gate. A `--max-iter` other than the gated budget must not reuse the
  1e-7 ceiling: refuse it, or record the result as an observation that never passes. A second row
  at the shipped budget is optional and only with a measured self-spread ceiling.
- RG-44: a registered layout whose `ours` column is missing is an explicit `Err`, not an omission.
- RG-47, RG-49: a compile-time assert that the family lengths sum to the table length, and one
  test that walks `BASELINE` (non-empty cause, finite ceiling, pinned motor sha where the mode is
  `bitwise`). The six unpinned `basic.rs` rows: if pinning them would move a row, record the
  finding's second half under "decisions needed" with the row names; do not re-pin from here.
- RG-52: refuse duplicate ids, fewer than 2 nodes, out-of-range endpoints and a pair whose length
  is not 2 before filtering; add the lower bound beside `gate_model.rs`'s upper cap. A repo fixture
  that the new refusal rejects is a finding to report, not a fixture to edit.
- RG-54, RG-55: as the review proposes.

Paths: `crates/graph-cli/src/oracle_python/conformance/{emit,motor,verdict,fixtures,baseline}.rs`
and their child modules (`verdict/**`, `fixtures/**`, `baseline/table.rs` for the assert only,
never a `row(...)` block), `crates/graph-cli/src/oracle_python/conformance/tests.rs`,
`crates/graph-cli/src/oracle_python/spring.rs` and `spring/**`, `crates/graph-cli/src/oracle_python/{fa2,igraph}.rs`,
`harness/scigraphs-conformance.py`, `harness/scigraphs-conformance/**`, `harness/oracle-spring.py`,
`scripts/scigraphs-conformance.sh`, `docs/measurements/fix-gates-conformance.md`. Never `rows.rs`,
`gaps.rs`, a family file's row entries or `motor.rs`'s row dispatch (sg jobs).

Done when, in addition to fix-common: `scripts/scigraphs-conformance.sh` exits 0 with no row moved
and `--break` exits 1; a misspelled `--break` name exits 2 (paste it); `graph-cli oracle-spring` on
a one-seed run exits non-zero (paste it); `capabilities --check` lists no new problem.
