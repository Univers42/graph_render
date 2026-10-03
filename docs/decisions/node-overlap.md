# Node overlap removal: where it goes, what it reads, how it works

Status: accepted, 2026-10-03. Ruled by the `devil` agent (verdict below, §7).

The user's report, 2026-10-03: *"sometimes there are too many nodes all compressed in the same
space, it's unreadable"*. The motor places points; it never separates discs. `post::fdeb` and
`post::mingle` bundle edges and leave every node exactly where the layout put it, and the one
live collide term (`layout/force/params.rs:49`, `collide_radius`) is a force inside an iterative
solver — it never runs on a finished layout, and it is only reachable by asking for the live
force layout. So a layout that compresses, and a layout that is not the force layout, both ship
with overlapping nodes. There is no stage whose job is to fix it.

The paper is Gansner and Hu, "Efficient, Proximity-Preserving Node Overlap Removal", JGAA
14(1) 2010 — PRISM. The reference implementation read for behaviour only is Graphviz 16.1.0 at
`$GM_SCRATCH/refs/graphviz-16.1.0`, `lib/neatogen/overlap.c` (PRISM, under
`HAVE_GTS && SFDP`, `overlap.c:17`) and `lib/neatogen/adjust.c`. It is EPL-1.0: read, never
copied. Nothing below is a transliteration of it.

## 1. Placement: a node-moving POST, not a new stage

**Decided: (b), a POST pass under a revised contract.**

A new stage after LAYOUT is the cleaner contract on paper and costs more here, for three
measured reasons:

- The `Stage` trait takes a topology, not a geometry: `stage.rs:23-31`,
  `fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError>`. A stage
  that consumes LAYOUT's output does not fit it, so it needs a second pipeline type, a new
  arity for `PipelineRun::stages()` (`stage.rs:78-83`, a fixed `[_; 2]` consumed by
  `harness/wasm-run.mjs`), and a new `PipelineRun` field. That is a wider public change than
  anything the POST route needs.
- A stage outside the POST registries gets no ABI. There is no `gm_stage_run`; the surface is
  `gm_post_count` / `gm_post_id` / `gm_post_run` indexing
  `graph-wasm/src/post/registry.rs:23` (`CAPABILITIES`, append-only). A new stage is invisible
  to wasm, to the SDK, and to the hash gate unless all three are edited anyway.
- POST is already plumbed for this. `hashgate/stages.rs:61` extends `staged::posts()` from
  `CAPABILITIES`, so appending one row is hashed per stage, four ways (native ×2, node ×2), with
  **no existing digest moving**.

Per-stage hashing is kept. The pass is its own entry in `post::POSTS`, so `post::find(id)`
resolves it and the hash gate has a stage name of its own.

**The cost, named.** POST's contract says a pass does not move nodes, and that claim is
asserted over the whole registry in `post/tests.rs:111-151` (`bundled.geometry.nodes ==
input.nodes`, "a post pass may not move a node"). The revision is not to delete that assert but
to make it conditional on a declared flag (§3), so every existing pass keeps identical teeth and
the claim becomes a field rather than an assumption.

## 2. Node sizes in layout units, and what a `Point` counts as

Node geometry is three variants in one snapshot (`docs/contract/`,
`graph-contract/src/geometry.rs:108-136`): `Point { x, y }`, `Circle { x, y, r }`,
`Box { x, y, w, h }`. Sizes are in **layout units** — the same units as `x` and `y`, the units
SCALE and GEOMETRY work in. SCALE does not convert units (`scale/mod.rs:1-19`: LOD, simplify,
adaptive — no projection), so nothing downstream has to know what a node's size means.

The rule, one line per kind:

| kind | the extent used | where it comes from |
| --- | --- | --- |
| `Circle` | `r` | the layout's own column |
| `Box` | the circumscribed radius `√((w/2)² + (h/2)²)` | the layout's own columns |
| `Point` | `params.point_radius` | **the caller**, default `0.0` |

A `Point` has no size in the snapshot, so the pass cannot invent one. It counts as a disc of
radius `point_radius` — a caller-supplied number in layout units, default `0.0`, which makes the
pass a **no-op on a `Point` layout at the default**. That is the honest default: a point has no
extent, and guessing one would make the pass move nodes on layouts that never asked. A renderer
that draws a 12 px dot passes the radius it actually draws.

The `Box` radius is the circumscribed circle, not the inscribed one. Two boxes can be disjoint
while their bounding discs overlap, so this **over-separates a `Box` layout**, never
under-separates. That is the safe direction for an invariant test and the wrong direction for
fidelity to the reference; the reference works on circles too (`getSizes` returns one radius
per node, `adjust.c:598`). Ponytail marker on it.

