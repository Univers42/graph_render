# sg-sugiyama — the `SUGIYAMA` conformance row, stage by stage

Row: **`SUGIYAMA`** (`conformance/rows.rs`), reference `apply_graph_layout`, motor
`layout.dag.sugiyama`. Gaps it started with: `G_NO_ITERATIONS`, `G_SCALE_FIXED_LAYER`
(`conformance/gaps.rs`).

**Result: `shape`/`algorithm` 348/1020 f64, 349/1020 f32 → `tolerance`/`arithmetic` 597/1020
f64, 1020/1020 f32.** Every coordinate of all 23 measured fixtures is now bit-identical as
`f32`; the 423 remaining `f64` differences are the narrowing itself, one `f32` ULP
(max gap 1.91e-07), and the row's Procrustes median fell from 0.384 to 1.26e-16.

## Baseline, on the untouched tree

```
scripts/orch/gr cargo build --release -p graph-cli        -> 0
scripts/scigraphs-conformance.sh                          -> 0   (PASS)
  SUGIYAMA: ok — 348 f64, 349 f32 of 1020 coordinates, median 3.838e-1 <= 1.000e0
```

| fixture | f64 | f32 | max gap | procrustes |
|---|--:|--:|--:|--:|
| lesmis | 77/231 | 77/231 | 4.94e+01 | 3.61e-01 |
| tree-balanced | 16/45 | 16/45 | 7.00e+00 | 6.38e-01 |
| dag-diamond | 4/12 | 4/12 | 5.00e+00 | 1.09e-01 |
| bipartite | 15/42 | 15/42 | 5.00e+00 | 3.42e-01 |

The `f64` and `f32` counts are near-identical, which says the disagreement is **units and
ordering**, not last-digit arithmetic: the motor drew the right graph in the wrong frame.

## Step 1 — the unit gap, closed first

The motor drew X in the priority method's own units (`coords.rs`'s `GAP = 1.0`, uncentred)
and Y as `layer * LAYER_SPACING`. The reference maps **each axis on its own**
(`hierarchical.py:679-685`): X by `((x - lo) / width * 2 - 1) * scale` over the drawing's own
extent, Y by `((layer / max_layer) * 2 - 1) * scale`, `z = 0`.

**`lo`/`hi` are over every vertex of the ordering graph, dummies included** — `_assign_x`
returns one X per vertex, dummy or not — and `Geometry` carries no dummy coordinates. So the
normalisation cannot be a post pass in the conformance arm. It became a second entry point,
`sugiyama::run_scaled(topology, scale)` (`layout/sugiyama/scaled.rs`), over the same six
stages; `conformance/motor.rs` calls it for this row, the way it already calls
`sfdp::run_seeded`. `layout.dag.sugiyama` and `SugiyamaParams` are untouched.

Tests (`scaled/tests.rs`), all against the formula read off `hierarchical.py:679-685`:
the 3-node chain (`[-5,-5] [-5,0] [-5,5]`, the `(hi - lo) or 1.0` branch), a fan-out (both
axes at the ends of the range), a drawing with no layers (`max_layer == 0` → every Y `0.0`),
and the refused scales.

Re-measured: **348 → 621 f64, 349 → 923 f32**, median 3.838e-1 → 2.99e-1. `lesmis` went to
**231/231 f32** and `dag-diamond` to **12/12 f64 and f32**, which isolated the rest as a
shape problem on the gate models alone.

## Step 2 — the per-stage diff

Two halves over the same graphs by the same node-order rule, neither copying the other:

- **reference**: the script in the appendix below, run in `ge-python-oracle` with the
  `SciGraphs/` submodule on the path. It imports
  `scigraphs_core.mesh.layouts.hierarchical` and calls `_acyclic_arcs`,
  `_longest_path_layers`, `_reduce_slack`, `_build_ordering_graph`, `_init_order`,
  `_order_layers`, `_assign_x` and the normalisation itself, dumping `arcs`, `layer_of`,
  `num_dummies`, per-layer `order`, `crossings` and `x` per fixture. **It imports the
  reference; it does not restate it.**
- **motor**: the ignored test
  `layout::sugiyama::stages::dump::dump_stage_measurements` writes the same fields for the
  diamond, `lesmis`, `tree-balanced` and gate seeds 0..19 to
  `target/sugiyama-stages.json`. The gate models come from `graph_core::seeded_model`, the
  other three from `include_str!` on the repo fixtures.

