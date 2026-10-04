# yifan-hu-octree: an octree for Barnes-Hut, then `layout.force.yifan_hu.3d`

One new id, one new tree, one new tick. p12-t4b shipped the 3D arms of five force kernels
but not yifan_hu's own 3D mode, because its Barnes-Hut sat on
`crates/graph-core/src/layout/force/quadtree.rs` — a 4-way tree with a 2-bit slot index, and
making that 8-way would have moved the 65 frozen force-session digests the 2D gate depends
on. So this job builds the octree **beside** the quadtree and puts the 3D arm on top of it.

| new | what | where |
|---|---|---|
| the tree | an 8-way octree over three dense columns, 3-bit slot, same build order | `crates/graph-core/src/layout/force/octree.rs` + `octree/{bounds,build,preorder,charge}.rs` |
| the walk | the many-body force over the octree: same theta test, same fixed-order reduction | `crates/graph-core/src/layout/force/octree/charge.rs` |
| the tick | a 3D sibling of `Sim::tick`: link, charge, center, collide, integrate | `crates/graph-core/src/layout/force/barnes_hut/{sim3d,link3d,collide3d,settle3d}.rs` |
| the arm | `layout.force.yifan_hu.3d`, the 2D arm's coarsening over the 3D settle | `crates/graph-core/src/layout/force/yifan_hu/arm3d.rs` |
| the row | `YIFAN_HU_3D` metadata + the registry shim | `crates/graph-core/src/registry/arms_3d.rs` |

**Nothing in the 2D path was generalised.** The octree shares no line of build code with the
quadtree, and the 3D tick shares no line with `Sim::tick`. The two bargains are the same one
and they are stated in both headers: `octree.rs` and `sim3d.rs`.

## Why a second tree rather than a parameter

`quadtree.rs`'s `node_id(k)` is the key the many-body walk's opening-angle jiggle has hashed
since the session refactor, and the arena the tree flattens into is pinned byte-for-byte by
`force/session/tests/golden.rs`'s 65 digests and by `harness/golden-2d-before.txt`'s 224
lines. A branch factor that became a parameter would move the slot arithmetic inside every
one of those, so the octree is written out beside the quadtree in the quadtree's own shape:
the same `cover` growth of a cube, the same ascending-index insertion (already a `while`
loop, no recursion), the same exact-coincidence chaining, and the same flatten into a
preorder arena that is the only form a pass walks — no stack, no recursion (devil C11).

The one arithmetic difference is the slot: the quadtree's is `y<<1 | x`, the octree's is
`z<<2 | y<<1 | x`. That single bit is what makes this a different tree rather than a
re-spelling — the same point set subdivides along a different axis each level, so the arena,
the cell order and `node_id` all differ, which is why the 3D arm can have its own bytes and
the 2D ones keep theirs.

## The brute-force comparison at `θ = 0`

`octree/tests/forces.rs` is the suite that matters. `θ = 0` makes
`opening_threshold` return `+inf` for every cell, so `open >= l` always holds, the walk
descends to every leaf, and each leaf adds its chain's exact terms — the `O(N²)` the caller
asked for. `at_theta_zero_the_tree_and_a_pairwise_scan_agree` then compares that against a
pairwise scan over the same points, through the **same** `settle` tail, so what the row
isolates is the traversal and not the heuristics.

Agreement is measured against the **condition number** of the sum — the total magnitude of the
terms the pairwise scan added, not the size of its answer. A many-body delta is a
near-cancelling sum (a node inside a cluster is pushed almost equally both ways), so
comparing two such sums relative to the answer would demand more than `f64` can deliver and
the row would be unfalsifiable in the wrong direction. Tolerance: `1e-12` of that work.

Two rows around it:

- `at_theta_zero_the_agreement_is_not_vacuous` is the **negative control**: the same
  comparison at the shipped `θ = 0.9` must *disagree* (asserted `> 1e-6`), or the row above
  would also pass for a walk that returns zeros or never descends.
- `a_coincident_chain_shares_one_settled_gap_so_it_is_not_the_pairwise_sum` is the one place
  `θ = 0` is **not** a pairwise scan, asserted rather than smoothed over: `direct` keys
  `settle` on the chain's head and applies that gap to every member, so a chain of exactly
  coincident points is charged at the head's jiggled separation. The 2D walk makes the same
  choice; the pairwise reference keys on each partner, so the two differ and the test says so.

`a_planar_point_set_gives_the_octree_the_quadtrees_arena` is the strongest structural row in
the module. On a planar point set the octree never splits on `z`, so `x` and `y` halve exactly
as they do in the quadtree, every internal cell has one child, and the two arenas must be
identical — cell for cell, id for id, point for point. It is the row that would fail first if
the octree's build order, chaining or preorder flatten ever drifted from the quadtree's.

