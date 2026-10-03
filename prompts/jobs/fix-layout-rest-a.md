# Job fix-layout-rest-a (agent build, review repairs: spiral, circular, coords, planarity, layout root)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-layout-rest.md` (ids `LR-NN`,
paths relative to `crates/graph-core/src/`).
Ids: LR-01 (MAJOR); LR-12 (the `layout/circular.rs:3` half only), LR-13 … LR-19, LR-24 … LR-30,
LR-34 … LR-40 (MINOR). The 3-D, grid and random ids are job `fix-layout-rest-b` (it waits for
sg-spiral3d, sg-mt19937 and sg-grid-minors to land).

Judgement notes:
- LR-01 (`spiral.rs`, `resolution = 0` returns NaN coordinates with `Ok`): the contract at
  `spiral.rs:43` decides. A parameter that cannot produce finite geometry is refused with an error
  value (the function already returns a `Result`), never clamped silently. RED: a test that runs
  `resolution = 0` and asserts the error; it fails today because the output is `Ok` with NaN.
- A refusal or a clamp that changes no registered default moves no hash; prove it with
  `hashgate --seeds 8` (same per-stage hashes before and after for every id this job touches).
- Planarity (LR-24 … LR-30): an `expect`/index that the caller guarantees gets the message naming
  the guarantee; one the caller does not guarantee becomes an error value.
- LR-39, LR-40 (spiral tests): a test that cannot fail is rewritten so it can, and shown failing
  once against a mutated input inside the test.

Paths: `crates/graph-core/src/layout/{spiral.rs,spiral/**,circular.rs,circular/**,coords.rs,
planarity.rs,planarity/**,mod.rs,tests.rs,adjacency.rs}`. Never `layout/basic_3d/**`,
`layout/grid*`, `layout/random*` or `registry/**`.

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with the same per-stage hashes
as the untouched tree, and its `GM_MUTATE_REFERENCE_DEGREE=9` control exits non-zero; paste both.