## 3. The contract revision, in three fields

- **`moves_nodes: bool` on `post::Metadata`** (`post/mod.rs:40-55`). `false` for every existing
  pass. `post/tests.rs` `check_output` keeps `bundled.geometry.nodes == input.nodes` for every
  `moves_nodes: false` row and asserts a different, weaker-but-stated set for a `true` row: edge
  kind unchanged, z column carried, node and edge geometry checked, and no node left outside the
  grid. The edge-shaped bounds (`pairs <= m(m-1)/2`, `unbundled <= m`, `tests.rs:148-149`) become
  per-capability declared bounds. **They are not deleted.**
- **A z column is refused**, with `StageError::Param { name: "geometry.z", rule: "must be absent" }`,
  no new enum variant. Projecting 2D discs under a z column answers a question nobody asked, and
  `docs/decisions/contract-3d-verdict.md` condition 6 stays unamended: no POST pass rewrites node
  columns. Three layouts emit z; they are refused, not damaged.
- **`Geometry::with_nodes`**, alongside `with_edges` (`layout/mod.rs:105-112`). That doc claims
  "there is nowhere else to build one"; this makes it false, so `with_nodes` is documented as
  reachable **only** by a `moves_nodes: true` pass, and the invariant `with_edges` encoded moves
  to the flag. Without that the z-preservation guarantee degrades from a type-level fact to a
  convention.

`Bundled`'s two `u32` counts are reused with per-pass semantics, which the type already documents
(`post/mod.rs:78-89`, "FDEB: … MINGLE: …"): `pairs` = overlapping pairs resolved, `unbundled` =
pairs still overlapping by more than the tolerance when the iteration cap was reached. The counts
never cross the ABI (`registry::run` returns `Bundled` to Rust; `snapshot()` drops it).

## 4. The algorithm: uniform-grid sweeps, not PRISM

**Decided: a deterministic uniform-grid separation pass. PRISM is not implemented, and the
reason is stated rather than hidden.**

PRISM needs a Delaunay triangulation. This crate may not take a new dependency, and the tree has
no triangulation code to reuse — `post::grid_index` is a uniform grid, not a triangulator.
Implementing Delaunay by hand would be a new subsystem with its own degenerate-case surface,
which is a worse trade than a grid sweep that is auditable end to end. So: **uniform-grid
sweeps**, and the ponytail marker says which inputs they get wrong.

The pass is `O(n · k)` per sweep, `k` = the nodes in one node's 3×3 cell neighbourhood:

1. **Size every node** by §2 into one radius per node, in dense node order.
2. **Bucket** the nodes into a uniform grid whose cell side is `2 · (max radius + margin)`, so
   any two nodes whose discs could touch are within one 3×3 neighbourhood by construction, not
   by search. A counting sort into CSR buckets — dense order in, dense order out, no hashing, no
   `HashMap` in any output order (D2, D4).
