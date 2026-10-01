# Shared rules for every `sg-*` repair job (SciGraphs conformance)

Read this before the job body; the body names its row(s), files and done-when.

Precondition: the tree has `crates/graph-cli/src/oracle_python/conformance/` and
`scripts/scigraphs-conformance.sh` (branch `scigraphs-conformance`, landed on develop). If either is
absent, stop and report under "decisions needed": the job cannot measure its row.

The gate. `scripts/scigraphs-conformance.sh` compares every SciGraphs layout with the motor over one
fixture set and judges each row against the pinned baseline in
`crates/graph-cli/src/oracle_python/conformance/baseline/table.rs` (exit 0 agree, 1 a row moved,
2 could not run). The matrix and the reasoning per row are in `docs/measurements/scigraphs-conformance.md`;
its "Repairs" section is a hypothesis, not a spec: where it disagrees with the code or the reference,
the code and the reference win, and you correct the doc's text for your row.

The loop, for each row the body names:
1. Measure first. `scripts/orch/gr cargo build --release -p graph-cli`, then
   `scripts/scigraphs-conformance.sh`; it must exit 0 on the untouched tree. Paste the row's line
   from `target/scigraphs-conformance/metrics.json` (tier, cause, disparity, bitwise counts).
2. RED: a unit test in graph-core (or graph-cli) that pins the reference value the row needs and fails
   on the current code. Paste the failing run.
3. GREEN: the smallest change. Paste the passing run.
4. Re-measure. The script now exits 1 and names your row. Copy ONLY your row's `row(...)` block from
   `target/scigraphs-conformance/conformance-baseline-proposed.rs` into `baseline/table.rs` (never
   retype a sha256), re-run: exit 0. Then `scripts/scigraphs-conformance.sh --break` must exit 1.
   Any OTHER row that moved is a regression: stop and report it, do not re-pin it.
5. When a gap is closed, delete it from the row's `gaps` in `conformance/rows.rs` (and its const in
   `gaps.rs` once no row names it); a gap left behind after the fix is a false record.
6. Update your row in the doc's matrix and its repair paragraph; put the before/after numbers and
   the commands in `docs/measurements/<job label>.md`.

What "conformant" means here. The motor's `Geometry` is `f32` (`layout/basic_3d.rs:45-51` narrows
once), so the reachable exact target is the `f32 k/N` column at N/N; `f64 k/N` reaches N/N only
where the reference value is itself `f32`-representable. A row whose last difference is numpy's or
LAPACK's own arithmetic stops at `tolerance` with the reason written down; that is a result, not a
failure. A row the reference cannot reproduce (FDP: two runs differ) is recorded, never chased.

Constraints that bind every sg job:
- Graphviz ports (`layout/graphviz/**`, `layout/radial/twopi.rs`, ...) are gated against Graphviz
  16.1.0 output (their registry `oracle` field). The user decided on 2026-09-30 that they must match
  Graphviz output. SciGraphs post-processes engine output (`yifan_hu.py:318-325`: centre on the
  mean, divide by the largest extent, times `scale`); that convention belongs in the conformance arm
  (`crates/graph-cli/src/oracle_python/conformance/motor.rs`), never inside a Graphviz port.
- A registered default changes only when the row needs it, with the reason in the doc; otherwise add
  a seeded or parameterised entry point and call it from `motor.rs`, the way `sfdp::run_seeded` is.
- Determinism (prompt.md §6): `libm` for transcendentals, no FMA, fixed-order sums, no `HashMap`.
  numpy's own `sin`/`cos` may differ from `libm` in the last ulp: such a row reaches the tolerance
  tier, not bitwise, and the doc says why.
- Edit only your row's lines in `table.rs` and the matrix; other sg jobs edit the neighbouring rows.
- House limits: 40 lines per function, 300 per file, 4 parameters; every heuristic carries `Ponytail:`.
- A new registry entry fills every `Metadata` field and passes `capabilities --check` and
  `codegen --check`.

Done when (in addition to the body): the merge floor is green (`fmt --check`, clippy `-D warnings`,
`cargo test --workspace --no-fail-fast`), `hashgate --seeds 8` exits 0 and its
`GM_MUTATE_REFERENCE_DEGREE=9` negative control exits non-zero, the conformance script exits 0 with
your re-pinned row and `--break` exits 1. Paste each command and its last lines.
