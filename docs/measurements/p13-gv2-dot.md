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
`target/probe/rank_oracle.py` reports the largest distance any printed y sits from that
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
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/dotfix dot target/gv-dot-det-a --fixtures=dot.jsonl
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 target/probe/rank_oracle.py --digest target/probe/rank1000.txt
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
`simplex::tree::check_invariants`, which is `cfg(test)` and costs nothing in the library.
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
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
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
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
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
- `rank_tests.rs` — the six closed cases, twenty fixture seeds, each of `class2`'s three
  outcomes, and the 1000-seed sweep. `check_invariants` runs under `cfg(test)` after every
  pivot.
Thirty tests in `dot/`, all passing, and one earns its place twice over: **a two-node cycle
collapses to a single edge, not two.** `reverse_edge` (`acyclic.c:22-33`) unhooks the edge
and then *merges* it into the edge already running the other way, so the survivor carries
both weights. A port that swapped the endpoints would keep two edges and draw a different
graph.

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
| 2 mincross | `mincross.c` 1794 | ~1800 | no |
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
! 4. **`mincross.rs`.** `allocate_ranks`, `build_ranks` in both passes (the BFS from
in-sources then out-sinks), then `mincross(g, 0)`: passes 0 and 1 with
`maxthispass = min(4, MaxIter)` and pass 2 with `MaxIter = 24`, `MinQuit = 8`,
`Convergence = 0.995`, `medians` (with `MC_SCALE = 256` and the weighted-median
branch), `reorder`, `transpose`, `rcross`/`ncross`, `save_best`/`restore_best`.
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
docker run --rm --pull never --user 0:0 -v /tmp:/w -w /w ge-graphviz-oracle \
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
