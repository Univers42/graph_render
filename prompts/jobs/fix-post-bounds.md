# Job fix-post-bounds (agent build, follow-up to fix-post-bundle)

Read `prompts/jobs/fix-common.md` first. Source: the "decisions needed" items 7 and 8 of
`docs/measurements/fix-post-bundle.md`. Ids: `PB-7`, `PB-8` (record them under those names).

PB-7, FDEB `iterations` unbounded (`crates/graph-core/src/post/fdeb.rs`, `FdebParams::iterations`
and `check`). `cycles` and `segments` are refused past the reference panel's ceilings
(`MAX_CYCLES`, `MAX_SEGMENTS`); `iterations` is not, so one parameter buys unbounded work.
Find the reference panel's own max for `bundle_iterations` in
`SciGraphs/properties/edge_style_properties.py` (cite `file:line`; if the panel has none, stop and
report under "decisions needed"). Add `MAX_ITERATIONS` beside the other two consts, with the same
`Ponytail:` shape, and refuse past it in `check` with `StageError::Param`. RED: a test that
`iterations = MAX_ITERATIONS + 1` is refused. The registered default must not move.

PB-8, the ink walk is bounded only by the box width (`crates/graph-core/src/post/ink.rs`,
`Raster::mark` and `Raster::steps`). An edge point far outside the nodes' bounding box makes the
half-cell walk as long as the segment: a box of 1 with a point at 1e7 is about 2.56e9 steps.
Clip the segment to the raster's box before walking (Liang–Barsky: one function, its own unit
test), and walk only the clipped part. The part outside the box used to mark the clamped border
cells; say in the module doc that it no longer does, and why. `length` stays the full segment's.
RED: a test with a 1x1 box and one edge point at `1e7` that finishes and marks at most
`2 * INK_RESOLUTION` cells; and a test that a segment fully inside the box marks exactly the cells
it marked before (compare against the unclipped walk on the same input).
Check whether any ink figure is pinned (`git grep -n 'cells' -- docs/measurements/phase08-ink.md
crates/`): a pinned number that moves is reported with its before/after, never silently re-pinned.

Paths: `crates/graph-core/src/post/fdeb.rs`, `post/fdeb/**`, `post/ink.rs`, `post/ink/**`,
`docs/measurements/fix-post-bounds.md`.

Done when: fix-common's done-when, with both ids in the report table.
