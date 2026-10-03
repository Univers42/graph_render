# Job sg-propose-classify (agent build, SciGraphs conformance: the cause classifier)

Your worktree is cut from `sg-spring-seed`, which is not on develop yet; do not merge develop.

Read `prompts/jobs/sg-common.md` first. Source: `docs/measurements/sg-spring-seed.md`, section
"`SPRING`'s recorded cause is wrong, and it is pinned anyway" (option 2, the recommended one).

`harness/scigraphs-conformance/sc_propose.py` calls a row `arithmetic` only when the shape agrees
(`disparity <= 1e-3`) and the raw gap is `<= ARITHMETIC_GAP` (1e-6). `SPRING` is exact on 22 of 23
fixtures and off by 2.37e-03 on one, with disparity 3.77e-16, so it falls through to `convention`,
a label no scale, centre or axis fix can act on.

1. **RED.** Write a test in the harness's own style (none exists for `sc_propose.py`: add
   `harness/scigraphs-conformance/test_sc_propose.py`, stdlib `unittest`, run inside
   `ge-python-oracle` the way `scripts/scigraphs-conformance.sh` runs the harness) that feeds
   `classify` the `SPRING` entry from the report and expects `arithmetic`, plus one row that must
   stay `convention` (a real scale gap: every fixture off by the same factor) and `GRID`'s
   (disparity 5e-32, gap 4.0) which must not become `arithmetic`. See it fail.
2. **Widen the rule.** A row whose shape agrees on every fixture and whose raw gap is above
   `ARITHMETIC_GAP` on at most one fixture (or on a stated fraction — pick the smallest rule that
   separates the three cases, and say why in the `#:` comment) is `arithmetic`. Keep the order
   `rng` before `convention`. The rule is a heuristic: it carries a `Caveat:` line naming the input
   it misreads.
3. **Re-propose.** Run `scripts/scigraphs-conformance.sh`, and transcribe only the cause columns that
   changed from `conformance-baseline-proposed.rs` into
   `crates/graph-cli/src/oracle_python/conformance/baseline/table/*.rs`. List each changed row with
   its old and new cause in `docs/measurements/sg-propose-classify.md`.

Paths: `harness/scigraphs-conformance/sc_propose.py`, `harness/scigraphs-conformance/test_sc_propose.py`,
`crates/graph-cli/src/oracle_python/conformance/baseline/table/*.rs` (cause fields only),
`docs/measurements/sg-propose-classify.md`, and the `sc_propose` paragraph of
`docs/measurements/scigraphs-conformance.md`.

Done when: the new test passes and failed before the change, `scripts/scigraphs-conformance.sh` and
its `--break` control behave as before, fmt/clippy/test green, and no row's f32/f64 counts moved.
