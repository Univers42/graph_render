# `sg-spectral-mds` — `SPECTRAL_3D` and `MDS_3D` at the reference's own `dims = 3`

**What the job found.** Both rows named a motor layout that is not the row's reference.
`layout.spectral` and `layout.mds.pivot` are graph-core's ports of SciGraphs' *own* spectral
and pivot-MDS kernels at `dims = 2` — the pair `harness/oracle-spectral.py` pins. `SPECTRAL_3D`
and `MDS_3D` are SciGraphs' *own* 3D entries, `_spectral_layout_3d`
(`SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py:249-269`) and
`_mds_layout_3d` (`:271-291`). Nothing anywhere delegates to networkx for these two; finding 9
of `docs/measurements/scigraphs-conformance.md` said so and was wrong, and is corrected there.

So the measured gap was never a solver difference: the motor returned a plane where the
reference returns a volume. Grey was a vertical line, green a cluster at one end.

## Before

`scripts/scigraphs-conformance.sh` on the untouched tree, exit 0, from
`target/scigraphs-conformance/metrics.json`:

| row | tier | cause | Procrustes median | Procrustes max | `f64` bitwise | `f32` bitwise |
|---|---|---|--:|--:|--:|--:|
| `SPECTRAL_3D` | `shape` | `algorithm` | 3.333e-1 | 8.07e-1 | 1/1020 | 1/1020 |
| `MDS_3D` | `shape` | `algorithm` | 7.832e-2 | 5.411e-1 | 0/1020 | 0/1020 |

### The z column is not degenerate — checked before anything was built

`target/scigraphs-conformance/ref/SPECTRAL_3D.f64` on `lesmis` (77 nodes), the first 231
`f64` of the file read as little-endian triples:

```text
x  min -0.6683834689647723  max 1.9638162208631107   spread 2.632
y  min -0.41105331836756215 max 4.838579832867965    spread 5.250
z  min -0.6819587186968824  max 5.0                  spread 5.682
x mean  1.18e-16   y mean 4.98e-17   z mean -1.17e-17
first three triples:
  (1.9638162208631107, 0.19436613659969776, -0.07251021371949662)
  (1.561233788832187,  0.12263902030554971, -0.028557390860543055)
  (0.8186574872292265, -0.015640817523437645, 0.062097972874578106)
```

