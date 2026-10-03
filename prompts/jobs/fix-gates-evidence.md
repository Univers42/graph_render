# Job fix-gates-evidence (agent build, review repairs: the ledger, its evidence and the fingerprint)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-gates.md` (ids `RG-NN`).
Ids: RG-02, RG-03, RG-08, RG-17, RG-23, RG-24, RG-28 (BLOCKER/MAJOR); RG-29 … RG-34, RG-50, RG-53
(MINOR).

Each repair closes a path by which `capabilities --check` or a recorded gate reads as green when it
should not, so each ships a negative control (a test that fails on the current code): an empty
`functions` slice (RG-02), a file outside the fingerprint that changes a result (RG-03), a record
written for another tree (RG-17), a new layout id falling through `layout_row.rs` (RG-24), a stale
generated file `codegen --check` misses (RG-28).

Judgement notes:
- RG-02: an empty set is "absent", never "gated". Fix it where the verdict is computed
  (`capabilities/verdict.rs`), once, not per caller.
- RG-03 (decided 2026-10-02): fingerprint only the scripts that change a gate's result:
  `scripts/orch/gr`, `scripts/scigraphs-conformance.sh` and every file either sources or runs
  (follow them; list them in the report). Do NOT add all of `scripts/`: `scripts/orch/queue.txt`
  and the orchestration scripts change hourly and would void every record. Name the boundary in a
  `Ponytail:` line next to `FINGERPRINTED`.
- RG-23: promoting an `analysis.*` row is a ledger change; if the check cannot run for a row,
  the row says so (status `stub` with the reason), it does not read `gated`.
- Adding a path to the fingerprint voids today's records. That is expected: say so in the report
  and do not re-record evidence from this job.

Paths: `crates/graph-cli/src/capabilities.rs`, `crates/graph-cli/src/capabilities/**`,
`crates/graph-cli/src/fingerprint.rs`, `crates/graph-cli/src/evidence.rs`,
`crates/graph-cli/src/codegen.rs`.

Done when, in addition to fix-common: `capabilities --check` exits with the same set of problems
as on the untouched tree except the ones a finding changed on purpose (paste both lists and name
each difference), and `codegen --check` exits 0.
