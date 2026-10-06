# Job render-sparse-cells (agent build: the spacing estimate is capped by the nearest neighbours)

Why: the studio draws a commit history under `layout.dag.lanes` with its nodes fused into a solid
bar. The orchestrator measured this with the pw MCP on develop f0049f8c, on git/git (85,928 nodes):
- motor pitch is 1 unit per row and 1 per lane, and one node sits on each row;
- the true nearest-neighbour distance has p10/p25/p50/p75/p90 = 1 / 1 / 1 / 1 / 3.61 motor units;
- `typicalSpacing` returns 8.44;
- the world factor is therefore 56 / 8.44 = 6.633;
- every node radius is 11.07 world units, so neighbours 6.6 apart overlap about 3.3×.

The cause is `packages/graph-render/src/spacing.ts`:
- `gridOf` (`:46-70`) grids the 196 × 85,927 box as a plane, with `side` = 56.
- A typical cell holds 44 nodes. That is under `CROWDED` = 64, so the cell is never re-gridded.
- `spaceGroup` (`:111-122`) writes `side / sqrt(occ)` = 8.44. That is the spacing of 44 nodes spread
  uniformly over the cell. These 44 sit on a few lines at pitch 1, scattered across the cell
  (the members' box is 41 of 56 wide at the median), so no box-based rule sees it. The orchestrator
  tried "read a sparse cell by its members' box", and it moved the median only from 8.44 to 8.17.

The failing input is any cell whose nodes are not uniform inside it: lanes, and Sugiyama layers.

Ownership: `packages/graph-render` belongs to the session graph-render-0e. On 2026-10-06 it handed this
one fix to the orchestrator ("it stays yours"). Touch only what this brief names.

Facts (develop, 2026-10-06; re-check each on your branch before editing, and stop if one no
longer holds):
- `spacing.ts` is 175 lines:
  - `PER_CELL = 16`, `CROWDED = 64`, `MAX_DEPTH = 4`, `SAMPLE = 4096` (`:8-15`);
  - `passOf` (`:79-90`) counts the nodes per cell;
  - `regroup` (`:93-108`) gathers each cell's members by a counting sort (`order`, `start`);
  - `spaceGroup` writes `columns.s[i]` per node;
  - `sampledMedian` (`:125-131`) and `typicalSpacing` (`:141-148`) return the sampled median.
- `packages/graph-render/tests/spacing.test.ts` has seven tests. **Every one keeps its
  assertions unchanged and stays green.** In particular:
  - "a uniform lattice keeps today's factor" pins 0.9 to 1e-6;
  - "a uniform random spread of 200 000 nodes barely moves" pins `readable / floor` in
    [1, 1.25].
- `frame.ts` (`readableFactor`, `worldFactor`) is not changed.

**The rule.**
1. Add a second per-node column, `columns.d`, a `Float64Array` filled with `Infinity`.
2. In every pass, for every cell with `2 ≤ occ ≤ CROWDED`, write each member's distance to its
   nearest other member **of the same cell**, ignoring distance 0 (coincident nodes). Do this by
   brute force over the cell's members.
   - Get the members from the same counting sort `regroup` uses. Extract that sort into one
     function called by both; do not write a second copy.
   - The cost is at most `CROWDED · n` distance evaluations, with no sort and no hashing.
   - Cells with `occ > CROWDED` are re-gridded (or, at `MAX_DEPTH`, keep `Infinity`).
3. `typicalSpacing` returns `min(sampledMedian(s), NEAREST_RATIO · sampledMedian(d))`, with
   `NEAREST_RATIO = 2.5`. Keep the existing "0 when it cannot be told" behaviour: a median of
   `s` that is 0 or not finite still returns 0.
   - Why 2.5: for a uniform random spread of density ρ, the median nearest distance is
     `sqrt(ln 2 / π) / sqrt(ρ)` = 0.47 / √ρ, and today's cell estimate reads about 0.97 / √ρ.
     2.5 × 0.47 = 1.17 stays above 0.97, so uniform spreads and lattices keep today's number
     exactly. A within-cell search can only read a nearest distance high (a neighbour across the
     border is missed), and high is the safe direction.
   - Put that derivation in the constant's doc comment.

Steps:
1. **RED.** Add two tests to `spacing.test.ts`:
   - `a lanes drawing is capped by the pitch of its lines`:
     - 85,928 nodes; node `i` at `y = i` and `x = lane(i)`;
     - `lane` draws from `mulberry32(7)` (add a 6-line local helper if the file has none): 0 with
       probability 0.8, else a uniform integer in 1..195;
     - assert `typicalSpacing` is in `[0.9, 2.5 + 1e-9]`;
     - assert `readableFactor` is at least `TARGET_SPACING / 2.5 - 1e-9`.
   - `a layered drawing is capped by the pitch inside its layers`: 40 layers 50 units apart, each
     holding 300 nodes at x = 0, 1, …, 299. Assert `typicalSpacing` is in `[0.9, 2.5 + 1e-9]`.

   Run `scripts/studio.sh test`. Both new tests must fail today: the lanes one reads about 8.
   Record both numbers it prints.
2. **The fix**, in `spacing.ts`.
   - Each function stays ≤ 40 lines, ≤ 4 parameters, nesting ≤ 3; the file stays ≤ 300 lines.
   - Rewrite the `Ponytail:` paragraph of the `typicalSpacing` doc for the new rule. Keep its
     existing failure inputs that still hold, and add these two:
     - a drawing made of far-apart pairs, each node with one close twin, reads 2.5 × the pair
       distance and spreads the drawing out;
     - the nearest search stays inside a cell, so it misses neighbours across a border.
   - Update the file header, which says "read from how crowded a uniform grid over them is".
3. **Green.**
   - Run `scripts/studio.sh test`: all tests pass, the seven old ones unchanged.
   - Run `scripts/studio.sh check`.
   - Run the browser gates below; each is in the rows file.

Rules (beyond `scripts/orch/common.md`):
- Node only through `scripts/studio.sh` / `scripts/orch/node-slim.sh`. The browser gates never
  take the host gate lock.
- Paths you may touch:
  - `packages/graph-render/src/spacing.ts`;
  - `packages/graph-render/tests/spacing.test.ts`.
- Nothing else changes.
- If a browser gate goes red because a drawing's scale moved, stop and report the gate, the row
  and both numbers. Do not re-baseline anything.
- No type assertions (`as X`), no `any`, no `eslint-disable`.

Done when `scripts/orch/gate.sh target/rows-render-sparse-cells
scripts/orch/rows/render-sparse-cells.rows` writes a `summary.txt` with every row PASS.

Return:
- the branch tip;
- the files changed, with line counts;
- the two RED numbers from step 1;
- the new `typicalSpacing` on both new tests;
- each row's result;
- every deviation.
