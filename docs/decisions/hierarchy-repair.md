# D-H — messy hierarchies are repaired, and every repair is recorded

Status: **decided** (Phase 3, user decision D-H; orientation from user decision D-Q1).
Scope: `crates/graph-core/src/layout/hierarchy.rs`, the tree that the hierarchy layouts
lay out, and note codes 1 and 2 (`docs/decisions/snapshot-notes.md`).

## Context

Phase 3 step 1 (`prompts/phase-03-deterministic-layouts.md`) says real data is not a
clean single-rooted tree. A graph can have several roots. It can have no root, because of
a cycle. A node can have several parents, which the hierarchy CSR allows and a tree
layout does not. The prompt makes each case "a decision, not an error to swallow". It
also lists "a hierarchy fixture (cyclic, multi-parent, forest) has no obviously right
answer" as a reason to stop and ask. That question was asked, and the user decided.

## The user's decisions, as given

D-Q1, `child_of` orientation, option (c):

> Direction decided per wire type at classification time. `child_of` edges are flipped
> when the index is built so the hierarchy CSR always reads parent → children; `parent`,
> `parent_of` and `*hierarchy*` keep source = parent.

D-H, messy hierarchies are REPAIRED and RECORDED:

> - Parent candidates come from the hierarchy CSR (after D-Q1). Self-loops are ignored as
>   parents.
> - Multi-parent: a node keeps the parent from its FIRST hierarchy edge in topology edge
>   order; every other parent edge is dropped and recorded (note code 2).
> - Cycles: after multi-parent repair every node has ≤1 parent, so a cycle is a
>   parent-pointer cycle. Break it deterministically: the lowest dense index node on the
>   cycle becomes a root, its parent edge is dropped and recorded (note code 1). Process
>   cycles in ascending dense-index order.
> - Roots: nodes with no (kept) parent, ascending dense index. Children of a node:
>   ascending dense index (= input order, which is what d3.stratify gives).
> - Forest: with exactly ONE root, that root is the tree root. With ≥2 roots, all roots
>   become children of ONE hidden virtual root that is never emitted (no geometry); every
>   layout lays out the virtual-rooted tree, and the oracle arms build the identical
>   virtual root. (For circular: … ring = depth + (roots ≥ 2 ? 1 : 0).)
> - Nodes with no hierarchy edges are roots (leaves of the virtual root).

## Decision, as implemented

`Hierarchy::of(&Topology)` runs these steps in order. It is O(n + m).

1. **Orientation.** Each edge keeps a `child_first` flag. The flag is set when the
   lowercased wire type is exactly `child_of` (`child_first_from_type`). The hierarchy
   CSR files an edge under `Topology::parent(e)`, and the repair reads both ends through
   `parent(e)`/`child(e)`. Nothing reads `target` as the child. The edge's
   `source`/`target` stay as they arrived.
2. **Candidates.** Every edge in the hierarchy CSR, except a self-loop. A self-loop is
   never a parent and never produces a note.
3. **One parent each.** A node keeps the parent of its hierarchy edge with the **lowest
   edge index**. "First in topology edge order" means exactly that. It does not mean the
   first edge met while walking the CSR rows in parent order, which would quietly use the
   parents' dense order instead. Every other parent edge is dropped as note
   `(2, edge)`, and that includes a repeated parallel edge from the same parent.
4. **Cycles.** Each node now has at most one parent, so a cycle is a parent-pointer cycle
   and no two cycles share a node. One pointer walk per unseen node finds every cycle.
   Each cycle is broken at its lowest dense index node, in ascending order of that node.
   That node's parent edge is dropped as note `(1, edge)`, and the node becomes a root.
5. **Roots and children.** Roots are the nodes with no kept parent, in ascending dense
   order. An isolated node is a root. Each node's children are in ascending dense index.
6. **The root.** A single real root is the tree's root. With two or more real roots, the
   roots hang, in ascending order, off one **virtual root** at dense index `n`. The
   virtual root is an index, never a string id, so it cannot collide with a real node.
   It has no geometry and is never emitted. With `n = 0` there is no root.
7. **Depth.** Depth is computed breadth first from the root. Under a virtual root the
   real roots are at depth 1, so the circular ring is simply `depth`.
8. **Notes.** The notes are sorted by `(code, index)`. A layout passes them to
   `Geometry::notes`, and `layout::snapshot` writes them into the snapshot on both faces.

The rest of the API is `roots`, `root`, `virtual_root`, `parent`, `parent_edge`,
`children`, `depth`, `max_depth`, `order` (breadth first, virtual root first) and
`notes`.

## Alternatives considered

- **Refuse a cycle or a second parent.** This is the prompt's other option. A refusal
  gives no picture at all for data that has a sensible drawing, and real host data is
  messy. The user chose repair.
- **Repair silently.** This hides a degraded result. The user required every dropped
  edge to be recorded in the snapshot itself (D-N), not in a log.
- **Keep the parent with the lowest dense index, or the first one met walking the CSR.**
  Both make the parent depend on node order, which is not the decision. The walk order
  is the parent's dense order. The dense-order and edge-order fixture shows the
  difference: in `cyclic.json`, `m` keeps `b` (edge 1) over `a` (edge 3), even though
  `a` comes first in dense order.
- **Break cycles by a minimum feedback arc set or by degree.** Minimum feedback arc set
  is NP-hard. A degree rule needs its own tie-break. The lowest dense index is total,
  O(n), and easy to explain.
- **Keep a node under all its parents, as a DAG.** That is the layered layout of
  Phase 5. A tree layout cannot draw it.
- **Lay out each component separately and space the components apart.** The prompt
  suggests SciGraphs' `_COMPONENT_SPACING = 2.5` for this. The virtual root replaces it:
  every layout lays out one tree, and the d3 oracle arms can build the same
  virtual-rooted tree. That way the differential covers forests, and no spacing constant
  is needed that the oracle cannot check. This is a deviation from the prompt's
  suggestion, authorised by D-H.

## Why

- **Deterministic and total.** Every choice keys on the edge index or the dense index.
  Both are total orders, and no hash iteration is involved.
- **Visible.** Every dropped edge is a note with its edge position, on the binary and
  JSON faces, and it is part of the hash.
- **Order-bound only by decision.** For a clean hierarchy (no cycle, one parent per
  node), neither node order nor edge order moves any node's parent or depth. A test
  reorders the three acyclic fixtures to check this. Only a messy hierarchy depends on
  order, and only in the two places above.

## Pinned by

- `fixtures/hierarchy/{tree-balanced,tree-degenerate,forest,cyclic}.json`. The format is
  nodes `{id, weight}` and edges `{id, source, target, type}`. Each edge keeps its raw
  wire `type`, so an oracle arm applies its own `child_of` flip and its own repair.
- `crates/graph-core/src/layout/hierarchy/tests/fixtures.rs`. `cyclic.json` contains:
  - a self-loop;
  - a multi-parent node whose dense order and edge order disagree;
  - a repeated parallel parent edge;
  - two cycles, one of them closed by a `child_of` edge.

  Its notes are exactly `(1, 7), (1, 8), (2, 3), (2, 4)`, with roots `r, x, c` under a
  virtual root.
- `crates/graph-core/src/layout/hierarchy/tests.rs`. Unit cases cover each rule, `n = 0`,
  `n = 1`, and one tree plus an isolated node. Property tests over 64 seeded models check
  that:
  - every node is reached exactly once from the root;
  - kept parents form a forest;
  - the dropped edges are exactly the recorded ones;
  - renaming every id changes nothing.
