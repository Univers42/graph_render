# Job sg-igraph-dh (agent build, SciGraphs conformance: IGRAPH_DH)

Read `prompts/jobs/sg-common.md` first. Needs `sg-igraph-dims` landed (the fit in the
conformance arm, the corrected `G_IGRAPH_SEED`). Row: `IGRAPH_DH`. Target: the motor's
distribution inside igraph's, not bytes.

Why not bytes. Davidson-Harel draws random numbers throughout: the start, the node order of
every round, each candidate move's angle. python-igraph routes those draws through Python's
`random`, but the integer draws and the shuffle go through igraph's own RNG wrapper, and
reproducing it is reproducing igraph's RNG (layouts-igraph.md rule 4). So the two arms run the
same energy from different random streams, and the row stays `rng`.

Facts:
- SciGraphs calls `layout_davidson_harel` with maxiter=10, fineiter=0, cool_fact=0.95,
  weight_node_dist=1, weight_border=0, weight_edge_lengths=1, weight_edge_crossings=1,
  weight_node_edge_dist=1, then `_igraph_fit_positions` (`igraph_layouts.py:461-491`). These
  equal `DhParams::default()` (`davidson_harel.rs`); pin that in a test.
- Shape ratio on lesmis (small / large covariance eigenvalue of the 77 points, 1 = round):
  motor 0.70, igraph 0.77 (measured 2026-10-02 on develop daebb36).
- The p12 gate's worst stress ratio is 51.91 against a tight ceiling of 10
  (`docs/measurements/p12-igraph-ceilings.md:54-75`); the doc calls it a worse local optimum
  from a different draw. That is an explanation nobody measured.
- Spec-level differences the motor carries (`docs/layouts/layout.force.davidson_harel.md`):
  the self-loop skip, the squared-distance floor `MIN_D2 = 1e-12` (`davidson_harel/energy.rs:7`),
  the angle rounding. Each is stated in the spec or is a defect; check each against the spec text
  (never igraph's C) and write which.

Do:
1. Measure the row; paste its metrics line and both shape ratios.
2. Distribution, lesmis and the conformance fixtures, K = 32 seeds per arm: the motor through
   `DhParams { seed: k, ..default }`; igraph in `ge-python-oracle` with `random.seed(k)` before
   each call, SciGraphs' parameters. Both arms fitted the same way. Per arm and fixture: shape
   ratio, normalised stress at the optimal scale (reuse `oracle-igraph.py`'s function, do not
   write a second one), edge crossings, min pairwise distance over the extent. Paste min /
   median / max per metric. A new harness script follows `oracle-igraph.py`'s manifest and
   sha256 pattern.
3. The p12 tail: find the seed and graph behind 51.91, run both arms on it, and say what
   differs (a component, a coincidence, a crossing count, the start). If it is a defect against
   the spec: RED test, GREEN fix, re-run `oracle-igraph`, paste the before/after worst. If it is
   the search, show the igraph distribution over K seeds on that graph reaching the same tail.
4. The conformance arm passes `seed: LAYOUT_SEED` through a `DhParams` override (as `fa2` does)
   so the row is reproducible from the arm's own constant; re-pin only if it moved.

Paths: `layout/force/davidson_harel*` (fixes only), a harness script under `harness/`, the
conformance `motor.rs` and its tests, `baseline/table/igraph.rs` (IGRAPH_DH), the doc's row,
`docs/measurements/p12-igraph-ceilings.md` (the 51.91 paragraph, measured),
`docs/measurements/sg-igraph-dh.md`.

Done when: sg-common's done-when, the distribution table pasted, the motor's median inside
igraph's [min, max] on every metric and fixture or each miss named with its cause, and the
51.91 paragraph rewritten from a measurement.
