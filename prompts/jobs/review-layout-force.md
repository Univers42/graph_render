# Job review-layout-force (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). The `scigraphs-conformance` job compares outputs; nobody has read this code against its references, its degenerate inputs or D1-D10.

Scope: `crates/graph-core/src/layout/force/**` (spring, spring3d, barnes_hut, quadtree, fruchterman_reingold, kamada_kawai, davidson_harel, drl, graphopt, lgl, yifan_hu, session, params), `layout/forceatlas2*`, `layout/spectral*`, `layout/spectral_stage.rs`, `layout/pivot_mds*`, and their rows in `crates/graph-core/src/registry*`.

Rules, checks and output shape: exactly as `prompts/jobs/review-core-post.md` says (read it first),
with `docs/reviews/review-layout-force.md` as the report and the only path you write. igraph licence: never open igraph's C sources; an igraph-derived layout is checked against igraph's documented behaviour and its Python outputs (`harness/oracle-igraph.py`), never its code. A finding the conformance report already names (`docs/measurements/scigraphs-conformance.md`) is cited by its row, not re-reported.
Review `origin/develop` as it is when you start; fan out one `explore` subagent per module in ONE
message, then merge, deduplicate and verify each finding yourself.

Done when: every module in scope is named in the report with its finding count (0 is a valid count);
every BLOCKER/MAJOR row has a command and its output or a reference `file:line`.
