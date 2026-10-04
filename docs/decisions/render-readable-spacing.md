# The renderer scales a drawing by its local spacing, not its bounding box

Date: 2026-10-04. Code: `packages/graph-render/src/spacing.ts`, `readableFactor` in
`packages/graph-render/src/frame.ts`. Tests: `packages/graph-render/tests/spacing.test.ts`.

## The defect

`worldFactor` took the typical spacing as `sqrt(bbox area / n)`. One dense clump plus a few far
outliers inflates the box, so the clump is under-scaled; and a node's screen radius grows with
the zoom as fast as the gaps, so zooming never separates it. Test 2: 900 nodes at pitch 0.01
plus 100 on a circle of radius 10. The old factor is 88.54, a clump pitch of 0.885 world units
against a TARGET_SPACING of 56.

## The rule

`typicalSpacing(x, y)` grids each group's box into cells of about 16 nodes; a node's local
spacing is `side / sqrt(occ)` (`side / occ` on a line). A cell over 64 nodes is gridded again
over its own box, up to depth 4, its members gathered by a counting sort. The answer is the
median over a stride sample of at most 4096 nodes. O(n) time and memory, no randomness.

`readableFactor = min(max(TARGET_SPACING / spacing, floor), floor * MAX_SPREAD)`, with
`floor = worldFactor(bounds, n)` and `MAX_SPREAD = 32`; the floor alone when the spacing is 0.

- The floor: a drawing is never packed tighter than before. A 10 × 10 lattice reads 0.9, the
  floor's own spacing to the bit, so the factor is unchanged.
- The clamp: a clump on one point, or a near-point, cannot push the far nodes past 32 times
  where the floor put them. Test 2 hits it: the clump asks for 5929, gets 2833.4, pitch 28.33.
- A uniform random spread barely moves: 200 000 nodes give 1.031 times the floor.

## What it does not do

- The median serves the majority: a clump holding under half the nodes can still overlap.
- x and y only: a 3D drawing seen edge-on can still overlap.
- Live force frames arrive in motor units and bypass the factor (`drawnRadius` in
  `packages/graph-studio/src/state/keepForces.ts`).
- A box thinner than 1 / (16 m) of its length is gridded as a line, so a near-line's thin
  extent is ignored: the cell count stays O(m) instead of growing with the aspect ratio.

The escape hatch for all four is zooming in.
