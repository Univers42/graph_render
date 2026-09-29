# Phase 5 — crossing-count differential against dagre-d3-es

This margin is written down **before** any number below it was measured. It is not
adjusted after the fact. If the run below crosses it, the phase reports `status: blocked`
and the constants stay as designed — this file records the numbers, it does not license
tuning them into passing.

## The margin (frozen)

Oracle: `dagre-d3-es@7.0.14`, installed by `npm install -D -E dagre-d3-es@7.0.14` (pinned in `package.json`/`package-lock.json`), reference copy at
`/goinfre/dlesieur/refs/npm/dagre-d3-es-7.0.14`.

**"Materially worse"** means either of:

- on any of the 6 named fixtures (`fixtures/dag/*.json`), our crossing count is greater
  than dagre's own count on the same graph **plus `max(2, ceil(10% of dagre's count))`**;
- or, summed over the fixture set **plus** a ≥200-seed synthetic DAG sweep, our total
  crossing count is greater than **1.10 × dagre's total**.

If either holds: do not tune `ordering.rs`'s constants (iteration/transpose throttle,
median tie-breaks) to chase the number down. Report the numbers as measured, mark the
phase `status: blocked`, and leave the decision to raise the margin or accept the
regression to whoever owns that call.

## Method

Our own number, for a graph `g`, is `layout::sugiyama::crossings_for(g)` — the crossing
count the ordering stage's median+transpose sweep actually settled on, read directly
(not re-derived from geometry).

dagre-d3-es has no public "crossing count" API: it only returns final node/edge
positions. **A first attempt** reconstructed a count from that geometry — bucket every
node center and edge bend point into a layer by exact `y`, sort each layer by `x`, count
interleaved segments per adjacent rank pair. That attempt was wrong and was caught before
being trusted: `edge.points` are spline control points from dagre's curve-fitting step,
not points that sit on a rank's `y` at all (a diagnostic dump on the K4,4 `wide-layer`
fixture showed a first bend point at `y: 6.25` against real rank `y`s of `5` and `65`).
Bucketing by exact `y` therefore split real segments across spurious one-point buckets and
silently dropped almost all of them from the count — it reported `wide-layer: 4`, where a
direct hand count shows any two-layer straight-line drawing of the complete bipartite
K4,4 has **exactly** `C(4,2) * C(4,2) = 36` crossings, unavoidably, regardless of ordering
(every one of the `C(4,2)` top-pairs crosses every one of the `C(4,2)` bottom-pairs
exactly once, since all `4*4` edges exist). That 36 is what our own layout produces on
that fixture, so the geometry reconstruction, not our layout, was the thing wrong by 9x.

The corrected method asks dagre for its own crossing count directly, instead of
re-deriving one from geometry it was never meant to expose that way. It re-runs the same
internal pipeline stages `layout()` itself runs, stopping at its own `order` stage, then
calls its own `crossCount` — the exact function dagre's ordering heuristic minimizes
internally (Barth, Jünger & Mutzel's bilayer counting, the same algorithm family as our
own `ordering::bilayer_crossings`), read directly rather than reconstructed:

```
acyclic.run -> [attach every node to a scratch root, matching what dagre's own
nesting-graph does for a flat graph, so network-simplex ranking never sees a
disconnected input] -> rank -> [drop the scratch root] -> normalizeRanks ->
normalize.run (splits long edges into per-rank dummy chains) -> order -> crossCount
```

The root-attachment step reproduces `nesting-graph.js`'s `run` for the flat (no
subgraphs) case exactly: with no compound children, its `treeDepths` gives every node
depth 1, so `height = max(depths) - 1 = 0` and `nodeSep = 2*height + 1 = 1` always,
meaning its own edges would be `{weight: 0, minlen: 1}` from a virtual root to every
node — precisely what `harness/oracle-layouts.mjs` adds by hand. Re-running this on
`wide-layer` gives dagre `36`, matching the hand count and our own layout exactly.

Both sides run on **the same graph** (same node ids, same edge list, same direction) —
the JSON fixtures for the 6 named graphs, and Rust-generated synthetic graphs for the
sweep. This correction is itself a deviation (a broken measurement technique replaced
before its output was trusted, not an algorithm retuned to pass); see
`docs/decisions/sugiyama-heuristics.md`.

### Reproduction

```sh
# 1. Rust side: our own crossing count, for the 6 fixtures + the synthetic sweep,
#    dumped to target/dag-crossings.json (graph + our_crossings per entry).
gr cargo test -p graph-core dump_crossing_measurements -- --ignored

