# Job review-core-post (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). Layouts are covered by
the `scigraphs-conformance` job and a 2026-10-01 layout review; post, analysis and scale are not.

Scope: `crates/graph-core/src/post/**` (FDEB, mingle, routed, ink, styles, grid_index),
`crates/graph-core/src/analysis/**` (centrality, communities, components, depth, paths),
`crates/graph-core/src/scale/**` (simple, adaptive, lod, simplify), and their rows in
`crates/graph-cli/src/capabilities/`.

Check, per function: correctness against its stated reference (the oracle named in its capability row,
the paper or library it cites; references on disk are listed in `prompts/REFERENCES.md`, pinned under
`$GM_SCRATCH/refs`); D1-D10 determinism (libm, fixed-order sums, no HashMap, u32/u64 on the wire);
degenerate inputs (0 nodes, 1 node, no edges, self-loops, multi-edges, disconnected, NaN/inf weights);
complexity against the row's declared complexity; duplicated primitives (the same arithmetic twice).

Rules for every finding: `file:line`, severity BLOCKER/MAJOR/MINOR, the concrete failing input, and evidence: a command with its output (a scratch test you ran and did not commit counts), or the reference `file:line` the code disagrees with. A finding without evidence is listed under "unverified", never as a finding. No style nits without a house rule (`CLAUDE.md` "House limits", `prompt.md` §6 D1-D10). Fan out first: one `explore` subagent per module in ONE message; you merge, deduplicate and verify.

Output: `docs/reviews/review-core-post.md`: a table `id | severity | file:line | defect | failing input |
evidence | proposed fix`, sorted by severity, then the unverified list, then the counts.

Paths: `docs/reviews/review-core-post.md` only. No code change.

Done when: every module in scope is named in the report with its finding count (0 is a valid count);
every BLOCKER/MAJOR row has a command and its output or a reference `file:line`.
