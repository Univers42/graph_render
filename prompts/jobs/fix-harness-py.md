# Job fix-harness-py (agent build, review repairs: the Python oracle arms)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-harness-sdk.md` (ids `B`, `M`,
`m`, `U`). Ids: B3, B4 (BLOCKER); M15-M23, M26-M29, M37 (MAJOR); m44-m60, m62-m71, m77, m106-m112
(MINOR); U5, U7. The other ids are owned by `fix-sdk` and `fix-harness-mjs`.

These arms judge the motor, so a fix must make an arm stricter, never looser: no ceiling raised, no
case dropped. Each arm runs in its pinned image (`ge-python-oracle`, `ge-graphviz-oracle`, the
Dockerfile header of each says how). U5 and U7 were read, not run: run each before you fix it.

RED for a Python arm: the reproducing command from the finding's evidence cell, run on the current
code, then on the fix (for B3, truncate `twopi.jsonl` to one seed: the current arm passes, the
fixed one refuses). A `test_*.py` beside the arm is the test when one exists.

Judgement notes:
- B3: the seven siblings already recompute the digest and refuse a mismatch. Copy their shape;
  if the same few lines would now live in nine arms, extract them into one shared module under
  `harness/` and call it from every arm you touch.
- B4, and every other "empty set exits 0" finding: refuse an empty case set with a non-zero exit
  and a message naming the file.
- Shared files: `fix-gates-oracles` owns RG-22 in `harness/gv_plain.py` (`write_dot`, :37-47) and
  `fix-gates-conformance` owns RG-16 in `harness/oracle-spring.py` (the fixture-digest echo). Fix
  your ids in those files without touching those lines.
- M27: the Rust half (`crates/graph-cli/src/oracle_python/fa2.rs:44`) is in your paths for M27 only.

Paths: `harness/*.py` (the arms, `gv_*.py`, `perturb-*.py`, `fa2-chaos.py`, `scigraphs_lesmis_*.py`,
`emit-scigraphs-lesmis.py`, `test_*.py`), a new shared module under `harness/`,
`crates/graph-cli/src/oracle_python/fa2.rs` (M27 only), `docs/measurements/fix-harness-py.md`.
Never `harness/scigraphs-conformance*` (fix-gates-conformance).

Done when, in addition to fix-common: every arm you touched passes on its committed fixtures through
its `graph-cli oracle-*` verdict (paste each), and refuses its finding's reproducer (paste each);
`scripts/scigraphs-conformance.sh` exits 0 with no row moved.
