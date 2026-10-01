# Job sg-grid-scale (agent build, SciGraphs conformance: GRID)

Read `prompts/jobs/sg-common.md` first. Row: `GRID` (cause `convention`, same shape to 5.5e-32,
max gap 4.0, f32 342/1020).

Why: SciGraphs `_grid_layout` (`basic.py:11-20`): `grid_size = ceil(sqrt(n))`, node i at
`x = (i % grid_size) * scale / grid_size`, `y = (i // grid_size) * scale / grid_size`, `z = 0`,
starting at the origin. graph-core `layout/grid.rs`: same columns, but pitch
`GridParams::spacing` (f32, default 1.0) and the lattice centred on the origin
(`grid.rs:11-13`). Same lattice, different pitch and origin.

Do:
1. RED: a test that `grid::run_with` at the SciGraphs parameters gives, for n = 5
   (grid_size 3), x = 0, 5/3, 10/3, 0, 5/3 and y = 0, 0, 0, 5/3, 5/3, computed as
   `((i % g) as f64 * scale) / g as f64` (multiply first, then divide: that is numpy's order).
2. GREEN: the smallest parameter change that expresses SciGraphs' placement without changing
   the registered default (prefer a `GridParams` field such as an origin flag plus a pitch the
   conformance arm sets to `scale / grid_size`; if the pitch must be computed per coordinate in
   f64 to match, do that in the new path only). The registered `run` keeps its bytes; check with
   `hashgate --seeds 8`.
3. Conformance `motor.rs` calls the new path for `layout.grid` at `scale = 5.0`. Close
   `G_GRID_SCALE`; `G_GRID_ITER` stays.

Paths: `crates/graph-core/src/layout/grid.rs`, the conformance `motor.rs`, `rows.rs`, `gaps.rs`,
`baseline/table.rs` (row GRID), doc row 2 and repair 1, `docs/measurements/sg-grid-scale.md`.

Done when: GRID reaches f32 1020/1020 (paste the metrics line); the sg-common done-when holds.
