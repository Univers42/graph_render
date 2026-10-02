# Job fix-layout-rest-b (agent build, review repairs: basic 3-D, grid, random)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-layout-rest.md` (ids `LR-NN`,
paths relative to `crates/graph-core/src/`).
Ids: LR-02 … LR-05 (MAJOR); LR-06 … LR-11, LR-12 (the `registry/hierarchy.rs:119` half),
LR-20 … LR-23, LR-31 … LR-33 (MINOR). Starts after sg-spiral3d, sg-mt19937 and sg-grid-minors are
on develop: they rewrote these files, so re-read every cited line first; an id their change already
closed is `fixed-by <job>`.

Judgement notes:
- LR-03 (`basic_3d.rs:41`, `SCALE` fixed at 5.0 while the studio's `layout_scale` slider spans
  0.1-100): the doc claim "no caller passes anything else" is false. Either thread the scale through
  (a `run_scaled` entry point, the way `layout/grid/scaled.rs` does, registered defaults unchanged so
  no hash moves) or correct the claim and name the slider's effect in a `Ponytail:` line. Prefer the
  entry point if it stays under the house limits; say which in the report.
- LR-02 (`grid/scaled.rs`, a huge scale writes `inf`): refuse a scale whose output is not finite,
  with an error value; RED test at the first scale that overflows `f32`.
- LR-04: name a path that exists (`git grep` the constant), doc-only.
- LR-05 (`cube/tests.rs`, the seed test never varies the seed): two seeds must give different
  bytes and one seed equal bytes; show the old test passing against a seed-ignoring mutant.
- SciGraphs rows (`scripts/scigraphs-conformance.sh`) must not move: RANDOM, CUBE, GRID and SPHERE
  are `f32`-identical on develop.

Paths: `crates/graph-core/src/layout/{basic_3d.rs,basic_3d/**,grid.rs,grid/**,random.rs,random/**}`,
`crates/graph-core/src/registry/three_d.rs` (LR-04 line only), `registry/hierarchy.rs` (LR-12 line
only).

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with the same per-stage hashes
as the untouched tree for every registered id, and its `GM_MUTATE_REFERENCE_DEGREE=9` control exits
non-zero; the conformance script exits 0 with no row moved.
