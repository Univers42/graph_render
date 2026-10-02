# Job review-layout-rest (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). The `scigraphs-conformance` job compares outputs; nobody has read this code against its references, its degenerate inputs or D1-D10.

Scope: `crates/graph-core/src/layout/{basic_3d,bipartite,circular,coords,planarity,random,spiral}/**` and their `.rs` parents, `layout/grid.rs`, `layout/adjacency.rs`, `layout/mod.rs`, and their rows in `crates/graph-core/src/registry*`.

Rules, checks and output shape: exactly as `prompts/jobs/review-core-post.md` says (read it first),
with `docs/reviews/review-layout-rest.md` as the report and the only path you write. A finding the conformance report already names (`docs/measurements/scigraphs-conformance.md`) is cited by its row, not re-reported. Graphviz (`layout/graphviz/**`) is out of scope: the `sg-graphviz-*` jobs are rewriting it.
Review `origin/develop` as it is when you start; fan out one `explore` subagent per module in ONE
message, then merge, deduplicate and verify each finding yourself.

Done when: every module in scope is named in the report with its finding count (0 is a valid count);
every BLOCKER/MAJOR row has a command and its output or a reference `file:line`.