A z spread of 5.68 against a max of exactly `5.0` (= the dispatcher's `scale`) and a per-axis
mean of 1e-17 is a real third axis, not a padded column: `_rescale_positions`
(`:238-247`) put one point on the cube's surface. The motor's z column was `0.0` on all 1020
coordinates.

## The change

**Component kernels take a width, not a constant.** `layout/spectral/width.rs` holds a
`Width` enum — `Spectral2d`, `Spectral3d`, `PivotMds2d`, `PivotMds3d` — and `scatter`,
`solve_component`, `to_geometry` and the two lattices all take it. It is an enum and not a
`dims: usize` because the two families also disagree about a short block:
`_spectral_component_coordinates:158-159` fills a dimension the solve did not reach by
repeating the last column it did, while `_pivot_mds_component_coordinates:214` writes only
what it solved. (For the 2D arms the two rules coincide — `dims_eff = min(dims, k)` is the
column count either way — so `layout.spectral` and `layout.mds.pivot` keep every byte.)

**Two lattices, two rescale-free/`rescale`-ful endings** (`layout/spectral/pack.rs`):

- `pack_components` — the 2D adaptation already pinned by
  `docs/decisions/eigensolver.md` decision 4 (side `ceil(sqrt(k))`, scale
  `sqrt(n_c/biggest)`, two cell axes). Unchanged, byte for byte.
- `pack_component_blocks_3d` — `_pack_component_blocks` (`:218-236`) as the reference writes
  it: side `ceil(k^(1/3))`, scale `(n_c/biggest)^(1/3)`, cell
  `(slot % side, (slot // side) % side, slot // (side * side))`, spacing 2.5, offset
  `(side - 1) * 2.5 * 0.5`, components in descending size. `libm::pow(x, 1.0/3.0)` and not
  `cbrt`, because `** (1.0/3.0)` is what the reference's `**` evaluates to.
- `rescale_to_scale` — `_rescale_positions` (`:238-247`), also igraph's `fit_positions`
  formula: per-axis mean out, divide by the largest absolute coordinate left, multiply by
  `scale`. `scale` is the dispatcher's 5.0, a constant for the reason `basic_3d::SCALE`
  states.

**Two new ids, `layout.spectral3d` and `layout.mds.pivot3d`** (`registry.rs`, full `Metadata`
in `registry/spectral.rs`), registered through `spectral_stage::spectral_3d` /
`pivot_mds_3d`. Separate ids and not a changed default, because a `layout.spectral` that
drew a volume would move the bytes `harness/oracle-spectral.py` is pinned on.

**`n < 4` is the reference's own guard** (`_spectral_layout_3d:257-258`, `_mds_layout_3d:283-284`):
`_random_layout`, i.e. `RandomState(seed).rand(n, 3) * scale`. It is an explicit branch of
`run_3d(topology, seed)` and not a fallback any other failure could reach;
`random::run_seeded` is the port, exactly as the `RANDOM` row uses it. The registered
defaults pin their own seed (`spectral::DEFAULT_SEED`) and the conformance arm passes the
layout seed, the `sfdp::run_seeded` shape.

**The rows were remapped** in `conformance/rows.rs`, with two arms added to
`conformance/motor.rs` so both rows run at the layout seed.

## After

| row | tier | cause | Procrustes median | Procrustes max | `f64` bitwise | `f32` bitwise |
|---|---|---|--:|--:|--:|--:|
| `SPECTRAL_3D` | `bitwise` | `convention` | **4.667e-16** | 5.865e-1 | 13/1020 | 676/1020 |
| `MDS_3D` | `bitwise` | `convention` | **3.114e-16** | 2.556e-1 | 25/1020 | 903/1020 |

Target was `<= 1e-6`; the medians are five orders of magnitude past it. `lesmis` is at
`max gap 1.78e-7` / `1.17e-7` — one `f32` ULP at the drawing's extent — with all 231 `f32`
coordinates bit-identical.

## The fixtures that stay apart, named with their multiplicity

The job's done-when asks for these by name rather than tuned away. Every one has a repeated
Laplacian eigenvalue **inside the three the 3D row asks for**, so any rotation inside that
eigenspace is a legal answer and no basis can agree. Measured on the reference's own dense
`eigh` (`target/sg-spectral-mds/multiplicity.py`, the generator is left in the tree under
`target/` for the reader):

| fixture | n | λ2 | λ3 | λ4 | multiplicity of the repeated one | `SPECTRAL_3D` | `MDS_3D` |
|---|--:|--:|--:|--:|--:|--:|--:|
| `bipartite` | 14 | 6.000 | 6.000 | 6.000 | 7 | 0.587 | 0.097 |
| `gate-06` | 8 | 1.000 | 1.000 | 1.000 | 3 | 0.371 | 1.0e-5 |
| `gate-07` | 9 | 1.000 | 1.000 | 1.000 | 5 | 0.459 | 0.256 |
| `gate-08` | 10 | 1.000 | 1.000 | 1.000 | 4 | 0.210 | 0.036 |
| `gate-09` | 11 | 1.000 | 1.000 | 1.000 | 4 | 0.135 | matches |
| `gate-10` | 12 | 1.000 | 1.000 | 1.000 | 5 | 0.561 | matches |
| `gate-11` | 13 | 0.557 | 1.000 | 1.000 | 5 (at λ4) | 0.203 | matches |

The multiplicity is one per pendant node for the gate models (a leaf puts `1` in the Laplacian
spectrum) and seven for `bipartite`'s own symmetry. `gate-06` is the smallest witness and the
clearest: nodes 3, 4 and 7 are leaves, so λ₃ = λ₄ = λ₅ = 1, and the motor's first coordinate
matches the reference's exactly (`0.9085 x` on all eight nodes) while the second and third are
a free rotation inside that eigenspace.

The spectrum table was produced by reading `target/scigraphs-conformance/conformance.jsonl`
(one line per fixture, with its edges) and running the reference's own
`networkx_layouts._laplacian_nontrivial_eigenvectors` on each component in the pinned oracle
image. `target/` is gitignored, so the generator is not in the tree; the command is
`docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle python3 <script>` with
`sys.path` on `SciGraphs/core`, exactly as `harness/scigraphs-conformance/sc_reference.py:26`
does it.

Three further fixtures (`tree-balanced`, `gate-02`, `gate-03` on `SPECTRAL_3D`; `tree-balanced`,
`gate-02`, `gate-03` on `MDS_3D`) report a large `max gap` with a Procrustes of 1e-16 to
1e-20: the shape is identical up to a rigid motion and the two metrics disagree about which
motion counts. Same cause, seen by the other number.

**A second, independent effect: `_fix_eigenvector_signs`' tie** (`:66-72`). The rule pins a
column by flipping it when its *largest-magnitude* entry is negative. A path eigenvector is
symmetric or antisymmetric about the midpoint, so several entries have the same magnitude
*exactly*, and which one `argmax` names is decided by the solver's last bit. On the 6-node
path the λ = 2 eigenvector is `[∓0.5, 0, ±0.5, ±0.5, 0, ∓0.5]` — four entries tied at the
peak — and LAPACK's `syevd` and our `tred2`/`tql2` break that tie opposite ways with the
eigenvalues themselves in agreement. This is not eigenvalue degeneracy and it is not
reachable bitwise; the differential in `layout/spectral/tests/three_d.rs` therefore compares
**each column up to its own sign** and says so in its header, and
`a_six_path_eigenvector_has_a_tied_largest_magnitude_entry` checks the tie on real solver
output rather than leaving it as an unexplained tolerance.

## Commands

Every command below was run in this worktree; the exit code is the real one.

| command | exit |
|---|--:|
| `scripts/orch/gr cargo build --release -p graph-cli` (untouched tree) | 0 |
| `scripts/scigraphs-conformance.sh` (untouched tree) | 0 |
| `scripts/orch/gr cargo test -p graph-core --lib spectral` (RED: `run_3d`/`Width`/`scatter` do not exist) | 101 |
| `scripts/orch/gr cargo test -p graph-core --lib spectral` (GREEN) | 0 |
| `scripts/orch/gr cargo test -p graph-core --lib pivot_mds` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- codegen --check` | 0 |
| `scripts/scigraphs-conformance.sh` (after the change, before re-pinning) | 1 — names exactly `SPECTRAL_3D` and `MDS_3D` |
| `scripts/scigraphs-conformance.sh` (after re-pinning the two rows) | 0 |
| `scripts/scigraphs-conformance.sh --break` | 1 — catches `SPRING_3D`, the row it perturbs |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- emit-spectral-fixtures --seeds 1000` | 0 |
| `docker run … ge-python-oracle python3 harness/oracle-spectral.py SciGraphs/core target/spectral-fixtures` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- oracle-spectral` | 0 — `layout.spectral` 984 cases, worst 7.178e-6 ≤ 1e-5; `layout.mds.pivot` 996 cases, worst 3.691e-8 ≤ 1e-7 |
| `scripts/orch/gr cargo fmt --all -- --check` | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8` | 0 — `layout.spectral3d` and `layout.mds.pivot3d` 4-way equal on 8/8 seeds |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q --release -p graph-cli -- hashgate --seeds 8` | 1 — 8 of 8 seeds diverge |

`capabilities --check` exits 1 on this tree with 40 problems, all of the class
`gated, but no <gate> record: run the gate`; four of them name the two new ids and are the
same complaint `layout.spectral` itself carries in the log above them. Those records are
produced by the gate run, not by the tree.

## Not done, and why

- `docs/decisions/eigensolver.md` still records the 2D deviation ("our `Point` geometry is 2D
  only, so we port with `dims = 2` everywhere the reference passes 3", `:27-30`) and the 2D
  packing adaptation (`:242-246`). Both are now true of `layout.spectral` and false of the new
  ids. The file is not in this job's path list, so it is left for a doc job;
  `docs/measurements/scigraphs-conformance.md` rows 13 and 21 and repair 9 carry the
  correction in the meantime.
- `harness/oracle-spectral.py` does not cover `layout.spectral3d` / `layout.mds.pivot3d`; the
  existing 2D differential and its ceilings are unchanged and still pass, which is what the
  done-when asks for.
- The gate row `oracle-spectral-py` in `scripts/orch/rows/develop-full.rows:90` invokes
  `harness/oracle-spectral.py` with one argument while the script reads two
  (`core, directory = sys.argv[1:3]`, `oracle-spectral.py:30`), so that row cannot run as
  written. It is pre-existing and outside this job's paths; the differential was run with the
  second argument spelled out and both layouts are inside their ceilings.