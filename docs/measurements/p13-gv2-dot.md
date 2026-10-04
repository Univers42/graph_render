# `layout.graphviz.dot` — Graphviz's `dot`: the measurements, the blockers, and the draft of what is left

**Status: not implemented. The `GRAPHVIZ_DOT` row stays as it was.** What this file
records is the measurement the job asked for first, the two named blockers, and the
draft of what is left, so the next attempt starts from facts rather than from a
re-derivation of them.

# What was measured

## The rank pass: how much of it agrees with the oracle

The rank pass — `class1`, `acyclic`, one network simplex per component, `TB_balance` — is
ported, and its output is compared with the oracle over the full 1000-seed fixture set.

**The rank is derived from the printed y, not assumed.** `set_ycoords` (`position.c:773-786`)
puts `GD_maxrank` at the *bottom* and stacks every rank below it higher up, so **rank 0 is the
top row** and `y = y_of_rank_0 - rank * (height + ranksep)`. That direction is the whole
measurement: reading y the other way round gives every one of the six closed cases upside
down, which is what the mirror test in `dot/rank_tests.rs` is there to catch.
`harness/oracle-dot-probe.py` reports the largest distance any printed y sits from that
grid, over every node of every seed, and it is **0.0000 of a step** — so every node of every
fixture lands exactly on a rank, and the table is the oracle's own layering rather than a
rounded guess at it.

| measurement | over 1000 seeds |
|---|---|
| every node on the same rank as the oracle | **692** |
| identical total weighted edge length | **993** |
| a strictly greater total weighted edge length | **6** |
| a strictly smaller one | **0** |

```sh
scripts/orch/gr cargo run -q -p graph-cli --release -- \
    emit-graphviz-fixtures --engine twopi --seeds 1000 --out target/dot-probe1000
cp target/dot-probe1000/twopi.jsonl target/dotfix/dot.jsonl
cp target/dot-probe1000/twopi-manifest.json target/dotfix/dot-manifest.json
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/dotfix dot target/gv-dot-det-a --fixtures=dot.jsonl
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl --digest=target/probe/dot1000.txt
scripts/orch/gr cargo test -p graph-core --lib -- --ignored \
    rank_agreement_over_1000_seeds --nocapture
# 692 of 1000 seeds agree node for node; 993 have equal cost; 6 are worse
```

### What the 308 node-for-node disagreements are

**The 302 that have the same cost are ties.** Blocker 2 below already says it: the simplex's
optimum is a *face* of the polytope, not a point, so two correct implementations reach
different optimal rankings, and `TB_balance` then spreads nodes across equally good ranks in
an order that depends on the visit order. On seed 138 one node of 140 differs; on seed 143
two of 145; the number of ranks is the same and the cost is the same. That is the face, not a
defect.

**The 6 that cost more are a different DAG.** Seeds 330, 351, 469, 497, 930 and 951. On
seed 330 the port's ranking is feasible, its spanning tree is tight, and every cut value
agrees with the value recomputed from scratch — checked after *every* pivot by
`simplex::checks::check`, which is `cfg(test)` and costs nothing in the library.
Against that same edge list the **oracle's** ranking has slack **−4**: it is not feasible for
the graph the port ranked. So the two ranked different DAGs, and the difference is in
`acyclic`'s choice of back edge on a graph with more than one cycle. `acyclic` is a faithful
port of `acyclic.c:33-69`, and the port's choice is a function of the dense node index and
`decompose`'s pop order — which is what the reference's is a function of too — so what is
left is a walk-order detail in a pass whose input is a stack pop order, not an algorithmic
difference. It is recorded rather than closed: 6 of 1000 seeds, bounded, and measured.

### The closed cases and the twenty fixture seeds

Both are pinned node for node in `dot/rank_tests.rs`: the six closed cases from the table
below, and seeds 0 to 19 (`n` = 2 to 21) from the oracle. Those 26 cases are **inside** the
692 — the first disagreement in the whole set is seed 138.

## The oracle is deterministic over the full 1000 seeds

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine twopi \
--seeds 1000 --out target/dot-probe1000

# the fixture file is the engine's own key; rename it so the harness reads it as dot's
cp target/dot-probe1000/twopi.jsonl        target/dotfix/dot.jsonl
cp target/dot-probe1000/twopi-manifest.json target/dotfix/dot-manifest.json
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
python3 harness/oracle-graphviz.py target/dotfix dot target/gv-dot-det-a --fixtures=dot.jsonl

