# Job fix-sc-misalign (agent build, branch fix-sc-misalign, worktree ~/goinfre/wt/fix-sc-misalign)

Context: the branch starts from `sg-igraph-3d`, which is still waiting to land. If it has landed by
the time you finish, `git merge origin/develop -m updated` (never rebase).

The defect, measured on 2026-10-04 in the sg-igraph-3d worktree after `scripts/scigraphs-conformance.sh`:
- `target/scigraphs-conformance/ref/IGRAPH_KK.json` marks fixture 5 (`gate-01`, 3 nodes) `"not run"`.
  Its transcript reads `ValueError: IGRAPH_KK produced 9 non-finite coordinate(s)`. This is the
  reference defect `G_KK_NON_FINITE`, and its 9 coordinates are absent from `ref/IGRAPH_KK.f64`,
  which holds 1011 values where the motor's `motor/IGRAPH_KK.f64` holds 1020.
- `harness/scigraphs-conformance.py` `_row_metrics` (about line 150) slices both files fixture by
  fixture with two running offsets and never reads the reference's per-fixture status.
- So from `gate-01` on, every IGRAPH_KK fixture compares the motor against the next fixture's
  reference coordinates. The shortfall only surfaces at the last fixture:
  `metrics.json` `rows.IGRAPH_KK.per_fixture[23]` says
  `not run: IGRAPH_KK holds 1011 coordinates, gate-19 needs 1020`.
- The function's own docstring says a fixture the reference did not reach is dropped. The code
  does not do that.
- IGRAPH_KK is the only row with a non-ok reference fixture today. The fix must hold for any row
  and for either arm.

Exact tasks:
1. Write the failing test first: a `unittest` case beside `harness/scigraphs-conformance/test_sc_propose.py`
   (a new `test_sc_metrics_slice.py`; it runs as `python3 harness/scigraphs-conformance/test_<x>.py -v`).
   Use synthetic arrays: three fixtures, the middle one not run on the reference. Assert that the
   third fixture is compared with its own coordinates and the middle one is reported `not run:` with
   the reference's own detail. Also cover the symmetric case, where the motor skipped a fixture.
   Show it red before the fix.
2. Fix `_row_metrics` (and `_slice` if needed). For each fixture, read the status each arm
   recorded:
   - Reference: `ref/<NAME>.json` `fixtures[i].status`.
   - Motor: find where the motor arm records a skipped fixture (`motor.jsonl` `skipped`, or
     the motor writer under `harness/scigraphs-conformance/`), and read it from there.
   Advance an arm's offset only for a fixture that arm wrote. Report a fixture either arm skipped
   as `not run: <arm> <status>: <detail>`. Keep the function ≤ 40 lines; split a helper if needed.
3. Re-run `scripts/scigraphs-conformance.sh` and `scripts/scigraphs-conformance.sh --break` (the
   latter must exit 1). Paste the IGRAPH_KK row before and after: per-fixture count, the aggregate
   metric and its ceiling.
   - If the corrected number breaks the pinned ceiling in
     `crates/graph-cli/src/oracle_python/conformance/baseline/table/networkx.rs` (the `IGRAPH_KK`
     entry, ceiling `1e0`), change only that entry.
   - Justify the new value from `target/scigraphs-conformance/conformance-baseline-proposed.rs`.
4. Add a short paragraph to `docs/measurements/scigraphs-conformance.md` covering the defect, the
   fix, the before/after numbers and the commands.

Paths allowed: `harness/scigraphs-conformance.py`, `harness/scigraphs-conformance/**`,
`crates/graph-cli/src/oracle_python/conformance/baseline/table/networkx.rs` (the IGRAPH_KK entry
only), `docs/measurements/scigraphs-conformance.md`, `prompts/jobs/fix-sc-misalign.md`.
Not allowed: everything else, in particular `SciGraphs/` (a submodule; the reference stays as it
is), `crates/graph-core`, `packages`, `app`, `server`, `deploy`, and every other baseline table file.

Limits: functions ≤ 40 lines, files ≤ 300 lines, every heuristic carries a `Ponytail:` line.
Every container goes through `scripts/orch/drun`.

Done when, each with its command and exit pasted:
- the new test passes, after having been shown red;
- `python3 harness/scigraphs-conformance/test_sc_propose.py -v` passes;
- `scripts/scigraphs-conformance.sh` exits 0 and its `--break` exits 1;
- `scripts/orch/gate.sh target/gate-fix-sc-misalign scripts/orch/rows/quick.rows` is all PASS.
