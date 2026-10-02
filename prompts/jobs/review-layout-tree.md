# Job review-layout-tree (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). The `scigraphs-conformance` job compares outputs; nobody has read this code against its references (d3-hierarchy, dagre, the Sugiyama papers in `prompts/REFERENCES.md`), its degenerate inputs (cycles, forests, a single node, deep chains) or D1-D10.

Scope: `crates/graph-core/src/layout/{hierarchy,tidy_tree,sugiyama,hierarchical_3d,radial,treemap,circle_packing}/**` and their `.rs` parents, and their rows in `crates/graph-core/src/registry*`.

Rules, checks and output shape: exactly as `prompts/jobs/review-core-post.md` says (read it first),
with `docs/reviews/review-layout-tree.md` as the report and the only path you write. A finding the conformance report already names (`docs/measurements/scigraphs-conformance.md`) is cited by its row, not re-reported. Complexity matters here: an O(n²) step on a path the studio runs at 1M nodes is MAJOR.
Review `origin/develop` as it is when you start; fan out one `explore` subagent per module in ONE
message, then merge, deduplicate and verify each finding yourself.

Done when: every module in scope is named in the report with its finding count (0 is a valid count);
every BLOCKER/MAJOR row has a command and its output or a reference `file:line`.