# …and again into target/gv-dot-det-b, then
cmp target/gv-dot-det-a/graphviz-dot.jsonl target/gv-dot-det-b/graphviz-dot.jsonl
```

`cmp` is silent. Measured: **byte-identical, 1000 seeds, 65 s per run**.

## `-Gstart` is inert for this engine, measured not assumed
The same 1000 fixtures at `-Gstart` 1, 7 and 99, all through the plain positional
call so all three write the same record shape:
```sh
for s in 1 7 99; do
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
python3 harness/oracle-graphviz.py target/dotfix dot "target/gv-dot-s$s" \
--fixtures=dot.jsonl --start=$s
done
cmp target/gv-dot-s1/graphviz-dot.jsonl target/gv-dot-s7/graphviz-dot.jsonl
cmp target/gv-dot-s1/graphviz-dot.jsonl target/gv-dot-s99/graphviz-dot.jsonl
```

Both `cmp`s are silent, and all three files hash to
`46c6a56480b1dae022205db343b863e17384a78a35a41afeb91b291e2ba60073`. So the answer to
the job's question — *if the start seed changes the output, reproduce Graphviz's
seeded initial placement* — is **it does not**. `dot` draws no random numbers on this
path: there is no `-Gstart` sensitivity to match, no initial-position dependence, and
no chaos to blame for a gap. Any difference from Graphviz is an algorithmic difference
or nothing. So there is nothing for a `Ponytail` marker to say about the seed.

# Mincross — the order within a rank, measured over the same 1000 seeds

## The probe is committed, so the numbers below are reproducible

`target/probe/rank_oracle.py` was never committed, so the rank numbers above could not be
re-derived from a clean checkout. **`harness/oracle-dot-probe.py` replaces it** and is in the
repository, and it writes **both** answers per seed in one `-Tplain` run: the rank of every
node, derived from its printed y, and **the order of the nodes inside each rank**, derived
from their printed x. One row per seed:

```
seed n  t,h t,h ...  <n ranks>  <n order>
```

where `order` is the per-rank left-to-right node lists concatenated, rank 0 first. The
digest is `target/probe/dot1000.txt`, read by `dot/oracle_probe.rs`, which holds both
`#[ignore]`d sweeps and the shared scaffolding.

```sh
scripts/orch/gr cargo run -q -p graph-cli --release -- \
    emit-graphviz-fixtures --engine twopi --seeds 1000 --out target/dot-probe1000
cp target/dot-probe1000/twopi.jsonl target/dotfix/dot.jsonl
cp target/dot-probe1000/twopi-manifest.json target/dotfix/dot-manifest.json
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl \
    --digest=target/probe/dot1000.txt
# 1000 seeds -> target/probe/dot1000.txt
# largest distance from the rank grid: 0.0000 of a step
```

The derivation is measured, not assumed: the probe reports the largest distance any printed
y sits from the 72-point rank grid, and it is **0.0000 of a step** over every node of every
seed — the same number the rank section records, read by the same run now. One `dot` per
seed, 1000 seeds in **67 s**.

**Measured over the probe's own rank column: no fixture seed of the 1000 has an edge whose
two ends land on the same rank** (0 of 1000, 0 of 400 280 edges). That is the escape hatch
for the one part of the reference's pass this port does not have — the same-rank edge
precedence matrix — and it is measured rather than assumed, below.

## What the port has, and how far it agrees

`dot/mincross.rs` and `dot/mincross/{ranks,build,median,transpose,crossings,driver}.rs`,
each step in its own words in the doc comment above it. `allocate_ranks` (one row per rank,
one slot per node and per edge that spans it, one spare), `build_ranks` in both directions
(the BFS from the in-sources, then from the out-sinks), `medians` with `MC_SCALE = 256` and
its weighted-median branch, `reorder`, `transpose`, `rcross`/`ncross` with the per-band
cache, `save_best`/`restore_best`, and the three passes: 0 and 1 with at most 4 sweeps, 2
with `MaxIter = 24`, `MinQuit = 8`, `Convergence = 0.995`, keeping the best order seen.
Integer arithmetic wherever the reference has it; the one `double` is the weighted median's
ratio, as the reference's is.

Over the seeds whose **ranks** already agree node for node — an order is only comparable
when the rows it orders are the same rows:

