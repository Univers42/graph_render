# Job fix-post-routed (agent build, review repairs: routed edges and the grid index)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-post.md` (paths under
`crates/graph-core/src/`). Ids: R1, R2 (BLOCKER), R8, R9, R10, R11, M11 … M22.

Judgement notes:
- R1 (`GridCsr::row` renumbers the stencil slot). The review's evidence came from a scratch test
  file that was never committed (`zz_scratch_review`); write your own RED test: a border cell whose
  step costs must equal the stencil's step lengths. The fix moves routed output, which is the point:
  paste the routed post's differential or golden test before and after.
- R2 (`nx * ny` overflows `u32`). Refuse a cell count above a documented ceiling with the post's
  existing error type; the ceiling carries a `Ponytail:` line.

Paths: `crates/graph-core/src/post/routed.rs`, `post/routed/**`, `post/grid_index.rs`,
`post/grid_index/**`.

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with its degree control
non-zero; paste both.