3. **Sweep** in fixed dense-node order. For each node, gather its neighbours from its own cell
   and the eight around it, and displace it by the sum of the separation vectors of the discs it
   overlaps — Jacobi, one read-only pass over the previous positions, applied afterwards
   (D10: the kernel is in gather form; a node's new position reads only the old array).
4. **Repeat** up to `max_iterations`, stopping early when a sweep resolves nothing. Ties on
   "deepest overlap" break by dense index, never by iteration order.

Determinism (D1–D10): `libm::sqrt` and arithmetic only, no `mul_add`, no `powi`; every
reduction in dense node order with the neighbour list in dense cell order, so the sum is
fixed-order; no clock, no RNG; `usize` never on the wire. Same input, same bits, on every target.

`max_iterations` is the iteration cap; `margin` is the extra gap, in layout units, added to every
pair's required separation. Defaults: `margin = 0.0`, `max_iterations = 16`. A non-finite or
negative `margin`, a zero `max_iterations`, or a negative `point_radius` is `Err(Param)`, not a
clamp.

**Ponytail markers, all three required:**

- *(iteration cap)* the pass is iterative, so it can stop with overlaps still present. Failing
  input: the adversarial cases in §6 — all nodes on one point needs more sweeps than a normal
  layout. Direction: under-separates, never over-separates. Escape hatch: `max_iterations`, and
  `Bundled::unbundled` counts exactly what is left.
- *(uniform grid)* a grid with cell side `2·(r_max + margin)` has `O(n)` cells for uniform radii
  and `k` grows with radius spread — one huge node makes its cell enormous and its 3×3
  neighbourhood near-global. Direction: cost, not correctness; a huge node still separates
  correctly, just slowly.
- *(circumscribed `Box`)* see §2. Over-separates `Box` layouts.

## 5. Opt-in, so today's hashes do not move

The pass is registered and hashed, and **not** in any default pipeline. A caller names it by id
(`post::find("post.separate.grid")`, `gm_post_run` by index). No layout's output changes, no
existing `Bundled` changes, no existing digest changes — verified by running the hash gate at 8
seeds before and after (`docs/measurements/ux-overlap.md`).

## 6. The invariant and what it is measured against

**After the pass, no two nodes overlap by more than the tolerance.** Stated as an exact
property, not a "usually". The check is a brute-force all-pairs scan, which is `O(n²)` and is
therefore a **test instrument, never product**: it runs at 2 000 nodes at most, and the pass
itself never calls it.

Inputs covered:

- every fixture under `fixtures/`
- 100 seeds of random graphs
- two adversarial cases: every node on one point, and every node on one line

The negative control is the `GM_MUTATE_OVERLAP_*` knob, which perturbs the pass in a way that
must leave an overlap behind; the invariant row must then go red.

Quality is measured two ways and both go in `docs/measurements/ux-overlap.md`: **mean
displacement** (how far the pass moved nodes on average, in layout units, and as a fraction of
the layout's own mean pairwise distance) and the **stress ratio** (layout distances against graph
distances, before and after). Recorded at 1 000, 10 000 and 100 000 nodes with the wall time.

## 7. The `devil` verdict

`devil` returned **PROCEED-WITH-CONDITIONS** on (b), with these conditions, all met:

1. Implement as POST, registered in `post::POSTS`, graph-wasm `CAPABILITIES` and graph-cli
   `capabilities/post.rs` — append-only, no existing digest moves.
2. `moves_nodes: bool`; `post/tests.rs` branches on it, keeping node-equality and z-equality
   asserts for every `moves_nodes: false` pass. Deleting the edge-shaped bounds is **refused**;
   they become per-capability declared bounds.
3. Refuse a z column via `StageError::Param`; no new `StageError` variant;
   `contract-3d-verdict.md` condition 6 stands unamended.
4. `Geometry::with_nodes`, documented as reachable only by a `moves_nodes: true` pass;
   `layout/mod.rs:105-112`'s "nowhere else to build one" updated, since this makes it false.
5. `Bundled` doc states per-pass semantics for both counts, including "still overlapping at the
   cap", plus a non-zero `degradation` string and a Ponytail marker.
6. Ledger row's `edges` documented as pass-through; `capabilities --check` green, no duplicate ids.
7. A new hashgate digest, identical native vs wasm32 (D1), with a negative control that fails.

The ruling also flagged the failure nobody had named: "pairs still overlapping at the cap" is a
heuristic failure, so it needs a declared `degradation` string and a negative control, not a
silent partial result. That is §4's iteration-cap marker and §6's knob.

## 8. The oracle, and what it can and cannot say

The oracle image is built with `libgts-dev` so `-Goverlap=prism` runs real code
(`overlap.c:17` gates PRISM on `HAVE_GTS && SFDP`; without GTS the real body compiles out and
every `-Goverlap` value hits the stub that prints "not built with triangulation library",
`overlap.c:585-610`). `--with-gts=yes` is passed explicitly **and asserted** on configure's own
summary line, because `PKG_CHECK_MODULES` reports failure by setting `use_gts=No` rather than by
failing (`configure.ac:1532-1544`) — a silent build with prism still stubbed is the failure mode
worth being loud about.

**A measured trap, recorded because it cost a build cycle:** with GTS built,
`adjustMode[1]` **is** PRISM (`adjust.c:775-800`), and `getAdjustMode` maps `overlap=false` to
`adjustMode[1]` (`adjust.c:846-849`). So in a GTS build `-Goverlap=false` and `-Goverlap=prism`
are the same algorithm and produce byte-identical output. Prism is proved live by its output
**differing from `voronoi`** and from the no-GTS build — not by differing from `false`. A
harness that checked the obvious thing would have concluded prism was still a stub.

The differential is a **ceiling, not a bitwise match**: Graphviz's PRISM is a stress-majorising
algorithm on a Delaunay triangulation, this is a grid sweep, and the two will never agree
coordinate for coordinate. What is compared is quality — mean displacement and stress ratio, ours
next to Graphviz's on the same input.