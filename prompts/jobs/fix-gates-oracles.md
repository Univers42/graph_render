# Job fix-gates-oracles (agent build, review repairs: oracle differentials, fixtures, stress)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-gates.md` (ids `RG-NN`).
Ids: RG-04, RG-09, RG-10, RG-11, RG-12, RG-18 … RG-22 (BLOCKER/MAJOR); RG-45, RG-46, RG-56, RG-57
(MINOR). Not here (held for `fix-gates-conformance`, after the sg wave lands): RG-13 … RG-16,
RG-25, RG-43, RG-44, RG-47, RG-49, RG-52, RG-54, RG-55.

A differential that compares against a ceiling it can never reach, or against our own output,
passes by construction. Each repair ships a negative control that fails on the current code.

Judgement notes:
- RG-04 (circo 1e5, fdp 1e6, sfdp 1e3): a ceiling is a measurement. Re-measure the worst gap on
  this tree (the refs are at `$GM_SCRATCH/refs`; `scripts/orch/fetch-refs.sh` if absent), set the
  ceiling to the measured worst gap times a stated margin, and write the run in the report. If the
  measured gap is so large that the row checks nothing (the review's case), the row's verdict
  becomes `tolerance` with that reason in the ledger, never a pass. Graphviz output is the target
  (user decision 2026-09-30): never loosen the port, only the claim.
- RG-11, RG-12: a closed case that skips the comparison, or a judge that maps "missing column" to
  a pass, is a refusal (exit 2) with the case named.
- RG-19 (`child_first` dropped by `oracle_fixtures/wire.rs`): both directions of the wire carry
  every `EdgeRecord` field; a round-trip test over a fixture with `child_first` set proves it.
- RG-09, RG-10 (`stress.rs`): the bound must be able to fail; show it with an input above it.

Paths: `crates/graph-cli/src/oracle_python.rs`, `crates/graph-cli/src/oracle_python/{circo,fdp,sfdp,
closed_form,osage,patchwork,spectral,hierarchical_3d}.rs` (and their child modules),
`crates/graph-cli/src/oracle_fixtures.rs`, `crates/graph-cli/src/oracle_fixtures/**`,
`crates/graph-cli/src/forcecheck.rs`, `crates/graph-cli/src/stress.rs`,
`crates/graph-cli/src/stress/**`, `harness/gv_plain.py` (RG-22 only), the ceiling docs under
`docs/measurements/` that a re-measured ceiling cites. Never `oracle_python/conformance/**`,
`igraph.rs`, `fa2.rs`, `spring/**` or `oracle_python/tests.rs` (owned by running sg jobs).

Done when, in addition to fix-common: each re-measured ceiling has its command and output in the
report; `capabilities --check` lists no new problem.