| measurement | of the 692 rank-agreeing seeds |
|---|---|
| the same order in **every** rank | **408** |
| the same number of edge crossings | **446** |
| a disagreement that keeps the crossing count | **38** |
| a disagreement where the port draws **fewer** crossings | **119** |
| a disagreement where the port draws **more** crossings | **127** |

```sh
scripts/orch/gr cargo test -p graph-core --lib -- --ignored \
    order_agreement_over_1000_seeds --nocapture
# 692 of 1000 seeds agree on the ranks; of those 408 on every rank's order, 446 on the
# crossing count. Of the 284 disagreements: 38 keep the crossing count, 119 draw fewer
# crossings and 127 draw more
```

**What the two numbers say.** The order is not reproduced on 284 of the 692, and the
crossing count is a coin flip across them: 119 draw fewer crossings than the oracle and 127
draw more. So the disagreements are **not** the port drawing something worse — it is the
pass reaching a *different local minimum of a heuristic*, which is what a 24-sweep
best-of-what-it-found pass does, and the pass's own answer is deterministic
(`order_tests::two_runs_order_identically` is the check). **Nineteen of the twenty fixture
seeds** (`n` = 2 to 21) reproduce exactly, and all six closed cases' orders are pinned.

### What the crossing count is, and is not

`-Tplain` prints no crossing count and prints no chain dummy, so **the pass's own count has
no counterpart on the oracle's side**: it counts over the chains, weighting each link by its
penalty, and the dummies it places are internal state the plain format never shows. The
comparison above is therefore over a count that reads nothing but a rank and a row — two
edges cross when they span the same pair of *adjacent* ranks with their ends in opposite
orders — which one implementation computes on both sides, so the two can only differ
because the orders do. Its two stated limits are pinned as tests in `oracle_crossings.rs`:
an edge that jumps a rank is not counted (72.8% of the fixture edges span exactly one rank),
and neither is a pair that shares an endpoint. Over the 1000 seeds, 400 280 edges, so the
count is most of a drawing's crossings but not all of them, and the gap is named rather than
approximated.

### Seed 4, the smallest disagreement, worked out by hand

Seed 4 is `n1 -- n0` twice, `n2 -- n1`, `n2 -- n0`, `n3 -- n0`, `n4 -- n0` twice and
`n5 -- n1` twice: six nodes, ranks `[2, 1, 0, 1, 1, 0]`, one chain dummy. The top rank
holds `n2` and `n5`, and **both orders of that pair draw with no crossing at all** — `n2`
reaches `n1` and reaches `n0` through the dummy, `n5` reaches only `n1`, and whichever of
the two sits left, the dummy and `n1` end up on the same side of it. The oracle's rows are
`[n2, n5] / [n1, n3, n4] / [n0]`; the port's are `[n5, n2] / [n1, n3, n4] / [n0]`. Both are
pinned, in `order_tests.rs`, together with the check that the crossing count is the same
either way round — so the seed is a recorded finding and not a hole in the table.

**What is not traced.** Which of the two the pass reaches is decided by the initial walk plus
the transverse pass's first round, and the port's answer is the one a faithful reading of
`build_ranks` and `transpose_step` produces: on these rows the top rank's adjacent pair
crosses twice as `n2`-left and not at all as `n5`-left, so the transverse pass swaps it.
Turning the transverse pass's rank loop round instead makes seed 4 agree and **seed 8
disagree**, at the same rate — so the direction of that loop is not what the 284 come from,
and the cause is not isolated. It is a finding with its seeds, not a reason to stop: the
pass is bit-deterministic, it is not systematically worse, and the next thing to try is the
initial walk's node order, which the rank pass's agreement does not pin (the simplex is far
less sensitive to it than a BFS is).

## The same-rank edge precedence, and what it would cost

The reference's transverse pass refuses to swap two nodes joined by an edge whose ends share
a rank, through a per-rank adjacency matrix built by a depth-first search over those edges.
This port has no such matrix and answers "swap allowed" for every pair, with a `Ponytail`
line on `transpose::order_is_pinned` naming the direction of the omission. The escape hatch
is measured: **0 of the 1000 fixture seeds has a same-rank edge**, so nothing measured here
can see it, and a graph that has one is the failing input.

