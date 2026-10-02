# Job fix-force-igraph (agent build: review-layout-force LF-01, LF-03, LF-06, LF-07, LF-19, LF-27)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-force.md` (ids `LF-NN`,
verified on `74f8994`). merge-p12-t4b and sg-igraph-dims rewrote parts of these modules since: step 1
of fix-common (confirm the defect on this tree) matters here, and a finding already fixed is
recorded `fixed-upstream` with the commit. igraph licence: never open igraph C sources; the spec is
`docs/layouts/layout.force.*.md` and igraph's documented behaviour.

1. **LF-01, BLOCKER.** `graphopt.rs` `.clamp(-cap, cap)` panics on a negative `max_sa_movement`.
   RED: `GraphoptParams { max_sa_movement: -1.0, .. }` on 2 nodes returns `Err`, never panics.
   GREEN: validate the parameters at the top of `run` (`max_sa_movement` finite and `>= 0`,
   `node_mass != 0`, `spring_length` finite) with a `StageError`.
2. **LF-03, MAJOR.** `davidson_harel.rs` `bounding` seeds the maxima with `f64::MIN`, and `grow`'s
   `else if` can leave the sentinel, so the fine radius goes to `-1.797e308`. RED: a scatter whose
   first point is the extreme on an axis (find a seed/size that hits it). GREEN: seed the bounds from
   the first point.
3. **LF-06, MAJOR.** `graphopt.rs` builds springs from `topology.edges()` and skips
   `simple_graph`, so parallel edges count twice. RED: 2 nodes, 2 parallel edges, compared with the
   same graph with one edge. GREEN: `super::simple_graph(topology)`.
4. **LF-07, MAJOR.** `drl/density.rs` floors a coincident pair with `+ 1e-50`, adding `1e46` to the
   repulsion. RED: two coincident nodes through `switch_to_fine` produce a finite `f32` output or a
   `StageError`, never `±inf` on the wire. GREEN: skip or bound the term below a minimum distance,
   as `barnes_hut/charge.rs` does with `dmin2`.
5. **LF-19, MINOR.** `lgl/grow.rs` with an isolated root returns a random scatter. RED: 5 nodes,
   edge `(0,1)`, `root = Some(4)`. GREEN: per the spec doc (centre the root and grow, or refuse).
6. **LF-27, MINOR.** Five stages check finiteness on `f64` and then narrow with `as f32`
   (`spring3d.rs`, `forceatlas2.rs`, `yifan_hu.rs`, `fruchterman_reingold.rs`, `graphopt.rs`). Add
   one shared helper in `layout/force/` that narrows and refuses a value that is not finite after
   narrowing, and call it at the five sites (one-line edits outside graphopt). RED: the helper on
   `[1e39]` refuses.
7. **LF-26 (graphopt part).** `graphopt/tests.rs` asserts `gap(&moved) > gap(&start)` on
   `graph(2, &[])`, which any motion satisfies; make it fail for the defect it names.

Paths: `crates/graph-core/src/layout/force/{graphopt.rs,graphopt/**,davidson_harel.rs,davidson_harel/**,drl.rs,drl/**,lgl.rs,lgl/**}`,
a new helper module under `crates/graph-core/src/layout/force/` (and its `mod` line), the one
narrowing line in each of `spring3d.rs`, `forceatlas2.rs`, `force/yifan_hu.rs`,
`force/fruchterman_reingold.rs`, `docs/measurements/fix-force-igraph.md`.

Done when: fix-common's done-when; every id above has a row; `scripts/scigraphs-conformance.sh`
exits 0.