```
scripts/orch/gr cargo test -p graph-core --lib sugiyama::stages -- --ignored     -> 0
docker run --rm -v "$PWD:/w" -w /w ge-python-oracle \
    python3 target/sugiyama-stages-py.py target/scigraphs-conformance > py.json   -> 0
python3 diff.py py.json target/sugiyama-stages.json
```

`target/sugiyama-stages-py.py` is a throwaway under `target/` (git-ignored), so it is
reproduced here rather than committed; `diff.py` is six lines of `json.load` and a
`!=` per stage.

### First divergent stage, before the fix

| fixture | first divergent stage | first divergent value |
|---|---|---|
| dag-diamond | — | all seven stages identical |
| lesmis | — | all seven stages identical |
| gate-00 (2 nodes) | `arcs` | index 0: ref `[0,1]`, ours `[1,0]` |
| gate-01 (3) | `arcs` | index 0: ref `[0,1]`, ours `[1,0]` |
| gate-02 (4) | `arcs` | index 0: ref `[0,1]`, ours `[1,0]` |
| tree-balanced (15) | `arcs` | index 0: ref `[0,1]`, ours `[1,0]` |

**Two causes, both in stage one**, and the second only became visible once the first was
fixed:

1. **`Acyclic::of` used the wrong vertex order.** `_acyclic_arcs` reads
   `_greedy_fas_order(G) if G.is_directed() else list(G.nodes())` (`hierarchical.py:304`),
   and `scigraphs_core/mesh/layouts/common.py:238` builds `nx.Graph()` — **undirected** — for
   every layout. The graph `apply_graph_layout` hands the pipeline therefore never reaches
   the greedy branch: the order is `list(G.nodes())`, the node listing, which under the
   fixture contract is ascending dense index. The port ran the greedy FAS unconditionally,
   and on any graph whose edges are not already spelled forward the two orders disagree.
   `ArcOrder` (`acyclic.rs`) now names both and `Acyclic::of` takes `NodeIndex`.
2. **Parallel arcs were not deduped.** The reference's `arcs` is a `set` of `(u, v)` pairs
   (`hierarchical.py:306`), so `k` parallel edges are one arc: one neighbour on each side of
   the layering, one dummy chain, one entry in `up`/`down`. The port kept one arc per edge,
   so a parallel edge doubled a neighbour's degree, shifted the median `_reduce_slack` slides
   toward, and built its own chain. `Arcs::distinct()` is the reference's own list, and
   `assign_layers`, `budget_plan` and `materialize` are built over it; `Route` stays
   per-edge, so every parallel edge is still drawn, through the one chain they now share.

This closes the "Parallel edges are not deduped" deviation in
`docs/decisions/sugiyama-heuristics.md` and replaces it with its replacement.

### RED / GREEN, per stage

RED (fails on the tree as it stood):

```
scaled::tests::an_edge_written_backwards_is_oriented_by_node_order_as_the_reference_orients_it
  left:  [[-5.0, 5.0], [-5.0, -5.0]]
  right: [[-5.0, -5.0], [-5.0, 5.0]]
```

`single-backwards` — two nodes, one edge `1 -> 0` — is the smallest graph where the two arc
orders disagree, and the expected value is the reference's own, dumped from
`hierarchical.py` in `ge-python-oracle`: arcs `[(0,1)]`, layers `[0,1]`, order `[[0],[1]]`,
positions `[-5,-5]` and `[-5,5]`.

GREEN: same run, `ok`.

### ### After both fixes — all 23 fixtures, every stage

`python3 diff.py py-stages.json target/sugiyama-stages.json` over dag-diamond, lesmis,
tree-balanced and gate seeds 0..19 reports **no difference in `arcs`, `layer_of`,
`num_dummies`, `order`, `crossings` or `x`** on any of the 23 — the full pipeline agrees with
the reference on every fixture, including `lesmis` at 254 arcs and 763 dummy vertices.

| stage | dag-diamond | lesmis | tree-balanced | gate-00 … gate-19 |
|---|---|---|---|---|
| `arcs` | same | same | same | same |
| `layer_of` | same | same | same | same |
| `num_dummies` | same (0) | same (763) | same (1) | same |
| `order` | same | same | same | same |
| `crossings` | same (0) | same (894) | same (0) | same |
| `x` | same | same | same | same |

