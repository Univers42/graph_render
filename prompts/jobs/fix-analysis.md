# Job fix-analysis (agent build, review repairs: centrality, communities, depth, paths)

Read `prompts/jobs/fix-common.md` first. Reviews: `docs/reviews/review-core-post.md` (paths under
`crates/graph-core/src/`; `capabilities/` is graph-cli) and `review-core-base.md` (F-34).
Ids: R5, R6 (BLOCKER), R18, R19, R20, R21, R25, M27, M28, M29, M30, M36, F-34 (F-34 and M36 are the
same line: one fix, `fixed-by`).

Judgement notes:
- R5 (closeness drops zero-distance reachable peers) and R25 (self-loop degree counted once): the
  declared oracle is networkx; pin its value in the RED test and cite the networkx line.
- R6 (inf/NaN reaches the JSON face). D9: a non-finite analysis value is refused or defined, never
  written. Fix it in the analysis (graph-core); `graph-wasm/src/analysis/report.rs` belongs to
  fix-wasm-abi, so a fix that needs it is "decisions needed".
- R20, M30 (a capability row's complexity understates the code). Either the code meets the row or
  the row states the real cost; `capabilities --check` must pass.

Paths: `crates/graph-core/src/analysis/**`, `crates/graph-cli/src/capabilities/analysis.rs`.

Done when, in addition to fix-common: `graph-cli capabilities --check` and `hashgate --seeds 8`
exit 0, the degree control non-zero; paste each.