Two smaller omissions, both consequences of the same absence of ports and clusters, are
named in `dot/mincross.rs`: the port-local half of the crossing count (the reference counts
inversions between two edges *out of the same node* against each other's port positions, and
this port has no ports), and the edge `ordering` attribute's virtual edges (a no-op when the
attribute is absent, which it always is here).

# What is ported
`crates/graph-core/src/layout/graphviz/dot.rs` and its children, with their own tests:
- `fast.rs`, `fast/edge.rs`, `fast/node.rs` — the fast graph: `node_t`/`edge_t` as dense
  indices, the three per-node edge lists, `orig_out`, `zapinlist`, `reverse_edge`,
  `merge_oneway`/`basic_merge`, `find_fast_edge`, `virtual_node`.
- `class1.rs` — **the pass that puts edges into the fast graph**: one constraint per input
  edge, parallel pairs folded together. Not a no-op, whatever the pass list suggests —
  without it the simplex has an empty graph to rank.
- `decomp.rs` — `decompose`, the components and the order each is walked in.
- `acyclic.rs` — `acyclic`, cycle breaking by edge reversal.
- `simplex.rs` and `simplex/{subtree,tree,cutval,pivot,balance}.rs` — `rank2`:
  `init_graph`, `init_rank`, `feasible_tree` with its tight-subtree heap and minimum-slack
  inter-tree walk, `init_cutvalues`, the `leave_edge`/`enter_edge`/`update` pivot loop with
  its `Search_size = 30` cut-off and rotating index, and the three balance passes.
- `rank.rs` — `dot1_rank`: `class1`, `decompose`, `acyclic`, one simplex per component, the
  four no-op stages named as no-ops, `cleanup1`.
- `class2.rs` — chains for edges spanning more than one rank, merged parallel edges,
  `virtual_weight`, and the flat and other lists.
- `mincross.rs` and `mincross/{ranks,build,median,transpose,crossings,driver}.rs` — the
  order pass: the per-rank rows, the two initial walks, `medians`/`reorder`, `transpose`,
  `rcross`/`ncross` and the three passes with `save_best`/`restore_best`.
- `rank_tests.rs` (the six closed cases and twenty fixture seeds), `order_tests.rs` (the
  same twenty, ordered), `mincross_tests.rs` (one closed case per step),
  `class2_tests.rs` (each of `class2`'s three outcomes), `oracle_crossings.rs` (the one
  crossing count both sides of an oracle comparison can be computed with) and
  `oracle_probe.rs` (both 1000-seed sweeps, `#[ignore]`d, which also holds the shared
  scaffolding). `simplex/checks.rs` re-derives the pass's invariants from scratch under
  `cfg(test)` after **every** pivot.

Twenty-three tests in `dot/`, all passing, and one earns its place twice over: **a two-node
cycle collapses to a single edge, not two.** `reverse_edge` (`acyclic.c:22-33`) unhooks the
edge and then *merges* it into the edge already running the other way, so the survivor
carries both weights. A port that swapped the endpoints would keep two edges and draw a
different graph.

# Blocker 1 — the node box is a font metric, and x is where it lands
Graphviz sizes a node from its **rendered label**, not from `width`/`height` alone.
Measured on the oracle image, `dot -Tplain` over one-node graphs with `width=0
margin=0`, so the printed width is the text width alone, then the node's own box with
the default `margin` of `0.11, 0.055` inch:
| label | text width (points) | node width (points) |
|---|---|---|
| `n0` … `n9` (2 characters) | 19.2211 | 54.0000 — the `0.75` inch minimum |
| `n10` … `n99` (3) | 29.3976 | 57.9420 |
| `n100` … (4) | 39.5734 | 70.3577 |
| `n1000` (5) | 49.7491 | 82.7712 |
Two things follow, and the second is the blocker.
**The text width is additive per character but the advance is not a round number.** For
`n` repeated `k` times the width is `9.04536 + (k-1) × 10.1758` points — the first
character is *narrower* than the rest, and no per-character table reproduces that. The
image has `libfreetype` and `libgd` and **no fontconfig fonts registered** (`fc-list`
is absent, `/usr/share/fonts` holds only DejaVu), so `gvtextlayout` either resolves
through a library whose metrics graph-core has no access to, or falls back to
`estimate_text_width_1pt` (`textspan_lut.c:833`) — whose `Times` table gives `'n' =
1024/2048 = 0.5` em, i.e. 7 points at the default 14-point size, against the measured
9.045. **Neither source is reproducible from this repository**, and one of the two is
which one is not knowable from outside the image.
**The width is not cosmetic here; it is an input to the x-coordinate network simplex.**
`make_LR_constraints` (`position.c:262-266`) builds the auxiliary edge between two
nodes on a rank with `width = ND_rw(u) + ND_lw(v) + nodesep`, so every node's box width
is a *constraint length* in the pass that decides x. A wrong width is a different
drawing, not a shifted one: past ten nodes the fixture ids stop fitting the `0.75`
inch minimum and the two arms constrain each node to a different length. The y
coordinates are unaffected — they are `rank × (0.5 + 0.5) inch` and every node has the
same height — so the disagreement is confined to x, and it is not a constant offset the
bounding-box rescale can absorb.
This is the same class of cause `layout.packing.osage` records, and worse: osage's
disagreement is a cell width in a uniform grid, this one is a per-node constraint length
in a simplex.
Escape hatch: none inside the motor. `graph-contract`'s node record carries a `label`
string, so the *characters* are available; what is missing is any font metric to turn
them into a width, and graph-core publishes no font engine and no attribute channel.
Closing this means a text-measurement table taken from the oracle image and pinned
here, which is a decision for the ADR (`docs/decisions/graphviz-oracle.md`), not for
this job.