## The 3D tick, and what it does not have

`sim3d.rs` runs the same pass order as `sim.rs` (link, many-body, center, collide, integrate)
with d3's own `alpha` decay, the same Jacobi link gather with the degree-bias split, the same
`p + v` collide projection keys, and the same `velocityDecay` tail. Two departures, both
stated in its header:

- **no pins, no gravity.** The 2D tick carries `node.fx`/`node.fy` and a gravity force because
  a *live session* needs them. This tick backs a frozen multilevel solve only, so both are
  absent rather than present-and-zero — applying a force at zero is explicitly not the same
  bytes as not applying it.
- **no threaded tier.** Every pass runs on one thread in one fixed order, so the arm has no
  worker-count dimension to hash-equal across. That is the same position the dense 3D arms
  (`fruchterman_reingold.3d` and friends) are in.

Collide is `O(n log n)` over a **second octree** built on the projected columns, not a dense
pair scan: a dense scan would be `O(n²)` per tick against the 2D pass's `O(n log n)`, and this
arm runs the same 112 ticks per level over up to 12 levels.

The 3D start is a Fibonacci sphere (`seed::sphere_point`), the same formula
`kamada_kawai/start.rs:47` builds, at the 2D spiral's own radius of `12 * sqrt(i+1)`. **The
sphere is required, not decorative**: a 3D start drawn in a plane leaves every `dz` zero on
the first tick, so the many-body walk's z gaps and the link and collide z terms would all take
their jiggle branches instead of their real values, and the octree would degenerate to the
planar case. The z column would be a hash's worth of noise rather than a layout.

## The step-1 digests, before and after

The bar is `prompts/jobs/yifan-hu-octree.md` step 1: the 65 session digests and every 2D
yifan_hu hash must be byte-identical at the end.

### 1. The 65 frozen force-session digests — `force-gate`

```sh
scripts/orch/gr cargo run -q -p graph-cli -- force-gate --seeds 4
```

Before:

```
force-gate: 4 arms
force-gate: wasm artifact /w/.../graph_wasm.wasm sha256 2c265b1d8c77eb95b332fea1c3eab61022c549c6090bd9131553dce2dc8672d6
force-gate: stage=force.session.positions ticks=50 seeds=4 control=none hashed=x then y, little-endian f64, in row order
  native run 1  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  native run 2  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  wasm32 run 1  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  wasm32 run 2  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  force.session.positions: 4-way equal on 4/4 seeds
force-gate: stream equal, 21 batches x 4 arms
PASS
```

After:

```
force-gate: 4 arms
force-gate: wasm artifact /w/.../graph_wasm.wasm sha256 5f9bb6656635420698f96bc9e4d9a718cf3ecb40d94151f442bd3449e2b6c8b5
force-gate: stage=force.session.positions ticks=50 seeds=4 control=none hashed=x then y, little-endian f64, in row order
  native run 1  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  native run 2  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  wasm32 run 1  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  wasm32 run 2  digest 656046e604a913c29e222cf08265d61c4fc8695b50944d7f3f070ecba175e34e
  force.session.positions: 4-way equal on 4/4 seeds
force-gate: stream equal, 21 batches x 4 arms
PASS
```

**The only line that moved is the wasm artifact's own sha256** (`2c265b1d…` before,
`5f9bb665…` after), which had to: a layout was appended to the registry and the octree is
linked into the artifact, so it is a different file. All four arm digests are `656046e6…`
before and after — byte-identical — and so is the stream row. The digests behind them are
`force/session/tests/golden.rs`'s `FROZEN`, reproduced in full by:

```sh
scripts/orch/gr cargo test -p graph-core session::tests::m1a
```

### 2. Every 2D yifan_hu hash — `golden-2d`

```sh
scripts/orch/gr cargo build --release -p graph-cli
scripts/orch/gr bash /w/harness/golden-2d.sh /w/target/golden-after.txt
diff <(grep -v '^#' harness/golden-2d-before.txt) /w/target/golden-after.txt
```

**224 lines, zero differences.** The eight `layout.force.yifan_hu` rows are the ones this job
names, and they are unchanged:

```
layout.force.yifan_hu 0 88ce9e5d1665a945eba1d3e0b1ead05e32000c5aac5b611aa5068a96fd7ea944
layout.force.yifan_hu 1 12f4e6d04150275531a46c5395fbc4ed4cf2db6d4ce6ae0b13f60e3007364680
layout.force.yifan_hu 2 af7908438b906f729a4cf2be8a5cb80ecc03ae8786d14e784ca2f9530f382e50
layout.force.yifan_hu 3 b244b9913f36d85752a369a9c6b5c54fb90e00aae4adb94b62bdc02222c470de
layout.force.yifan_hu 4 b0d4cc026b4f24189f8026ad01063ce7474c64bf7c292cc34337750957eec710
layout.force.yifan_hu 5 1bcf18fe2f89a882b18be61d1c2d2ab27e22c3844d1c246fc759b062b55aed4e
layout.force.yifan_hu 6 56e9f56905ac4be636a2c3073b3eace10d0493f2cf4a5f2a96f9df876f70d5a3
layout.force.yifan_hu 7 3709d691eeb4cedf60d40b65c91a44497dc7f19b90a9d7c6735ea955ebfe1fbd
```

