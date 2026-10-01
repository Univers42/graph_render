# Job sg-spiral3d (agent build, SciGraphs conformance: SPIRAL_3D)

Read `prompts/jobs/sg-common.md` first. Row: `SPIRAL_3D` (cause `algorithm`, disparity 0.585:
grey is a 3D conical spiral, green collapses to the centre).

Why: the row maps to `layout.spiral`, graph-core's planar Archimedean spiral (networkx
`spiral_layout`, `resolution = 0.35`, which is networkx's own default:
`networkx/drawing/layout.py:1247` in `$GM_SCRATCH/refs/networkx-3.6`). SciGraphs' `SPIRAL_3D` is not
networkx's: `dispatcher.py:105` calls its own `_spiral_layout_3d(num_nodes, scale)`
(`basic.py:36-62`), a conical spiral climbing from radius `scale*0.5` to `scale` and from
`z = -scale` to `scale`, spaced evenly along its arc length. The conformance doc's repair 10 (raise
`resolution` to 1.0) is wrong on both counts: rewrite it.

The reference, in order (port each step exactly):
- `turns = max(2, int(round(sqrt(n / (0.75*pi)))))` (Python `round` = half-to-even);
  `omega = 2*pi*turns`;
- `grid = linspace(0, 1, 65536)`; `speed = sqrt((0.5s)^2 + (0.5s(1+grid)omega)^2 + (2s)^2)`;
  `step = grid[1] - grid[0]`; `length = [0, cumsum(0.5*(speed[1:]+speed[:-1])*step)]` (cumsum is
  sequential, not pairwise);
- `wanted = linspace(0, length[-1], n)` (n > 1) else `[0.5*length[-1]]`;
  `t = interp(wanted, length, grid)`;
- `x = r cos(t omega)`, `y = r sin(t omega)`, `z = s(2t - 1)`, `r = s*0.5*(1+t)`.
numpy's `linspace` is `start + i*step` with a final-point fix-up, and `interp` has its own slope
formula (`numpy/_core/src/multiarray/compiled_base.c`): read both and port them, citing lines.

Do:
1. RED: a graph-core test pinning `t` and the three coordinates for n = 1, 2 and 7 at s = 5.0,
   values computed in numpy inside `ge-python-oracle` and pasted as hex.
2. GREEN: a new layout `layout.basic3d.spiral` (or the id the basic_3d family's naming gives) next
   to `sphere`/`helix`/`cube`, through `basic_3d`'s `in_space`, with a full registry `Metadata`
   entry (additive edits to `registry.rs`, `capabilities`, codegen outputs). Leave
   `layout.spiral` untouched. `libm` `sin`/`cos`/`sqrt` (D1).
3. Map `SPIRAL_3D` in `conformance/rows.rs` to the new id; close `G_NO_ITERATIONS` and
   `G_SNAPSHOT_SCALE` for the row if they no longer apply.

Caveat: numpy's `sin`/`cos` come from the platform libm (or SIMD loops) and may differ from `libm`
in the last ulp; most such differences vanish in the f32 narrowing. If some do not, the row stops
at `tolerance` and the doc says so with the count.

Paths: `crates/graph-core/src/layout/basic_3d/**`, `registry.rs` (additive), generated contract
files from `graph-cli codegen`, the conformance `rows.rs`, `gaps.rs`, `baseline/table.rs` (row
SPIRAL_3D), doc row 14 and repair 10, `docs/measurements/sg-spiral3d.md`.

Done when: SPIRAL_3D reaches f32 1020/1020 or `tolerance` with the residual counted;
`capabilities --check` and `codegen --check` pass; the sg-common done-when holds.
