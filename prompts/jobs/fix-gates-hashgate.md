# Job fix-gates-hashgate (agent build, review repairs: the hash gate and the CLI front door)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-gates.md` (ids `RG-NN`).
Ids: RG-01, RG-05, RG-06, RG-07, RG-26, RG-27 (BLOCKER/MAJOR); RG-35 … RG-42, RG-48, RG-51, RG-58
(MINOR).

The question the review asked of every gate: can it exit 0 when it should not? Each repair here
closes one such path, so each one ships a negative control: a test that feeds the input the review
names (`GM_MUTATE_NODE_COUNT=0`, `--seeds 0`, the report case of RG-06/07, ...) and asserts the gate
now refuses it (exit 1 or 2 per the gate's exit-code mapping, review §"Exit-code mapping"), and
fails on the current code. Paste the failing run, then the passing one.

Judgement notes:
- RG-01, RG-05: a knob or seed count that makes the gate compare nothing is a refusal (exit 2,
  "could not run"), never a pass. The refusal message names the argument and the accepted range.
- RG-26, RG-42: a knob value that parses but perturbs nothing is the same defect; one check in
  `knob/setting.rs` covers every knob (the knob list exists once, `crates/graph-cli/tests/common/mod.rs`).
- RG-51 (`$CARGO`/`node` taken from `PATH`): resolve once, refuse a missing binary with exit 2.
  Do not add a hostile-`PATH` experiment that needs a fake wasm artifact; a unit test on the
  resolver is enough.
- RG-48 (tests that cannot fail): make each named test able to fail, and show it failing once
  against a mutated input inside the test.

Paths: `crates/graph-cli/src/hashgate.rs`, `crates/graph-cli/src/hashgate/**`,
`crates/graph-cli/src/command.rs`, `crates/graph-cli/src/runner.rs`, `crates/graph-cli/src/main.rs`
(RG-51 only, additive), `crates/graph-cli/tests/common/mod.rs` (only if a knob's accepted range
lives there).

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0, its
`GM_MUTATE_REFERENCE_DEGREE=9` control exits non-zero, `hashgate --seeds 0` exits 2 and
`GM_MUTATE_NODE_COUNT=0 hashgate --seeds 8` exits non-zero; paste all four. The per-stage hashes of
`--seeds 8` are equal before and after (this job moves no output).