# Blocker 2 — the port does not fit one job
`dot` is four passes, and three of them are large:
| pass | reference | size | ported |
|---|---|---|---|
| 1 rank | `acyclic.c` 70, `decomp.c` 117, `ns.c` 1414, `rank.c` 1113, `class2.c` 294 | ~3000 lines | **yes** — see the rank section above |
| 2 mincross | `mincross.c` 1794 | ~1800 | **yes** — see the mincross section |
| 3 position | `position.c` 1133, plus a second `ns.c` run | ~1100 | no |
| 4 splines | `dotsplines.c` 2316 | ~2300 | not needed (polylines) |
Pass 1 alone is three times the size of `layout.packing.osage` and most of
`layout.treemap.patchwork`. Note the *second* `ns.c` run: `dot_position` re-runs the
whole simplex over the auxiliary graph, so the engine is two simplex implementations
deep — which is why `simplex::Params` carries the balance pass as a parameter rather than
hard-coding `TB_balance`, and why the second run is a call and not a copy. And the
tight-tree basis decides which optimum is reached (the optimum is a face of the
polytope, not a point) — so a simplification there is a different drawing, not a faster
one. **Measured, now:** the face is reachable two ways and both are correct, and
692 of 1000 seeds land on the same face as the oracle.

# The remaining passes, as a draft
1. **`simplex.rs` — `rank2`. Done** — `init_graph` / `init_rank` / `feasible_tree` (maximal
tight subtrees by DFS from each unvisited node of `nlist`, merged smallest-first
through the minimum-slack inter-tree edge) / `init_cutvalues` (`dfs_range_init` then
`dfs_cutval`) / the `leave_edge`–`enter_edge`–`update` pivot loop with its
`Search_size = 30` cut-off and its rotating `S_i` / then `TB_balance` (rank) or
`LR_balance` (x). Integer throughout, so the pivots are exact.
2. **`rank.rs` — `dot1_rank`. Done** — `edgelabel_ranks` (a no-op: no edge labels),
`collapse_sets` and `minmax_edges` (no-ops: no clusters, no min/max/same sets),
`class1` (**not** a no-op — it builds the fast graph), `decompose` ✓, `acyclic` ✓,
`rank1` per component, `expand_ranksets` (a no-op), `cleanup1`.
3. **`class2.rs` — chains and multi-edges. Done** — `make_chain` for every edge spanning
more than one rank, `merge_chain` for parallel edges, `virtual_weight`'s
`table[endpoint_class][endpoint_class]`, and the backward-edge shadowing. The
fixtures have no self-loops (`synthetic_edges` skips `a == b`) and no clusters, so
`interclrep` and `realFillRanks` drop out.
4. **`mincross.rs`. Done** — `allocate_ranks`, `build_ranks` in both passes (the BFS from
in-sources then out-sinks), then `mincross(g, 0)`: passes 0 and 1 with
`maxthispass = min(4, MaxIter)` and pass 2 with `MaxIter = 24`, `MinQuit = 8`,
`Convergence = 0.995`, `medians` (with `MC_SCALE = 256` and the weighted-median
branch), `reorder`, `transpose`, `rcross`/`ncross`, `save_best`/`restore_best`.
**Not** ported, with the measured reason in the mincross section: the same-rank edge
precedence matrix (no fixture seed of the 1000 has a same-rank edge), the port-local half
of the crossing count (no ports) and the cluster path (no clusters).
! 5. **`position.rs`.** `set_ycoords` (rank heights, `pht1`/`pht2`, `ranksep`), then
!    `create_aux_edges` = `make_LR_constraints` + `make_edge_pairs`, `rank(g, 2, …)`,
!    `set_xcoords`, `set_aspect` (a no-op at the default ratio), `remove_aux_edges`.
! 6. **The frame.** `dotneato_postprocess` translates so the drawing's lower-left *node
box* corner is the origin, which for the six closed cases is the offsets in the
table below. This is part of the answer, not presentation: the closed cases are
compared byte for byte against `-Tplain`'s printed text, so the translation has to
be applied on our side too or nothing can match.
! 7. **The differential.** `crates/graph-cli/src/oracle_python/dot.rs` as a `Differential`
(shape: `oracle_python/osage.rs`), one `by_engine` arm and one `ENGINES` entry, and
a `DOT_CLOSED` table in `harness/oracle-graphviz.py:144` for the six closed cases.
The metric, the rescale and `CLOSED_CASES` are imported from `harness/oracle-twopi.py`
and must not be copied.
!