### 3. `hashgate --seeds 8`

```sh
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
```

All **65 existing per-stage rows are byte-identical**, and one row is added:

```
  layout.force.yifan_hu.3d: 4-way equal on 8/8 seeds
```

The header's `stages=` list gains `layout.force.yifan_hu.3d` **at the end of the force block**
— `LAYOUTS` is append-only, so the new row took index 47 and every existing index kept its
position. The record's own aggregate digest moved, as it must when the stage set changes:

```
before: 21379ad5a93c08709b43468f0253e3ee82c04eac332680050d91b6e14b5ae824
after:  7178c68ed2a2ad8e2206a9213cfb7fa3f97ab049b87637cf427251e46b38bfa3
```

That aggregate covers all stages, so it is a different set of bytes by construction. The
per-stage rows are the claim that matters, and they did not move.

**One intermediate run of this pair is not the reported one.** `force-gate`'s artifact sha256
moved a second time between the first after-run and the last (`ebc667a5…` → `5f9bb665…`)
because `charge.rs`'s `settle` was split to stay inside the 40-line cap *after* the first
after-run. The split is a pure refactor of one function — the octree unit suite, which
compares the walk against a pairwise scan through that same `settle`, is the judge and it is
green — but the numbers above are from the final tree, not the first.

## The 3D arm's own rows

`crates/graph-core/src/layout/force/yifan_hu/tests3d.rs`, all green:

- `the_3d_arm_returns_three_finite_columns_of_equal_length` — 40 nodes in, 40 finite values
  per axis out.
- `all_three_axes_carry_a_live_column` / `the_3d_arm_is_not_the_2d_run_with_a_column_pasted_onto_it`
  — the two rows a 3D arm can fail while every structural test passes. Every axis spreads, `z`
  is not the 2D arm's `x`, **and the in-plane columns differ from the 2D arm's**, which is the
  only thing that distinguishes a real 3D run from the 2D picture with a column attached.
- `the_3d_arm_is_repeatable` — same topology, same bytes.
- `the_3d_arm_coarsens_on_the_2d_arm_s_levels` — a path of 200 coarsens `200, 100, 50, 25, 13`
  in both arms, because the hierarchy is a property of the graph and not of the dimension.

`crates/graph-core/src/layout/force/barnes_hut/tests3d.rs` adds the pass-level rows: the
centre pass shifts each axis by its own mean, a link between two coincident nodes has a
finite three-axis force and its two endpoints move in opposite directions, and two overlapping
nodes separate on **every** axis by exactly the negation each gets from the other (the 2D
canonical-orientation claim at three axes).

## What this arm does not claim

- **No coordinate oracle, and none is claimed.** This is not sfdp; sfdp's own force model
  (`K²/d` repulsion, adaptive step, weighted edge collapsing) is not reproduced here, exactly
  as for the 2D arm. SciGraphs' `_yifan_hu_layout` `'3'` mode calls sfdp at `dim = 3`, so no
  sfdp output was compared in any dimension.
- **`scale_ceiling` is inherited, not re-measured.** `FORCE_CEILING` is the 2D arm's. The
  octree makes the many-body pass `O(n log n)` in three dimensions exactly as in two, and the
  constant grows by a factor near 1 that a measurement would not resolve — the same argument
  the other 3D force arms make.
- **The ledger status is `implemented`, never `gated`.** `harness/stress-d3.mjs` is a 2-axis
  metric, so the row's own evidence is its unit rows, and `verdict::Evidence::oracle_record`
  has no arm for a 3D yifan record in any case.
- **No threaded tier**, so unlike the 2D arm there is no worker-count dimension to hash-equal
  across: one schedule, one set of bytes.

## Commands

```sh
scripts/orch/gr cargo fmt --all --check
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
scripts/orch/gr cargo test --workspace --no-fail-fast
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown

# this job's own rows
scripts/orch/gr cargo test -p graph-core --lib octree
scripts/orch/gr cargo test -p graph-core --lib 3d
scripts/orch/gr cargo test -p graph-core session::tests::m1a

# the digests of step 1
scripts/orch/gr cargo run -q -p graph-cli -- force-gate --seeds 4
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
scripts/orch/gr bash /w/harness/golden-2d.sh /w/target/golden-after.txt
```