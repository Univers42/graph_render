# `layout.dag.dot` — Graphviz's `dot`: the measurements, the blockers, and the three ported passes

**Status: three of the four passes ported and registered as `layout.dag.dot`.** The
rank pass, the mincross pass and the position pass are ported; the fourth, splines, is not
needed because the motor emits polylines through the chain dummies. The two blockers below
were measured before the port and one of them — the node width — is now *settled as a
finding* rather than closed: the width table the ADR names does not reproduce the oracle's
node width, which the "Position" section below measures and names. The differential and the
hash gate knob are the next job.

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
digest is `target/probe/dot1000.txt`, read by `dot/oracle_digest.rs`; the two `#[ignore]`d
sweeps that compare against it are in `dot/oracle_probe.rs`.

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

# Position

`position.rs` and its five children, ported from `position.c:127-153` and the frame. The pass is
five steps in the reference's order: `set_ycoords`, `create_aux_edges` (the rank constraints and
the edge pairs), **a second run of the same network simplex with `LR_balance`**, `set_xcoords` with
`remove_aux_edges`, and the frame. The second simplex is a call through `simplex::Params`, not a
copy, which is what `Params` was split into `top_bottom()` and `left_right()` for.

## What the pass costs, and the 1757 s that was not the pass

**The reviewer's hypothesis was wrong and the measurement says so: `fdeb` is not the cost.**
`fdeb::run` over `layout.dag.dot`'s geometry on `fixtures/post/hairball.json` (42 nodes, 178
edges) takes **3.1 ms** in debug and survives 158 pairs. The layout alone takes 2.81 s in debug
after the fix below and 1758 s before it.

The cost was the pass's own **x-coordinate simplex**, and specifically the `#[cfg(test)]`
invariant re-derivation that `rank2` runs after **every pivot**
(`dot/simplex/checks.rs`). The rank pass checks a 42-node graph; the position pass checks the
*auxiliary* graph, and the auxiliary graph carries one chain dummy per rank an input edge spans,
so the hairball arrives as **3860 nodes and 8170 edges** (1864 after `class2`, 1996 slack nodes
from `make_edge_pairs`, 1824 rank constraints and 3992 pair edges). `check_cut_values` was
`O(n * (n + m))` per call — one depth-first walk of the tree and one sweep of every edge *per
tree edge* — so it cost about **1.2 s per pivot** over ~1550 pivots.

Measured per step, debug, after the fix:

| step | before | after |
|---|---|---|
| `rank` | 1.14 ms | 0.33 ms |
| `mincross` | 318.8 ms | 237.9 ms |
| `Rows::of` | — | 0.29 ms |
| `ycoords::run` | — | 0.06 ms |
| `aux::build` | — | 1.27 ms |
| **`simplex::rank2` (the x pass)** | **1757.76 s** | **2.55 s** |
| `xcoords::run` | — | 0.02 ms |
| `frame::run` | — | 0.02 ms |
| **position total** | **1757.76 s** | **2.57 s** |
| `dot` total | 1758.08 s | 2.81 s |

`graph-cli snapshot --seed 0 --nodes 42 --layout layout.dag.dot`, release, three runs: **0.02 s
of user CPU** each (0.27–0.30 s wall, which is the cargo wrapper). So the whole 1757 s was the
test-only check and the shipped library was never slow — which is also why the reviewer saw it
in a test binary and not in a gate row.

**The fix, and why it is not a weakened assertion.** `check_cut_values` now derives every tree
edge's cut value from **one** pre-order walk of the tree and one reverse pass, instead of one
per edge: the cut value is the outgoing weight of the subtree at the edge's *deeper* endpoint
less the incoming weight there, negated when that endpoint is the edge's head. That negation is
`x_val`'s `dir` and it is the one thing that makes the identity exact — dropping it is what made
the first attempt of this fix disagree on half the test suite. The number is the same one the
per-edge loop summed (an interior edge adds its weight to both totals and cancels, a leaving
edge adds, an entering edge subtracts, and an interior tree edge's own cut value telescopes into
the same sum), and it is still a **from-scratch** recomputation: it reads only the tree's shape
and the edge weights, never the incremental cut values the pass maintains. Every assertion stands,
the cost is `O(n + m)` per check, and all 59 `dot::` tests pass unchanged — including the 1000-seed
rank and order sweeps, which are the two things that would catch a wrong cut value.

