# Job fix-post-bundle (agent build, review repairs: edge bundling, styles, ink)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-post.md` (paths under
`crates/graph-core/src/`). Ids: R3 (BLOCKER), R12, R13, R14, R15, R16, R17, M1 … M10, M23 … M26.

Judgement notes:
- R3 (`segments` and `cycles` unvalidated in FDEB). Bound both in `check()` against the reference
  the registry names for FDEB, refusing with the existing error; each bound carries a `Ponytail:`.
- R15 … R17 (mingle and ink). The oracle the registry names for each post decides the expected
  value; quote it in the RED test's comment.

Paths: `crates/graph-core/src/post/fdeb.rs`, `post/fdeb/**`, `post/styles/**`, `post/mingle/**`,
`post/ink.rs`, `post/mod.rs`.

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with its degree control
non-zero; paste both.
