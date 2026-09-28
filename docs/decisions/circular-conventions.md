# Circular / radial — ring, angle and radius conventions

Status: **decided** (Phase 3, `layout.circular.radial`; open questions phase-03 §4 left to
this module, closed here). Scope: `crates/graph-core/src/layout/circular.rs`.

## Context

Phase 3 step 4 (`prompts/phase-03-deterministic-layouts.md`) asks for concentric rings by
BFS depth, and names three decisions with no forced answer: angular order within a ring
(must be deterministic — index order, not traversal-incidental), radius progression
(linear or sqrt), and start angle. P3_SPEC closes two of the three: angular order is
ascending dense index, and radius progression is linear. This module pins the rest —
the start angle, the direction, and the spacing constant — and records where it departs
from `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py`'s
`_circular_hierarchy_layout` (`:693-732`), the read-alongside reference oracle names.

## Decision

- **Ring.** Node `v`'s ring is `Hierarchy::depth(v)` (D-H) directly, no adjustment on top.
  A lone real root is ring 0, dead centre. With two or more roots the virtual root — never
  emitted — is ring 0, and the real roots are ring 1, because `Hierarchy::depth` already
  runs breadth first from the tree's own root, virtual or real: the `+1` for the
  multi-root case is already inside `depth`, not a second offset this module adds.
  P3_SPEC's parenthetical, "ring = depth + (roots ≥ 2 ? 1 : 0)", describes this same
  number; there is nothing further to add once `Hierarchy::depth` is the depth read.
- **Angular order.** Within one ring of `k` nodes, ascending dense index gets slot
  `i = 0..k` in that order — never `Hierarchy::order`'s BFS visitation order, which is
  grouped by ring already but is not required to agree with plain ascending index within
  a ring once the tree is not a simple chain of full levels. P3_SPEC requires index order
  specifically, so slots are assigned by iterating real nodes `0..n` and bucketing by
  ring, which is ascending by construction.
- **Angle formula.** `angle = i * 2*pi / k`.
- **Start angle: 0**, the positive x axis. The first node (ascending dense index) in every
  ring sits on the positive x axis, whatever the ring.
- **Direction:** angle increases with `i`. Whether that reads as clockwise or
  counterclockwise depends on which way `y` points on screen — this module does not fix
  that, the same way `layout/grid.rs` leaves "which way is down" to the renderer.
- **Radius: `ring * RING_SPACING`, `RING_SPACING = 1.0`.** Linear in the ring number,
  independent of node count. Ring 0 is always radius 0, so a lone real root sits exactly
  at the origin whatever its angle would otherwise be.
- **No node ever has an undefined angle.** `k` (`count` in `circular.rs`) is always the
  actual number of nodes sharing that ring, counted from the same pass that assigns
  slots, so it is never zero when an angle is computed.
- **Edges.** `EdgeGeometry::Line`: straight, no interior points. SciGraphs draws no edges
  at all for this layout; this crate's Line convention costs zero bytes and is the same
  choice `layout/grid.rs` and `layout/treemap.rs` make for a layout with no polyline of
  its own.

## Departures from SciGraphs' `_circular_hierarchy_layout`

Read alongside, not copied: three differences, each because SciGraphs' input model
(an arbitrary possibly-undirected, possibly-disconnected graph, roots found by
`_component_roots`/in-degree, level 0 from an unbounded-fan-in multi-source BFS) does not
match `Hierarchy`'s (one repaired tree, forest hung off one virtual root, D-H).

1. **Radius does not normalise to a fixed `scale`.** SciGraphs computes
   `radius = max(level, 0.35) * scale / max(2, max_level)`, so the whole drawing always
   fits inside one `scale` regardless of how deep the tree is — deeper trees get thinner
   rings. This module uses `ring * RING_SPACING` with no such division: ring spacing is
   constant regardless of the deepest ring in the graph, matching `layout/grid.rs`'s own
   choice of an absolute `spacing` per cell over normalising to a bounding box. This is
   simpler to state and to pin in a snapshot hash, and it means adding a deeper subtree
   elsewhere in the same graph never moves a shallower node's radius.
2. **Ring 0 can hold more than one node in SciGraphs, never here.** SciGraphs' directed
   branch takes every in-degree-0 node as a root and runs one multi-source BFS, so several
   roots can land at level 0 together (a real forest without a virtual root standing in).
   D-H's virtual root removes that case for this crate: ring 0 holds at most the one real
   root, and two or more roots move to ring 1 under the (unemitted) virtual root.
3. **Angular order.** SciGraphs iterates `nodes_at_level` in whatever order its BFS
   produced them (`_multi_source_levels`/`_group_by_level`), which depends on component
   visitation and queue order — traversal-incidental, and phase-03 explicitly rules that
   out. This module orders by ascending dense index instead (P3_SPEC), independent of any
   traversal.

Kept the same as SciGraphs: nodes are placed on evenly spaced points around each ring
(`angle = i / count * 2*pi` there, `i * 2*pi / k` here — the same formula), one ring per
BFS depth, and a lone root at the centre.

## Alternatives considered

- **sqrt radius progression** (`radius ∝ sqrt(ring)`), so ring *area* rather than ring
  *radius* grows linearly with depth, evening out density the way `_disk_positions` (used
  for the unrelated 3-D hierarchical layout in the same file) spaces points inside one
  disk. P3_SPEC decided linear; not used here.
- **Radius scaled by ring population** (`_disk_positions`' own trick of sizing each ring's
  share of a fixed disk by its node count), which would avoid the crowding this module's
  Ponytail names. Left as the escape hatch in that Ponytail rather than the default,
  since it would make one ring's radius depend on how many nodes share *another* ring's
  depth level of the same graph — a coupling `treemap.rs`'s per-parent normalisation
  already accepts for boxes, but which a *radial* layout's simplest possible reading
  (radius is a pure function of depth, nothing else) does not need to take on.
- **Start angle from a node property (e.g. hash of id) instead of a fixed 0.** Rejected:
  adds a hash-based degree of freedom with no basis in P3_SPEC or the SciGraphs reference,
  and would not be simpler to state or explain than a single fixed constant.

## Pinned by

`crates/graph-core/src/layout/circular/tests.rs`:
- a single node at the exact centre;
- three children of one root at ring 1, ascending dense index, slot 0 landing exactly on
  `(RING_SPACING, 0.0)` (the one angle this hand oracle can check without leaning on
  `libm`'s own correctness);
- the `forest` fixture: virtual root never emitted, its four real roots at ring 1 radius
  `RING_SPACING`;
- the `cyclic` fixture: D-H's notes reach `Geometry::notes` unchanged;
- every fixture and 48 seeded synthetic topologies: two runs byte-equal, every coordinate
  finite, and every node's distance from the origin equals `ring * RING_SPACING` exactly
  (`f32` rounding aside).
