# Shared rules for every `fix-*` repair job (code-review findings)

Read this before the job body; the body names its finding ids, paths and done-when.

Source. The findings are in `docs/reviews/review-core-base.md` (ids `F-NN`),
`docs/reviews/review-core-post.md` (`RNN` and `MNN`, paths relative to `crates/graph-core/src/`
unless they start with `capabilities/`, which is `crates/graph-cli/src/capabilities/`) and
`docs/reviews/review-studio.md` (`ST-NN`). Precondition: the review the body names is on this tree.
If it is absent, stop and report under "decisions needed".

A review finding is a hypothesis, not a spec. Its "proposed fix" is the reviewer's guess. Where it
disagrees with `docs/contract/`, `prompt.md` §6 (D1–D10), a `docs/decisions/` record or the
oracle the registry names, those win; say so in the finding's row.

The loop, per finding id the body names:
1. Open the cited `file:line` and confirm the defect still exists on this tree (lines move).
2. RED: a test that fails on the current code because of exactly this defect (unit test next to
   the module, or the crate's `tests/`). Paste the failing run, filtered by test name.
3. GREEN: the smallest change that turns it green. Paste the passing run.
4. A finding whose test cannot fail on this tree is recorded `false` with the test as evidence,
   and the test stays (it pins the behaviour the reviewer doubted). A finding fixed by an earlier
   finding's change is recorded `fixed-by <id>`.
5. A MINOR that is only a naming, comment or doc fix needs no test; say `doc-only`.

Constraints:
- Public surface stays backward compatible: the wire format (`docs/contract/`), the wasm ABI, the
  SDK's exports and every `pub` item another crate uses. A breaking fix is additive (new function,
  new error variant, old one kept and documented) or it is a stop: "decisions needed".
- A change that moves a registered layout, post, analysis or scale output is allowed only to fix the
  finding, and the job pastes `hashgate --seeds 8` (exit 0) and its `GM_MUTATE_REFERENCE_DEGREE=9`
  control (non-zero). `scripts/scigraphs-conformance.sh` must still exit 0: a conformance row that
  moves is a regression to report, never a row to re-pin from a fix job.
- Determinism (§6): `libm`, no FMA, fixed-order reductions, no `HashMap`, wire integers `u32`/`u64`.
- House limits: 40 lines per function, 300 per file, 4 parameters; every heuristic, clamp or ceiling
  you add carries a `Ponytail:` line naming what it gets wrong.
- Edit only the paths the body lists, plus new test files beside them. Another fix job owns every
  other path; a fix that needs one is "decisions needed" with the id.
- The review's "unverified" items in your paths: check each; promote it to a finding with a RED test
  or record it `false`.

Report `docs/measurements/<job label>.md`: one table, one row per id:
`id | severity | verdict (fixed / false / fixed-by / doc-only / deferred) | test name | file:line`.
A `deferred` row names the reason and the decision it waits on. Then the commands and last lines.

Done when (in addition to the body): every id in the body has a row; the merge floor is green
(`scripts/orch/gr cargo fmt --all --check`, clippy `-D warnings`, `cargo test --workspace
--no-fail-fast`), `cargo build -p graph-core --target wasm32-unknown-unknown` builds, and
`scripts/scigraphs-conformance.sh` exits 0. Paste each command and its last lines.
