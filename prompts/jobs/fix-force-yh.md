# Job fix-force-yh (agent build: review-layout-force LF-12, LF-16, LF-21, LF-23, LF-24)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-force.md` (ids `LF-NN`,
verified on `74f8994`; yifan-hu-octree and merge-p12-t4b have since touched `yifan_hu.rs`, so
confirm each defect first). The spec is `docs/layouts/layout.force.yifan_hu.md` and Hu (2005),
cited there.

1. **LF-12, MAJOR.** `yifan_hu/coarsen.rs` says "heaviest edge first" in its header; the code
   walks vertices in index order. Decide from the spec doc: implement heaviest-edge-first (RED: path
   `a-b-c-d`, weights 1, 10, 1 matches `b-c` only) or correct the header to the index-order greedy
   the `Ponytail:` already describes. Header and code must agree.
2. **LF-16, MAJOR.** `yifan_hu.rs` scales the de-coincidence offset by `link_distance`; at 0 every
   child lands on its parent. RED: `link_distance: 0.0` on a graph that coarsens a 2-child cluster.
   GREEN: validate `link_distance > 0` if the spec requires it, else a constant offset.
3. **LF-21, MINOR.** No prolongation extrapolation from the grandparent level and a fixed refine
   budget. Implement only if the spec doc asks for it; otherwise add the `Ponytail:` line recording
   the absence and what it costs.
4. **LF-24, MINOR.** `registry/force.rs`: `YIFAN_HU` inherits `FORCE_CEILING` (60 s budget, BH's
   46 781 ms) while its own 100 000-node measurement is 60.9 s. Give it its own ceiling from its own
   measurement, or restate the shared one as a node-count guard.
5. **LF-23, MINOR.** `registry/force.rs`: the `SPRING` row cites networkx `method='force'`, which
   the reference uses only below 500 nodes (L-BFGS above). Split the oracle claim by size band or
   lower the stated band; say which.

Paths: `crates/graph-core/src/layout/force/{yifan_hu.rs,yifan_hu/**}`,
`crates/graph-core/src/registry/force.rs` (row text and ceilings only),
`docs/layouts/layout.force.yifan_hu.md` (doc only), `docs/measurements/fix-force-yh.md`.

Done when: fix-common's done-when; every id above has a row; `capabilities --check` problems name no
`layout.force.*` row that was clean before.