One field still differs and is not a divergence: `reversed`, the count of **edges** the cycle
breaker flipped against their own spelling. The reference reports no such number — it draws
no edges and has no `Route` — so there is nothing on the other side to agree with. It is in
the dump because a non-zero count is what tells a reader where to look first.

**No new registered layout was needed.** The SciGraphs order did not conflict with the dagre
differential, so `layout.dag.sugiyama` is repaired in place and no
`layout.dag.sugiyama_scigraphs` variant exists.

### One false divergence the diff itself produced, and how it was caught

The first `tree-balanced` comparison reported `arcs` index 1 as ref `[0,4]` against ours
`[0,2]` — and `(0,2)` is not an edge of the tree at all. The motor half's fixture loader was
wrong, not the pipeline: it took dense index from the file's **list** order, while the
conformance arm's rule is byte order (`conformance/fixtures.rs`), and
`fixtures/hierarchy/tree-balanced.json` lists `r` first for exactly that reason. The stage
dump is the test that found it, which is the argument for keeping the dump rather than
comparing inside graph-core.

## The dagre differential, before and after

`harness/oracle-layouts.mjs --dag`, over `target/dag-crossings.json` from the ignored
`dump_crossing_measurements`:

```
                        before                after
chain                     0   0   2   ok       0   0   2   ok
diamond                   0   0   2   ok       0   0   2   ok
cyclic                    0   0   2   ok       0   0   2   ok
multi-span                0   0   2   ok       0   0   2   ok
wide-layer               36  36   4   ok      36  36   4   ok
disconnected              0   0   2   ok       0   0   2   ok
graphs=236 sum(ours)=5242 sum(dagre)=7657 1.10x=8422.7 verdict=ok   (identical)
status: pass
```

`sum(ours) = 5242` before and after: **the crossing counts are byte-identical**, because the
gate's synthetic DAGs are spelled with every edge `i < j` and the six fixtures hold no
parallel edges, so node-index orientation and the greedy FAS order agree there and the dedup
removes nothing. What the dagre gate does *not* cover — a graph with backwards edges or
parallel arcs — is exactly what changed.

## Re-pinned row

```
scripts/scigraphs-conformance.sh                -> 1   (SUGIYAMA: FAIL, only that row)
    row("SUGIYAMA", "fb980d82…240cd", "308abc27…56b1d", "", 1e-15, "tolerance", "arithmetic")
```

`baseline/table/structured.rs` updated with that exact block, nothing retyped, no other row
touched.

| fixture | f64 before | f32 before | f64 after | f32 after |
|---|--:|--:|--:|--:|
| lesmis | 77/231 | 77/231 | 88/231 | **231/231** |
| tree-balanced | 16/45 | 16/45 | 21/45 | **45/45** |
| dag-diamond | 4/12 | 4/12 | 12/12 | **12/12** |
| bipartite | 15/42 | 15/42 | 30/42 | **42/42** |
| gate-00 … gate-19 | 2-24 / N | 2-24 / N | 13-41 / N | **N/N** |

Row totals: f64 348 → **597**, f32 349 → **1020**, max gap 49.4 → **1.91e-07**, max ULP
9.24e+18 → **2.57e+08**, Procrustes median 0.384 → **1.26e-16**, tier `shape` → **`tolerance`**,
cause `algorithm` → **`arithmetic`**.

**Why `tolerance` and not `bitwise`:** the `f64` file is an `f32` widened
(`graph-contract`'s node columns are `Vec<f32>`), so `f64` agreement is unreachable except
where the reference's own value is `f32`-representable. `f32 1020/1020` is the strongest
statement this tree can make, and the row now makes it. The remaining gap is the narrowing:
one `f32` ULP at that magnitude.

## What is left, stated rather than fudged

- **`G_NO_ITERATIONS` was a false record and is gone.** `_sugiyama_layout(G, scale)`
  (`hierarchical.py:638`) takes no count, and `dispatcher.py:142-143` passes it `scale` and
  nothing else, so there was no budget the reference ran with for this layout to have failed
  to bound.
- **`G_SCALE_FIXED_LAYER` is closed and its const deleted** (`scale` is now a parameter of
  `run_scaled`).
- **`ArcOrder::Feedback` is now unreachable from the reference** and from `Acyclic::of`; it
  stays because it is the port of `_greedy_fas_order` and its tie-break is what D4 requires,
  and it is pinned by `the_greedy_feedback_order_is_the_other_branch_and_reverses_more`
  rather than left as an unused function.
- **The graph-core `Topology` has per-edge `directed` and no whole-graph flag**, so
  `ArcOrder` is a choice the pipeline makes rather than one it reads off the input. If the
  pipeline is ever fed a genuine `nx.DiGraph` order, `ArcOrder::Feedback` is there and
  `Acyclic::oriented` is the seam. Same shape of gap as `registry/three_d.rs:218`.
- **`crates/graph-core/src/registry/grid.rs`'s `SUGIYAMA` metadata now overstates the
  cycle breaker.** Its `ponytail` field still reads "Ponytail (FAS): greedy, not minimum;
  extra reversed edges (note 5) are cosmetic" and its `oracle` field says "acyclic after
  FAS". The pipeline no longer runs the FAS; it orients by node index and still emits note 5
  for each backwards edge. That file is outside this job's envelope (`layout/sugiyama/**`
  and the conformance arm only), so it is reported here rather than edited. The two fields
  that would change are the `ponytail` sentence quoted above and the word "FAS" in
  `oracle`; nothing else in the entry is affected, and `capabilities --check` /
  `codegen --check` are unaffected because no `Metadata` field's value moved.

