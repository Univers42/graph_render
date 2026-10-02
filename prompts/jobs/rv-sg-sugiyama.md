# Job rv-sg-sugiyama (agent build, review only, docs)

Review the sg-sugiyama branch before it lands. This worktree is that branch; its delta is
`git diff origin/develop...HEAD` (18 files: `layout/sugiyama/{acyclic,acyclic/feedback,layering,
scaled,stages,stages/dump}*`, the conformance motor/rows/gaps, `docs/decisions/sugiyama-heuristics.md`,
`docs/measurements/{sg-sugiyama,scigraphs-conformance}.md`). Brief it answered: `prompts/jobs/sg-sugiyama.md`.
Reference: `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py` (in the worktree).

Rules, checks and output shape: as `prompts/jobs/review-core-post.md` says (read it first), with
`docs/reviews/rv-sg-sugiyama.md` as the report and the only path you write. Check in particular:

1. `layout.dag.sugiyama`'s public output: the brief allowed changing it only if the dagre crossing
   counts in `docs/decisions/sugiyama-heuristics.md` stay green. Run `hashgate --seeds 8` and say
   whether stage hashes moved, and if so whether the decision doc and the registry row say why.
2. The FAS rewrite (`acyclic.rs` -> `acyclic/feedback.rs`) against `hierarchical.py`'s
   `_acyclic_arcs` line by line: heap key, tie-break, self-loops. The pre-branch code was verified
   to match (`docs/reviews/review-layout-tree.md` "Checked and found correct"); say whether the
   rewrite still does.
3. `scaled.rs` / `run_scaled` against `hierarchical.py:679-685`, and that `stages/dump.rs` is test or
   gate instrumentation only (not reachable from the product path, no I/O in graph-core).
4. Complexity: no O(n^2) step on a 1M-node path; house limits (40-line functions, 300-line files).
5. Run `scripts/scigraphs-conformance.sh` and paste the `SUGIYAMA` line; it must match the
   measurement doc's result (597/1020 f64, 1020/1020 f32).
6. The worker's open question (whether registry `Metadata` text should be reworded off the greedy
   FAS): answer it from the code.

End the report with a verdict line `VERDICT: LAND` or `VERDICT: FIX` and, for FIX, a numbered list
of what to change with `file:line`.

Done when: the report has every check above with its command output or `file:line`, and a verdict.
