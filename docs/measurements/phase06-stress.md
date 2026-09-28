# Phase 6 — force-layout stress metric (branch p6f)

Ponytail: `P56_SPEC.md` decision 6 says this file is written *before* measuring. It
was not: the measurement infrastructure (the `force_dump` example, `scratch/stress.mjs`,
the synthetic-graph generator) had to be built and debugged first, and one bug in that
infrastructure (below) was only found by looking at a first, wrong set of numbers. The
metric and margin below were fixed **before** the numbers in this file's tables were
taken — those tables are the corrected, final run — but the file itself was written
after, not before, in violation of the letter of decision 6. Reported here rather than
silently reordered to look compliant.

## The metric (spec decision 6)

Primary: the Pearson correlation between graph (BFS hop) distance and Euclidean
(layout) distance, over the pairs `(pivot, v)` for `v` any other node reachable from
that pivot, across a fixed pivot set.

**Pivot set**: max-min (farthest-point) selection, starting at dense index 0. Pivot 0
is node 0. Each subsequent pivot is the not-yet-chosen node whose distance to its
*nearest* already-chosen pivot is largest; ties keep the lowest index. Selection stops
at 32 pivots or when every node has been chosen, whichever comes first — so a fixture
with fewer than 32 nodes uses all of them. A node unreachable from a pivot (a different
connected component) counts as infinitely far for the purpose of picking the *next*
pivot (so a disconnected fixture gets a pivot in every component before refining within
one), and its `(pivot, v)` pair is skipped entirely when accumulating the correlation
(hop distance is undefined, not merely large). Implemented in `scratch/stress.mjs`'s
`selectPivots`.

Secondary (reported, not gated): edge-length coefficient of variation — **not
computed for this phase**; no gate depends on it, and it is left for the integration
step per `P56_SPEC.md`'s own scoping of what that step does. Its absence is a
deviation, recorded here rather than fabricated.

**Margin** (spec decision 6): `ours (Barnes-Hut) >= d3's - 0.05`, checked on every
fixture and on the median of a 50-seed synthetic sweep. `d3` is a real
`d3-force@3.0.0` simulation (not a stand-in), run with the identical topology, the
identical golden-spiral seed positions, and the frozen force set below, ticked the same
`TICKS = 112` times.

## The frozen force set (spec decision 7)

`link` + `manyBody` (theta 0.9, `distanceMax` 520) + `center` + `collide` (r = 16).
Cluster forces are excluded — no node groups exist before Phase 10 — a deviation
recorded by the spec itself, not one this branch introduced.

d3 side (`scratch/stress.mjs::runD3`), matching `crates/graph-core/src/layout/force/
params.rs`'s cited engine constants exactly:

```js
forceLink(links).id(d => d.index)
  .distance(l => 60 / Math.max(0.4, l.strength))
  .strength(l => Math.min(0.7, 0.15 * l.strength))
forceManyBody().strength(-90).distanceMax(520)
forceCenter(0, 0)
forceCollide().radius(16).iterations(1)
// .alpha(1).alphaDecay(0.06).velocityDecay(0.42), then 112 ticks
```

## Deviations from d3-force@3.0.0

Every place our port's *behaviour*, not just its file layout, differs from
`/home/user/refs/npm/d3-force-3.0.0`:

1. **Link and collide are Jacobi gathers, not Gauss-Seidel scatters** (devil C7). d3
   mutates both endpoints of a link, or both members of a collision, as it visits them
   in whatever order the quadtree/edge list gives — order-dependent, and unsafe to
   parallelise later (Phase 11). Ours zeroes a double-buffer, has every node/edge read
   only the previous tick's frozen state, and merges once, after every reader has read.
2. **Counter-based jiggle, not a shared sequential LCG** (devil C8). d3's `jiggle()`
   draws from one `lcg()` stream consumed in visit order, so its result depends on
   visit order and (later) thread scheduling. `rng::jiggle(seed, tick, pass, (i, j))`
   is a pure, order-independent function of the pair — `jiggle(seed, tick, pass, (i,
   j)) == jiggle(seed, tick, pass, (j, i))` exactly, verified by test.
3. **Link's bias substitutes lo/hi simple-graph degree for d3's source/target role.**
   d3's `bias = count[source] / (count[source] + count[target])`, where "source" and
   "target" are whichever edge endpoint that role happened to be assigned on input.
   `SimpleGraph` (`layout/force/mod.rs`) has already collapsed direction (devil C6:
   undirected, parallel edges collapsed, self-loops dropped), so there is no surviving
   source/target role to read; ours uses `degree(lo) / (degree(lo) + degree(hi))` by
   ascending node index instead, a stated, arbitrary-but-fixed substitute.
4. **Collide is a full-tree-per-node symmetric gather, not a single shared
   computation with an `index > index` filter.** The frozen params give every node the
   same `collideRadius`, so d3's own per-pair split `rj² / (ri² + rj²)` is exactly
   one half either way; each node independently computing its own half, from a
   subtraction its partner would compute as the exact IEEE754 negation, realises devil
   C7's "canonical orientation" without needing d3's literal single-computation-shared-
   by-both-endpoints structure.
5. **No cluster force** (spec decision 7, the spec's own deviation, not this branch's).
6. **Origin-centered seed positions, not viewport-centered.** The osionos engine's
   `seedPositions` centers the golden spiral on the viewport
   (`forceLayout.ts`); graph-core has no viewport concept, so ours centers on the
   origin instead. `scratch/stress.mjs::goldenSpiral` mirrors this for the d3 side of
   the comparison, so both sides start from the same, origin-centered spiral — the
   deviation is from the *engine*, not between the two sides of this measurement.

## A discovery, not a defect: `seeded_model`'s `seed` does not vary topology

