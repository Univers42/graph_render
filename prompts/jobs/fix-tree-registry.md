# Job fix-tree-registry (agent build: review-layout-tree L-25..L-28)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-tree.md` (ids `L-NN`;
lines may have moved; the branch is cut after the 3D registrations land, which add rows to
`registry/three_d.rs` and `registry.rs`). Edit those two files additively only.

1. **L-25, MINOR.** `registry/three_d.rs`: `BASIC_3D_CEILING`'s doc covers the graph-free 3D
   placements, but `HIERARCHICAL_3D` reuses it. Give it its own ceiling from a measured
   `memory.rs`/`bench` row (paste it), or widen the doc to name the basis it really has.
2. **L-26, MINOR.** `registry/hierarchy.rs`: one ceiling backs four rows; its doc names three. State
   the shared basis in the ceiling's doc.
3. **L-27, MINOR.** `RADIAL_CEILING` and `BASIC_3D_CEILING` are both "the largest size `bench`
   accepts", written twice. Derive both from the one `bench` constant (or a `pub const` the motor
   owns that `bench` also reads; graph-core must not depend on graph-cli).
4. **L-28, MINOR.** `registry.rs` `LAYOUTS` order is load-bearing (`graph-wasm/src/exports/build.rs`
   maps by index; `bench/campaign.rs` `DEFAULT_ARM` is `LAYOUTS[3]`) and only its length is checked.
   GREEN: a `const` assertion (or a test) on the id at each index a caller depends on. RED: swap two
   rows locally and show it fails; do not commit the swap.

No output moves: `capabilities --check` problems filtered by the touched rows (empty),
`codegen --check` (exit 0), `hashgate --seeds 8` (exit 0) and its `GM_MUTATE_REFERENCE_DEGREE=9`
control (non-zero).

Paths: `crates/graph-core/src/registry.rs`, `crates/graph-core/src/registry/{three_d,hierarchy,radial}.rs`,
the `bench` constant's file if item 3 moves it, `docs/measurements/fix-tree-registry.md`.

Done when: fix-common's done-when; every id above has a row.