## Commands, with their real exit codes

```
scripts/orch/gr cargo build --release -p graph-cli                              -> 0
scripts/scigraphs-conformance.sh                       (untouched tree)          -> 0
scripts/orch/gr cargo test -p graph-core --lib sugiyama                          -> 0
scripts/orch/gr cargo test -p graph-core --lib sugiyama::measurement -- --ignored -> 0
scripts/orch/node-slim.sh node harness/oracle-layouts.mjs --dag                  -> 0  (status: pass)
scripts/orch/gr cargo fmt --all -- --check                                        -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings            -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                             -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8                   -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 ... hashgate --seeds 8            -> 1  (negctl)
scripts/orch/gr cargo run -q -p graph-cli -- codegen --check                       -> 0
scripts/scigraphs-conformance.sh                       (before the re-pin)       -> 1  (SUGIYAMA only)
scripts/scigraphs-conformance.sh                       (after the re-pin)        -> 0
scripts/scigraphs-conformance.sh --break                                          -> 1  (SPRING_3D)
```

`capabilities --check` exits 1 on this tree for a reason that predates the change and is not
about it: it wants `hashgate --seeds 1000` and the full oracle-diff records, which is the
once-on-develop gate `CLAUDE.md:65` describes. `codegen --check`, the other half, is 0.

## Appendix — the reference-side stage dump

```python
"""SciGraphs' sugiyama pipeline, stage by stage, by calling its own private functions."""
import json, os, sys
sys.path.insert(0, os.path.join("SciGraphs", "core"))
import networkx as nx
from scigraphs_core.mesh.layouts import hierarchical as H

out = {}
for line in open(os.path.join(sys.argv[1], "conformance.jsonl")):
    line = line.strip()
    if not line:
        continue
    fx = json.loads(line)
    if not (fx["name"].endswith(("dag-diamond", "lesmis", "tree-balanced"))
            or fx["name"].startswith("gate-")):
        continue
    n = fx["n"]
    G = nx.Graph()                      # what common.py:238 builds: undirected
    G.add_nodes_from(range(n))
    G.add_edges_from(list(zip(fx["source"], fx["target"])))
    nodes = list(G.nodes())
    arcs = H._acyclic_arcs(G)
    layer = H._reduce_slack(H._longest_path_layers(nodes, arcs), nodes, arcs)
    layer_of, up, down, num_dummies, straight = H._build_ordering_graph(nodes, arcs, layer)
    max_layer = max(layer_of) if layer_of else 0
    order = H._init_order(max_layer + 1, layer_of, up, down)
    position = {}
    for row in order:
        for i, v in enumerate(row):
            position[v] = i
    total = len(layer_of)
    crossings = H._order_layers(order, up, down, position,
                                8 if total <= 50000 else 4,
                                4 if total <= 5000 else (2 if total <= 30000 else
                                                        (1 if total <= 150000 else 0)))
    x = H._assign_x(order, up, down, n, 4 if total <= H._PRIORITY_NODE_BUDGET else 0)
    out[fx["name"]] = {"arcs": [list(a) for a in arcs], "layer_of": layer_of,
                       "num_dummies": num_dummies, "order": order,
                       "crossings": crossings, "x": [x[i] for i in range(len(x))]}
print(json.dumps(out))
```