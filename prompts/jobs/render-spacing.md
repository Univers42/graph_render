# Job render-spacing (agent build: the renderer scales a drawing by its local spacing, not its bounding box)

Why: the user reports drawings where many nodes sit "all compressed in the same space" and cannot be
read, at any zoom. Root cause (develop `f9677daa`):
- `packages/graph-render/src/frame.ts:89-97` `worldFactor` takes the typical spacing as
  `sqrt(bbox area / n)`. One dense clump plus a few far outliers inflates the box, so the clump is
  under-scaled: its nodes are closer than a node is wide.
- A node's screen radius is its world extent times `camera.scale` (`canvas2d/nodes.ts:46`), so
  zooming in grows the nodes as fast as the gaps. The overlap never goes away.
- The existing `Ponytail:` on `worldFactor` names exactly this failure.

## Contract

New module `packages/graph-render/src/spacing.ts` (≤ 150 lines, functions ≤ 40 lines, ≤ 4 params):

```ts
/** The spacing most nodes have to their neighbours, in motor units; 0 when it cannot be told. */
export function typicalSpacing(x: Float32Array, y: Float32Array): number;
```

Algorithm (O(n) time, O(n) memory, deterministic, no `Math.random`, no sort of all n values):
1. Every node gets a local spacing `s[i]`, from the occupancy of a uniform grid over its group's
   bounding box. The first group is all nodes.
2. For a group of `m` nodes with box `w × h`:
   - If `w * h > 0`: cell side `side = sqrt(w * h * PER_CELL / m)` with `PER_CELL = 16`.
     A node in a cell holding `occ` nodes gets `s = side / sqrt(occ)`.
   - If the group is collinear (exactly one of `w`, `h` is 0): `side = extent * PER_CELL / m` and
     `s = side / occ`.
   - If `w === 0 && h === 0`, every node of the group gets `s = 0`.
   - Columns `cols = max(1, ceil(w / side))`, rows likewise. A node on the max edge clamps into the
     last cell. Count with a `Uint32Array(cols * rows)`; the cell count is about `m / 16 + cols + rows`.
3. A cell holding more than `CROWDED = 64` nodes is a group of its own, at depth + 1. Gather its
   members with a counting sort, not one array per cell. Recurse on its own box. At `MAX_DEPTH = 4`
   a crowded cell keeps `side / sqrt(occ)`.
4. Result: the median of `s` over a stride sample of at most `SAMPLE = 4096` nodes
   (`i = floor(k * n / SAMPLE)`). Take a sorted copy of the sample. Return 0 when the median is 0
   or not finite.

`frame.ts` changes:
- `frameFrom` computes the factor by a new exported
  `readableFactor(x: Float32Array, y: Float32Array, bounds: Bounds | null): number`:
  - `floor = worldFactor(bounds, n)`, the old answer, kept and exported unchanged;
  - `spacing = typicalSpacing(x, y)`;
  - when `spacing > 0`, return `min(max(TARGET_SPACING / spacing, floor), floor * MAX_SPREAD)` with
    `MAX_SPREAD = 32`; otherwise return `floor`.
  - A drawing is never packed tighter than today. A pathological clump cannot push coordinates
    past f32 sense.
- `worldFactor` keeps its signature and its tests. Rewrite its `Ponytail:` to say it is now only the
  floor.
- `readableFactor` and `typicalSpacing` each carry a `Ponytail:` marker (the house marker, capital
  P) naming what they still get wrong:
  - the median serves the majority, so a minority clump under 50 % of nodes can still overlap;
  - x/y only, so a 3D drawing seen edge-on can overlap;
  - live force frames arrive in motor units and bypass the factor (`state/keepForces.ts:57-59`).

## Tests (node:test, `packages/graph-render/tests/spacing.test.ts`, plus `frame.test.ts` edits)

Each test is a fact with numbers; no timing assertions.
1. A 10 × 10 lattice at pitch 1: `typicalSpacing` is 1 (± 1e-6), and `readableFactor` equals
   `worldFactor` (== `TARGET_SPACING`).
2. The defect, as the failing case:
   - 900 nodes on a 30 × 30 lattice at pitch 0.01 near the origin, plus 100 nodes evenly on a circle
     of radius 1000.
   - Assert `worldFactor(bounds, 1000) * 0.01 < 2`: today the clump's pitch is under 2 world units.
   - Assert `readableFactor(...) * 0.01 >= TARGET_SPACING / 2`: the new pitch is at least 28.
   - Write this test FIRST and see it fail against a `readableFactor` that returns `worldFactor`.
3. All nodes on one point: `typicalSpacing` is 0, `readableFactor` returns the floor (1 for a zero box).
4. Collinear, 10 nodes at pitch 1 on a line: `readableFactor` equals `TARGET_SPACING`.
5. Determinism: two calls on the same columns return bit-identical numbers.
6. Bounded: 200 000 nodes, uniform pseudo-random from a fixed LCG (no `Math.random`):
   - `readableFactor / worldFactor` is within [1, 1.25]: a uniform spread barely moves;
   - the call returns.
7. Clamp: 999 nodes on one point plus 1 node at (1e6, 1e6): the result is at most
   `worldFactor * MAX_SPREAD`.
8. `frame.test.ts`:
   - Lines 99-109 and 112-126 assert `frame.factor === worldFactor(...)` for 2-node frames. With
     2 nodes the median rule may differ. Make them assert `frame.factor === readableFactor(...)` on
     the same columns, and keep their intent: the factor comes from the node hull and never from the
     edge points.
   - `packages/graph-studio/tests/parity-scene.test.ts:39` expects `factor 1` on a parity frame that
     never goes through `frameFrom`. Leave it alone; if it turns red, report it and stop.

## Do, in order
1. Read `frame.ts`, `camera.ts`, `scene.ts`, `tests/frame.test.ts`, `tests/support.ts`.
2. Write `tests/spacing.test.ts` test 2. Run `scripts/studio.sh test` and see it fail for the
   right reason (missing export, or the stub returning the floor).
3. Implement `spacing.ts`, then `readableFactor`, then the rest of the tests.
4. `scripts/studio.sh check` green: tsc, unit, render tests, eslint (`max-lines` 300,
   `max-lines-per-function` 40), vite build.
5. Write `docs/decisions/render-readable-spacing.md` (≤ 50 lines): the defect with test 2's
   numbers, the rule, the floor and the clamp, and "What it does not do" (the three Ponytail
   limits).
6. The rows gate (`job-check`) runs the browser rows. A red browser row is a stop: report its
   `target/studio-*/` report, do not tune thresholds.

## Never
- Never edit `crates/`, `src/`, `deploy/`, `server/`, `scripts/orch/`, or any file outside
  `packages/graph-render/{src,tests}/` and the one decision doc.
- Never change `TARGET_SPACING`, `MIN_SCREEN_RADIUS`, the camera, or any threshold in a browser gate.
- Never use `Math.random`, `Date`, or `performance.now` in `spacing.ts`.

## Return block
status (green / red / blocked) · gates (row → PASS/FAIL) · changed (files) · numbers (test 2 before
and after, test 6 ratio) · deviations · next.