The first attempt at the 50-seed sweep used graph-core's own
`seeded_model(seed, count, reference_degree)`, assuming a different `seed` would give a
different graph. It does not, for a fixed `count`: every one of the 50 dumps came out
byte-identical (`diff` on the dump files showed no difference), and so did every
aggregate statistic. Tracing it down: `seeded_model`'s `seed` only drives `remix()`,
which reshuffles `NodeRecord.source` strings and recomputes `weight` — a rendering
field, not structure — while the actual edges come from `synthetic_records(count)`,
which uses its own fixed internal constant, unrelated to the outer `seed`. This is not
a bug in graph-core; that function exists for the cross-target hash gate's
metadata-perturbation testing, not for generating varied topologies. But it meant the
first sweep measured the same graph fifty times under a different label. Fixed by
writing a small Barabási–Albert-style generator outside graph-core
(`/tmp/gen_synthetic.py`, not committed — its *output*, `scratch/synthetic/seed-{0..
49}.json`, is), and a `force_dump ... path <file> <layout>` mode to load it. Verified
this time that `seed-0.json` and `seed-1.json` actually differ.

## Measured results

Reproduce (from the worktree root):

```sh
/home/user/gr cargo run --release --example force_dump -- fixture grid barnes_hut
/home/user/gr cargo run --release --example force_dump -- fixture grid fa2
# ... one pair of dumps per fixture, then:
docker run --rm --network host -v "$PWD:/w" -v "/home/user/graph_render/node_modules:/w/node_modules:ro" \
  -v /root/.ccr/ca-bundle.crt:/etc/ssl/extra-ca.crt:ro -e NODE_EXTRA_CA_CERTS=/etc/ssl/extra-ca.crt \
  -e HTTPS_PROXY -e https_proxy -w /w node:22-slim \
  node scratch/stress.mjs scratch/dumps fixture-grid fixture-tree fixture-clustered fixture-disconnected fixture-single-node
```

(The extra `node_modules` mount is this branch's own deviation from
`/home/user/node-slim.sh` verbatim: the git worktree at `/home/user/wt-p6f` has no
`node_modules` of its own — only the original `/home/user/graph_render` checkout does
— so the wrapper script as written cannot see `d3-force`/`d3-quadtree` from inside the
worktree. The extra mount is read-only and adds nothing but visibility into an
already-installed, already-committed dependency tree.)

### Fixtures (`fixtures/force/*.json`)

| fixture | n | m | r(d3) | r(ours, Barnes-Hut) | r(ours, FA2) | margin (BH − d3) |
|---|---:|---:|---:|---:|---:|---:|
| grid | 64 | 112 | 0.17131 | 0.28701 | 0.95799 | **+0.11570** |
| tree | 121 | 120 | 0.20431 | 0.21493 | 0.56836 | **+0.01063** |
| clustered | 60 | 138 | 0.04460 | 0.15718 | 0.77259 | **+0.11258** |
| disconnected | 13 | 11 | 0.09866 | 0.13034 | 0.84308 | **+0.03168** |
| single-node | 1 | 0 | n/a | n/a | n/a | n/a (no pair exists; handled as `null`, not a fabricated 0 or 1) |

Every non-degenerate fixture's margin is positive, comfortably clear of the −0.05
floor.

### 50-seed synthetic sweep (`scratch/synthetic/seed-{0..49}.json`)

Barabási–Albert-style random graphs, `n` from 60 to 195, attachment count `m` from 2
to 4, `random.Random(seed)` for reproducibility (generator not committed; its output
is). All 50 margins:

| statistic | r(d3) | r(ours, Barnes-Hut) | r(ours, FA2) |
|---|---:|---:|---:|
| mean | 0.2313 | 0.2931 | 0.4302 |
| min | 0.1620 | 0.2115 | 0.2600 |
| max | 0.3234 | 0.4039 | 0.6440 |
| stdev | 0.0392 | 0.0451 | 0.0881 |

Margin (Barnes-Hut − d3): min 0.00825, max 0.12178, mean 0.06182. **The spec's own gate
is the median, which is 0.06009 — clear of −0.05. 0 of 50 cases fall below the −0.05
threshold** (the strictly stronger per-case check, reported above the gate itself).

- Worst margin: `synth-11` (n=75, m=290) — r(d3)=0.27142, r(ours)=0.27968,
  margin=+0.00825.
- Best margin: `synth-42` (n=90, m=177) — r(d3)=0.20634, r(ours)=0.32813,
  margin=+0.12178.

## Reading the numbers

Across all 54 non-degenerate cases measured (4 fixtures + 50 synthetic seeds; the
single-node fixture has no node pair to correlate), our Barnes-Hut port's stress
correlation meets or exceeds real `d3-force@3.0.0`'s own, on the same topology, same
seed positions, same frozen force set, same tick count. The margin criterion (spec
decision 6) is satisfied on every case, not just the aggregate.

The correlations themselves are modest in absolute terms (0.04–0.32 for d3, 0.13–0.40
for ours) at exactly `TICKS = 112`. That reflects the *frozen* engine parameters
(tuned in osionos for its own interactive use, not for this metric) applied to graphs
of these particular sizes and shapes, not a defect specific to the port — our numbers
consistently track d3's own closely (e.g. `tree`: 0.20431 vs 0.21493), which is what
"ported faithfully" should look like. FA2 correlates markedly higher throughout
(0.26–0.96): it runs to convergence or `max_iter = 100`, not a fixed 112-tick budget,
and has no collision/center pull competing with its pure attraction+repulsion+gravity,
so it settles into a more legible, if less "physically damped," layout — reported, not
gated (the margin criterion is Barnes-Hut vs d3 only, per spec decision 6).
