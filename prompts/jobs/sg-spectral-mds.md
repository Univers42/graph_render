# Job sg-spectral-mds (agent build, SciGraphs conformance: SPECTRAL_3D, MDS_3D)

Your worktree is cut from `sg-basic3d-spiral-oracle`, which is not on develop yet; do not merge develop.
Read `prompts/jobs/sg-common.md` first. Needs `sg-mt19937` landed (the `n < 4` path below).
Rows: `SPECTRAL_3D` (disparity 0.333) and `MDS_3D` (0.078), both cause `algorithm`.

Why: both references are SciGraphs' own 3D pipelines, not networkx's
(`networkx_layouts.py:249-291`; doc finding 9 says they delegate to networkx, which is wrong):
1. `n < 4`: `_random_layout(n, scale)`, i.e. `RandomState(seed).rand(n, 3) * scale`
   (`basic.py:5-9`, `random::run_seeded` after sg-mt19937).
2. Per connected component, coordinates in **3** dimensions. Spectral uses
   `_spectral_component_coordinates(G, 3)` (`:133`): `L = D - A`, the three smallest non-trivial
   eigenpairs. Every fixture is at most 256 nodes, so dense `np.linalg.eigh` is used
   (`_DENSE_EIG_LIMIT`, `:6`). Signs are pinned by `_fix_eigenvector_signs` (`:66-71`: flip a
   column whose largest-magnitude entry is negative). MDS uses
   `_pivot_mds_component_coordinates(G, 3, 100)` (`:200`).
3. `_pack_component_blocks` (`:218-236`): a cubic lattice, side `ceil(k^(1/3))`, spacing 2.5,
   block scale `(len/biggest)^(1/3)`, components in descending size.
4. `_rescale_positions` (`:238-247`): subtract the mean, divide by the max absolute coordinate,
   multiply by `scale`.
graph-core's `layout.spectral` (`spectral.rs`) and `layout.mds.pivot` (`pivot_mds.rs`) port the
same component kernels at `dims = 2`, with packing "adapted to 2D" (`spectral.rs` header). So
the motor draws a plane where the reference draws a volume. That is the measured shape gap
(grey a line, green a cluster).

Do:
1. Measure both rows. Then dump one fixture's reference 3D coordinates (z spread) to confirm
   the z column is not degenerate before you build anything.
2. RED: a graph-core test. A 6-node path's 3D spectral coordinates equal numpy's `eigh` result
   after `_fix_eigenvector_signs`, to a stated tolerance. Generate the expected values once in
   `ge-python-oracle` and paste them as `f64::from_bits`.
3. GREEN: make the component kernels take `dims` (2 keeps today's bytes, 3 is new). Add
   `pack_component_blocks_3d` and `rescale_to_scale` as one primitive each, in the module that
   owns packing today; `rescale_to_scale` is also `_igraph_fit_positions`' formula. Register
   `SPECTRAL_3D` and `MDS_3D` as new 3D layouts with full `Metadata`, and leave the registered
   2D ids and their oracles byte-identical. Map the two rows to the new ids in the conformance
   `motor.rs`.
4. Eigensolver: bitwise agreement with LAPACK `syevd` is not reachable
   (`docs/decisions/eigensolver.md`). The deliverable is the shape (disparity). A degenerate
   eigenspace (trees and grids have `lambda2 == lambda3`, `_eig_start_vector` `:53`) may stay
   apart: name each such fixture and its multiplicity in the report rather than tuning.

Paths: `layout/spectral*`, `layout/pivot_mds*`, `crates/graph-core/src/registry.rs` (additive),
the conformance `motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs` (these two rows), doc
finding 9 and repair 9, `docs/measurements/sg-spectral-mds.md`.

Done when: both rows' disparity is pasted before and after (target <= 1e-6 except named
degenerate fixtures), the spectral and pivot-MDS Python differentials are still within their
ceilings, and the sg-common done-when holds.