# The six closed cases, as the oracle prints them
Points, in the frame `-Tplain` prints — node boxes at the origin, half a node outside
the node-centre box. These are the byte-exact targets, and the constants they imply are
all confirmed: node `0.75 × 0.5` inch, `nodesep` `0.25` inch, `ranksep` `0.5` inch, so
ranks are 72 points apart and same-rank neighbours 72 points apart.
| case | nodes | oracle's centres (points) |
|---|---|---|
| one node | 1 | `n0` (27, 18) |
| two nodes, `n0--n1` | 2 | `n0` (27, 90), `n1` (27, 18) |
| 3-path | 3 | `n0` (27, 162), `n1` (27, 90), `n2` (27, 18) |
| 4-cycle | 4 | `n0` (54, 234), `n1` (27, 162), `n2` (27, 90), `n3` (54, 18) |
| 5-star | 5 | `n0` (135, 90), `n1` (27, 18), `n2` (99, 18), `n3` (171, 18), `n4` (243, 18) |
| 6-branch | 6 | `n0` (99, 234), `n1` (27, 162), `n2` (99, 162), `n3` (171, 162), `n4` (99, 90), `n5` (99, 18) |
The 4-cycle is the case that discriminates: `acyclic` must reverse `n3 -> n0`, which
puts `n3` on rank 3 and `n0` on rank 0, and the x-coordinates are off-centre by 27
points on the top and bottom ranks — a port that got the cycle-breaking direction wrong, or
that placed the ranks symmetrically, fails it.
**Read the ranks top-down: the largest y is rank 0.** The port's own ranks for the six
cases are `0`, `0,1`, `0,1,2`, `0,1,2,3`, `0,1,1,1,1` and `0,1,1,1,2,3`, pinned in
`dot/rank_tests.rs`.
Reproduce the table:
```sh
printf 'graph g {\n  n0; n1; n2; n3;\n  n0 -- n1;\n  n1 -- n2;\n  n2 -- n3;\n  n3 -- n0;\n}\n' > /tmp/cyc4.dot
scripts/orch/drun --rm --pull never --user 0:0 -v /tmp:/w -w /w ge-graphviz-oracle \
dot -Tplain -Gstart=1 /tmp/cyc4.dot
```

# Ceiling
**Not set.** The job's rule is that the ceiling is the next power of ten above the
worst gap measured over 1000 seeds, and there is no measured gap because there is no
`dot` arm to measure against — so writing a number here would be inventing it. When
the differential lands, the first thing to record is the gap on the `n <= 10` seeds,
where every label is two characters and every box is the `0.75` inch minimum, because
**that is the set on which byte-exactness is reachable at all** given blocker 1. The
row's status must be `Status::Implemented`, never `gated`: a layout that agrees only
where every node box is the default is not a gated layout, and `layout.packing.osage`
sets the precedent for saying so in `docs/measurements/scigraphs-coverage.md` rather
than in a ceiling.