# 2. JS side: dagre-d3-es's own order/crossCount on the same graphs, compared against
#    ours under the margin above. Exit 0 within the margin, 1 materially worse, 2 could
#    not run.
docker run --rm -v "$PWD:/w" -w /w node:22-slim sh -c 'npm ci --ignore-scripts && node harness/oracle-layouts.mjs --dag'
```

`package-lock.json` is kept because the arm imports `dagre-d3-es`: `npm ci` needs the
lock to install the pinned 7.0.14 and its dependencies. The arm refuses to report unless
dagre counts K4,4 at its hand-counted 36 crossings.

The synthetic sweep (`layout::sugiyama::measurement::synthetic_dag`) draws, per seed
`0..230`, a node count in `6..26` and includes each `i < j` pair as an edge independently
at density 0.2 — acyclic by construction (no back edge is possible) and free of parallel
edges (each unordered pair is considered once), using
`crate::synthetic::Mulberry32` seeded by the loop index. A seed that draws zero edges is
skipped, so the sweep still clears 200 non-trivial graphs.

## Results

Measured 2026-09-28 by running the two reproduction commands above verbatim.

### The 6 named fixtures

| fixture      | ours | dagre | margin `max(2,⌈10%⌉)` | verdict |
|--------------|-----:|------:|-----------------------:|---------|
| chain        |    0 |     0 |                       2 | ok |
| diamond      |    0 |     0 |                       2 | ok |
| cyclic       |    0 |     0 |                       2 | ok |
| multi-span   |    0 |     0 |                       2 | ok |
| wide-layer   |   36 |    36 |                       4 | ok |
| disconnected |    0 |     0 |                       2 | ok |

`wide-layer` is K4,4: any two-layer straight-line drawing of it has exactly
`C(4,2)*C(4,2) = 36` crossings regardless of ordering (see Method), so 36 on both sides is
the forced, optimal value, not a coincidence.

### Synthetic sweep

230 seeds (`0..230`), each drawing `6..26` nodes with `i<j` edges at density 0.2 (acyclic,
parallel-edge-free by construction); combined with the 6 fixtures above for the sum
clause, per the frozen margin text.

- graphs = 236 (6 fixtures + 230 synthetic)
- sum(ours) = 5242
- sum(dagre) = 7657
- 1.10 × sum(dagre) = 8422.7
- sum verdict: **ok** (ours is below dagre's own total, not just under the 10% margin)

Per-graph breakdown across all 236: ours strictly better than dagre on 133, equal on 89,
strictly worse on 14 (max per-graph gap where worse: 13 crossings) — no fixture crossed
its individual margin, and the summed total does not either.

### Verdict

**status: pass.** Neither margin clause is crossed. No tuning of `ordering.rs`'s
constants was needed or attempted.

### Raw evidence

The former `scratch/` working files, condensed. Every number regenerates from the two
commands above. The verdict record of the 2026-09-28 run:

```json
{
  "fixtures": "the 6 rows of the fixture table above",
  "sweep": { "seeds": 230, "sumOurs": 5242, "sumDagre": 7657, "sweepLimit": 8422.7 },
  "anyFixtureWorse": false,
  "sweepWorse": false,
  "materiallyWorse": false
}
```

The 14 graphs of the 236 on which ours is strictly worse than dagre:

| graph | ours | dagre | gap |
|-------|-----:|------:|----:|
| synthetic-227 | 38 | 25 | +13 |
| synthetic-16 | 25 | 16 | +9 |
| synthetic-22 | 75 | 70 | +5 |
| synthetic-37 | 11 | 7 | +4 |
| synthetic-70 | 57 | 53 | +4 |
| synthetic-26 | 3 | 1 | +2 |
| synthetic-212 | 13 | 11 | +2 |
| synthetic-8 | 1 | 0 | +1 |
| synthetic-49 | 1 | 0 | +1 |
| synthetic-50 | 9 | 8 | +1 |
| synthetic-77 | 2 | 1 | +1 |
| synthetic-147 | 1 | 0 | +1 |
| synthetic-154 | 1 | 0 | +1 |
| synthetic-169 | 2 | 1 | +1 |
