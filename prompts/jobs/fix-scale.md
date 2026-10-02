# Job fix-scale (agent build, review repairs: level of detail and simplification)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-post.md` (paths under
`crates/graph-core/src/`). Ids: R4, R7 (BLOCKER), R22, R23, R24, M31 … M35.

Judgement notes:
- R4 (`label_budget == 0` yields one label). The declared oracle is SciGraphs' engine (the review
  cites it); pin "no limit" for a non-positive budget only if that is what the reference does,
  quoted in the test.
- R7 (chain contraction names nodes `fold_leaves` already hid). RED: a graph with a leaf at a
  chain's end, asserting every link endpoint is visible and is its own representative.

Paths: `crates/graph-core/src/scale/**`.

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with its degree control
non-zero; paste both.