The graph size is *not* the bug and was not touched: Graphviz builds the same dummies, and the
reference has no per-pivot check to slow down.

### The two timings the reviewer asked for

| measurement | before | after |
|---|---|---|
| `cargo test -p graph-core --lib fdeb_surviving_pairs` | **1335 s** (reviewer's measurement, /proc utime, one test thread) | **5.32 s** wall / 0.02 s user (`test result: ok. 1 passed`) |
| `cargo test -p graph-core --test geometry_invariants` | develop: 697.76 s, 53 tests | **291.99 s** wall / **291.67 s** test time, **56 tests** |

`geometry_invariants` benefits from the same fix, because it runs every registered layout and
`layout.dag.dot` is one of them.

## The line the reference hides in a macro

`allocate_aux_edges` (`position.c:201-217`) looks like a capacity hint and is not one.
`alloc_elist` (`lib/common/types.h:267-270`) sets a list's **size to zero** and hands it a fresh
array, so the graph the x-coordinate simplex walks holds **only** the constraints the position
pass is about to make — the graph's own edges are not in it, and `remove_aux_edges` puts them
back afterwards. That is what leaves two nodes joined by one edge directly above each other with
the *same* x: the edge pair says their centres are not the same point, and one point of
separation satisfies it at zero cost. Keep the real edges and every node is pushed one point
right of its in-neighbour, which is what the two-node closed case measures. Measured both ways
on that case: 27 and 27 points with the macro's meaning, 27 and 28 with the other reading.

## The six closed cases: exact

Every row of the table under "The six closed cases" below reproduces **byte for byte** in the
plain format's own frame, at the printed precision (five significant digits, in inches), and in
points. Pinned twice in `dot/position_tests.rs`: once as the oracle's inch strings and once as
the measurement table's points, plus one test per step in `dot/position_steps.rs`.

**One correction to that table, and it is a table error rather than a port error.** The 6-branch
row there is the **seven-edge** graph (`n0 -- n1, n0 -- n2, n0 -- n3, n1 -- n4, n2 -- n4,
n3 -- n4, n4 -- n5`); `dot/order_tests.rs` calls a five-edge graph by the same name, without the
two `n4` edges. Both rank to `0, 1, 1, 1, 2, 3` and both order to `[0] [1 2 3] [4] [5]`, which is
why no rank or order test could tell them apart — but only the seven-edge graph puts `n4` and
`n5` under `n0` and `n2`, which is what the table records and what the oracle prints
(checked: `dot -Tplain` on the five-edge graph prints `n4` at `0.375`, on the seven-edge graph at
`1.375`). The seven-edge graph is what `position_tests.rs` uses.

## The twenty fixture seeds: 5 of 20, and both causes are named

| seed set | labels | exact |
|---|---|---|
| 0 to 8 | two characters | 5 of 9 |
| 9 to 19 | three characters | 0 of 11 |

`dot/position_fixture_points.rs` holds all twenty of the oracle's printed rows and
`dot/position_tests.rs` asserts the measured count, naming every disagreeing seed. The two causes are separated by that split, and each is
pinned as its own test:

**Cause 1, seeds 9 to 19, all eleven: the width table is the default box below four
characters.** `node_width(text_width(id))` is the formula of record,
`max(0.75 in, text + 2 * 0.11 in)`, and for `n0`…`n99` it returns exactly 54 points — the default
box. The oracle prints 0.80475 in = **57.942 pt** for `n10` and 0.97719 in = **70.358 pt** for
`n100` (measured, `dot -Tplain` on one-node graphs). So every seed whose ids reach three
characters is drawn with boxes three to sixteen points too narrow, and the x simplex is asked to
satisfy constraints that much too short. The relation that *does* reproduce the oracle's four
node-width rows is the one `text_width.rs`'s own module doc names,
`node = 1.37952 * label_box + 0.30669` inches, checked against all three rows to within 2e-4 inch
in `the_width_table_is_the_default_box_below_four_characters_and_the_oracle_is_wider`. **It is
not used**: this job's contract names `node_width(text_width(id))` as the only source of node
widths, and `text_width.rs` is outside this change. Settling it is one constant pair in that
module. This is Blocker 1 discharged as a measurement rather than closed as a defect.

**Cause 2, seeds 2, 4, 5 and 7, the four two-character seeds that still disagree: a chain
dummy's slot inside its rank.** The rank and order sweeps both call these seeds agreements
because they compare the *real* nodes of a rank, and a chain dummy is not one — so an order can
agree while the row the x constraints read does not. Seed 2 is the smallest witness: the
two-rank edge `n3 -- n0` puts a dummy on the middle rank beside `n1` and `n2`; the oracle draws
them **110** points apart and this port **72**. 110 is exactly the two constraint lengths of a row
ordered `n1, dummy, n2` (`27 + 10 + 18` and `10 + 27 + 18`, the dummy being a one-point box
widened by `nodesep / 2` on each side) and 72 is the single constraint of `n1, n2, …`. Pinned in
`the_disagreements_are_chain_dummy_slots_inside_a_rank`.

## The 1000-seed sweep

Population: the seeds whose **order** already agrees in every rank, because a coordinate is only
comparable when the row it sits on is the same row. Comparison: every node's centre against the
oracle's own printed strings, byte for byte, at five significant digits in inches.

| measurement | count |
|---|---|
| seeds whose every rank's order agrees (the population) | **408** |
| of those, every node's centre printed exactly as the oracle prints it | **10** |
| the exact seeds | **0, 1, 3, 6, 8, 600, 601, 603, 606, 608** |

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
scripts/orch/gr cargo test -p graph-core --lib --release -- --ignored \
    position_agreement_over_1000_seeds --nocapture
# 408 of 1000 seeds agree on every rank's order; of those 10 print every node centre
# exactly as the oracle prints it
# exact seeds: [0, 1, 3, 6, 8, 600, 601, 603, 606, 608]
# 1743 s in release, one pass per seed
```

**The probe's digest now carries the printed coordinates**, additively: `seed n t,h ... <ranks>
<order> <xs> <ys>`, the last two groups `n` tokens each being `-Tplain`'s own inch strings per
node in node-index order. Every existing column keeps its position, which is what keeps the rank
and order sweeps reading the same file; the parse in `dot/oracle_digest.rs` splits the trailing
`4 * count` fields rather than `2 * count`.

**What the ten are.** Every one of them is a graph with no chain dummy on a row that matters, so
the two causes above cannot both bite and often neither does. They are not the ten "easyest"
graphs in any measured sense — 600 and 600 + 8 are the same two shapes as 0 and 8 with four-
character ids, and they agree, which is a second statement of cause 1: for a shape whose rows
hold one node each, the box width never becomes a constraint length that the answer depends on.

**A disagreement is a finding with its seed, not a reason to stop.** All fifteen of the 408 are
accounted for by the two causes above, neither of which is an algorithmic difference in the pass.

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
- `position.rs` and `position/{rows,ycoords,aux,xcoords,frame}.rs` — `dot_position`: the
  rank heights, the auxiliary graph (`make_LR_constraints`' zero-weight neighbour constraints
  and `make_edge_pairs`' weighted pair per input edge), **a second run of the same simplex with
  `LR_balance`**, `set_xcoords` with `remove_aux_edges`, and the frame. Each step is described
  in the port's own words above it, and every omission is named where it is dropped.
- `position_tests.rs` (the six closed cases, byte for byte at the printed precision and again in
  points), `position_fixture_points.rs` (the twenty fixture seeds' printed rows, and the count
  the pass agrees on), `position_findings.rs` (both measured causes of the disagreements, each
  with its seeds), `position_steps.rs` (one closed
  case per step), `rank_tests.rs` (the six closed cases and twenty fixture seeds),
  `order_tests.rs` (the same twenty, ordered), `mincross_tests.rs` (one closed case per step),
  `class2_tests.rs` (each of `class2`'s three outcomes), `oracle_crossings.rs` (the one
  crossing count both sides of an oracle comparison can be computed with) and
  `oracle_probe.rs` (all three 1000-seed sweeps, `#[ignore]`d, which also holds the shared
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

**Now measured, and the finding is about the table rather than the port.** The ADR's table
exists and `position.rs` reads it, by exactly the formula the ADR and the job name:
`node_width(text_width(id))`. That formula returns **the 0.75 inch default box for every id of
two or three characters**, against an oracle that prints 57.942 points for `n10` and 70.358 for
`n100`. So blocker 1 is **not closed** by shipping the pass: it is *settled as a measurement*,
the escape hatch is named (one constant pair in `text_width.rs`), and the cost is a known
constant on every graph whose ids reach three characters. The "Position" section above carries
the numbers and the seeds.

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
5. **`position.rs`. Done** — `set_ycoords` (rank heights, `ranksep`), then
   `create_aux_edges` = `allocate_aux_edges` + `make_LR_constraints` + `make_edge_pairs`,
   `rank(g, 2, …)`, `set_xcoords`, `remove_aux_edges`. `set_aspect` is a no-op at the
   default ratio and `pos_clusters`/`compress_graph` are clusters, so both are named as
   omissions rather than ported.
6. **The frame. Done** — `dotneato_postprocess` translates so the drawing's lower-left *node
   box* corner is the origin, which for the six closed cases is the offsets in the table
   below. This is part of the answer, not presentation: the closed cases are compared byte
   for byte against `-Tplain`'s printed text, so the translation has to be applied on our
   side too or nothing can match. The vertical offset is **zero**, and that is measured
   rather than assumed: `set_ycoords` already puts the lowest rank's line at its own
   half-height, and a one-node graph on the default box and on `height=1` both print their
   centre unchanged.
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

**The 6-branch's edges**, since `dot/order_tests.rs` uses the name for a *different* graph with the same ranks and the same order: `n0 -- n1, n0 -- n2, n0 -- n3, n1 -- n4, n2 -- n4, n3 -- n4, n4 -- n5` — seven edges. The five-edge graph in that file drops `n2 -- n4` and `n3 -- n4` and draws `n4` and `n5` under `n1` instead; measured against the oracle, it prints `n4` at 27 points where this table's prints 99. See the "Position" section above.
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
**Derived, and labelled as such: 9 200 000**, the topology layer's own ceiling
(`graph-cli capabilities`' `MAX_SCALE_CEILING` is the same constant, and a row above it is
refused outright). It is not a guess at where `dot` stops working — it is where it cannot be
*asked* to work, because the layout's input is a `Topology` and that layer's ceiling bounds the
node count such a value can carry. What is **not** claimed is that `dot` lays out 9 200 000
nodes: it does not, and the differential that would measure where it becomes unusable is the
next job. `graph-core/tests/memory.rs` is the tool for restating this as a measurement, the
way `GRID_CEILING` restates its own.

The row's status is `Status::Implemented`, never `gated`, on the precedent this section already
set and `layout.packing.osage` established: a layout that agrees only where every node box is
the default is not a gated layout. `crates/graph-cli/src/capabilities/registry/unproven.rs`
carries the arm, naming the `unproven` record, because the graph-cli differential and the hash
gate knob are both the next job and an id in no arm at all would be an unstated claim rather
than a decision.

**When the differential lands, the first thing to record is the gap on the `n <= 10` seeds**,
where every label is two characters and every box *is* the `0.75` inch minimum. The Position
section above measures why that set is the one: the width table returns the default box for
every label of up to three characters, so above `n <= 10` the drawing is wrong by a known
constant rather than by anything algorithmic.

# Differential (2026-10-04)

The graph-cli differential landed: `crates/graph-cli/src/oracle_python/dot.rs`, `ENGINES` 7 -> 8,
one `by_engine` arm, and `FRAMED_CLOSED["dot"]` in `harness/gv_frames.py`. The fixture line is
`twopi`'s — the gate model, `source`/`target`, our coordinates under `"dot"` — with **no `box`
column**, so the harness draws the bare graph and both arms read the same width table. That is
the point: `dot` sizes a node from its *rendered label* (Blocker 1), and a pinned `box` column
would have been a second width table rather than a fix.

## The three commands

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine dot --seeds 1000
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/dot-fixtures dot target/gv-dot --differential
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine dot
# dot shard 0/1: 1000 seeds, worst 1.851e+04 points; closed 6 of 6 exact: True
#   layout.dag.dot: 1000 cases, worst 1.851e4, ceiling 1e5: ok
#   closed cases: 6 compared byte for byte: ok
# PASS
```

The third command's exit code is **0**. Every number below is `gv_closed.gap` — the harness's
own metric, imported by the script that produced them, not a re-derivation — run over the
recorded fixture coordinates and the oracle's recorded `graphviz-dot.jsonl`.

## The distribution, and it is a disagreement

| measurement | over the 1000 gate seeds |
|---|---|
| seeds at or under the printed resolution (1e-1 points) | **2** |
| median gap | **1.781e+03** points |
| p90 | **1.127e+04** points |
| worst gap | **1.851e+04** points, seed **587** |
| smallest gap | **0.0** points, seeds 0 and 600 |
| seeds carrying a gap above 1e3 | **559** |
| seeds carrying a gap above 1e4 | **142** |

**Only 2 of 1000 seeds sit at the oracle's own printed resolution, so this arm does not have
the `layout.twopi` / `layout.packing.osage` answer.** Those two state 1e-1 because `-Tplain`'s
five significant digits *are* the floor there — at the largest gate drawing one printed digit is
about 7e-2 points, and the measured gaps never exceeded half of one. Here the median is four
orders of magnitude above that floor, so the number records a **disagreement**.

**The cause is the order and position disagreements counted above, not the formatter.** Those
sections measure 692 of 1000 seeds agreeing node for node on the rank, 408 of those agreeing on
every rank's order, and only **10** of the 408 printing every node centre exactly. This metric
adds the uniform rescale over the whole bounding box, which is why it sees 2 where that
sweep sees 10: a single rank or order disagreement moves one node, and the rescale carries it
onto every other node's coordinate too.

The low tail is the fixture's **small** graphs, not the layout at large: seeds 0 and 600 are the
same two-node graph at two and four character ids, and the next four are 1, 601, 2 and 602 —
the same five shapes again. Past ten nodes the width table returns the `0.75` inch default box
where Graphviz is 3 to 16 points wider, and that width is a *constraint length* in the
x-coordinate simplex, so the two arms constrain each node to a different length (Blocker 1).

**No layout fix is attempted here.** The width table's escape hatch is one constant pair in
`text_width.rs`, outside this crate; the 302 rank ties and 284 order disagreements are
properties of the optimum each implementation reaches, and the "Mincross" section above says
which of the two the port reaches is not isolated. The row is `Status::Implemented`.

## The six closed cases are exact

`FRAMED_CLOSED["dot"]` holds all six, and the harness reports **6 of 6 exact: True** — every
row byte for byte against `-Tplain`'s own five significant digits.

**The 6-branch row needed checking before it could be written, and it is the collision the
"Position" section names.** `twopi_closed.CLOSED_CASES["six-branch"]` is the *five-edge* graph
`n0--n1, n0--n2, n0--n3, n2--n4, n4--n5`, while the table above is the *seven-edge* one. The
two were run through `dot -Tplain` and **print the same coordinates** — measured, not assumed —
because `n2` sits directly above `n4` in the five-edge graph, so dropping `n2--n4` and `n3--n4`
moves nothing. So the row is the answer to the graph the harness actually draws. All six rows
were re-measured here rather than copied from the table above, so the new table is checked
rather than restated.

## The hashgate knob's red run

`GM_MUTATE_DAG_DOT_NODES`, record `hashgate-control-dag-dot-nodes`, tabulated in
`hashgate::knobs::DOT_LAYOUT_STAGES`, held by `hashgate/tests/knob/dot.rs`. The port publishes
no `Params` and has no `impl Stage`, so the re-drawn-model probe is the only one available —
and it is the same shape as the osage control.

```sh
scripts/orch/gr -e GM_MUTATE_DAG_DOT_NODES=1 cargo run -q -p graph-cli -- hashgate --seeds 8
# exit 1
```

**Exactly one stage goes red, and it is this engine's own.** Every other stage — `topology`,
every other layout, every analysis and POST capability, and `transport.wasm.columnar`'s real ABI
check — reports `4-way equal on 8/8 seeds`:

| stage | honest run | with `GM_MUTATE_DAG_DOT_NODES=1` |
|---|---|---|
| `layout.dag.dot` | 4-way equal on 8/8 seeds | **4-way equal on 0/8 seeds** |
| `layout.dag.sugiyama` | 4-way equal on 8/8 seeds | 4-way equal on 8/8 seeds |
| `layout.packing.osage` | 4-way equal on 8/8 seeds | 4-way equal on 8/8 seeds |
| every other stage in the list | 4-way equal on 8/8 seeds | 4-way equal on 8/8 seeds |

The honest run (`--seeds 8`, no control) is **green, exit 0**, and the diverging stage's four
digests are all distinct from each other across the 8 seeds. `coverage.rs`'s allow list no
longer exempts `layout.dag.dot` as `Gap::NoControl`; the exemption would now fail
`the_allow_list_is_sorted_names_real_ids_and_exempts_nothing_that_is_tabled`, which is that
test doing its job.
