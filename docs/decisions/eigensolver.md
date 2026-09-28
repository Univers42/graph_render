# D-EIG — the two-tier eigensolver for spectral layout and Pivot MDS

Status: **decided** (Phase 6, branch `p6e`; decision source: the `devil` verdict
`a9f98adabc4ed50d5` (`/home/user/P6_DEVIL.md`) PROCEED-WITH-CONDITIONS on C1/C4, and
`P56_SPEC.md` Phase 6 decision 4, which delegates to that verdict's RECOMMENDATION
section verbatim). Written before any `linalg` code, per C4 and rule 0.6.
Scope: `crates/graph-core/src/linalg/*`, `crates/graph-core/src/layout/spectral.rs`,
`crates/graph-core/src/layout/pivot_mds.rs`.

## Context

`docs/decisions/eigensolver.md` is the file the devil verdict names by path. Phase 6's
spectral and Pivot MDS layouts both reduce to a symmetric eigenproblem: spectral needs
the smallest non-trivial eigenpairs of a graph Laplacian, Pivot MDS the largest
eigenpairs of a small dense Gram matrix. `prompts/REFERENCES.md` Tier 2 names
`SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py` as "the single
highest-value file in the corpus": the math is scipy's, but the determinism engineering
— fixed start vector, sign pinning, residual verification, per-component solving,
rejecting `which='SM'` — is SciGraphs' own and is carried forward verbatim (rule 0.6,
`prompt.md` §6). Reference re-read in full for this ADR; line numbers below are
re-verified against `/home/user/SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py`
today (guardrail 3), not copied from the prompt or the devil verdict, both of which the
devil's R11 already flagged as drifted.

## Scope

- **Spectral**: `L = D − A` per connected component; the smallest `dims = 2` non-trivial
  eigenpairs. **Deviation**: the reference computes 3D (`dims=3`,
  `_spectral_layout_3d`); our `Point` geometry is 2D only, so we port with `dims = 2`
  everywhere the reference passes 3.
- **Pivot MDS**: the largest `dims = 2` eigenpairs of the `k×k` matrix `Cᵀ C`,
  `k = min(100, n_c)` (`_MDS_PIVOTS = 100`). Always the dense tier — `k ≤ 100 < 256`.
- **Adjacency (C6)**: undirected, parallel edges collapsed to one edge of weight 1,
  self-loops dropped. This is what `nx.Graph()` gives `nx.adjacency_matrix` when
  SciGraphs builds its graph from our edge list — a plain `Graph`, not a `MultiGraph`,
  so parallel edges overwrite rather than accumulate, and `nx.Graph` never stores a
  self-loop weight through `add_edge(v, v)` the way our `Topology::incident` does
  (`index.rs` lists a self-loop twice in incident rows; that is for the JS `neighborhood`
  parity, not for adjacency here). Applies to the spectral Laplacian and to Pivot MDS's
  BFS distances. (FA2 mass and the force link set are p6f's concern, out of this branch.)
- **Components**: found by BFS from the lowest unvisited dense index; members sorted
  ascending after collection (`_connected_component_indices`, `networkx_layouts.py:36-45`,
  the sort at `:44`); components are then ordered by their own minimum index (the order
  they are discovered in, since we scan indices ascending). Packing (below) additionally
  stable-sorts by `(-size, min_index)`.

## Tier D — dense (`n_c ≤ 256 = _DENSE_EIG_LIMIT`, and every Pivot MDS `k×k`)

- **Primary**: Householder tridiagonalisation (`tred2`) then implicit-shift QL with
  eigenvector accumulation (`tql2`) — EISPACK's algorithm, ported line-for-line from
  `Jama.EigenvalueDecomposition`'s symmetric path (`/home/user/refs/jama-1.0.3/Jama/EigenvalueDecomposition.java`,
  sha256 `c9d8efe5cfd22b7dddfc3d7fb185bb01d1a6ed2130610c0fcfe7195ccee36a81`, matches
  `P56_SPEC.md` decision 4 exactly — verified on disk, not assumed). Only the symmetric
  branch (`tred2`/`tql2`) is ported; `orthes`/`hqr2` (the non-symmetric Hessenberg/Schur
  path) is not — every input here is a symmetric Laplacian or Gram matrix.
- **Cost**: ~9n³ (JAMA), ~5× cheaper than cyclic Jacobi's ~6n³ per sweep × 6-10 sweeps —
  matters for hashgate runtime.
- **`hypot`**: JAMA's own `Maths.hypot` is replaced with `libm::hypot` (decision 4;
  same numerically-safe two-argument hypot, but the one already on the D1 probe list).
- **Iteration cap**: 30 QL iterations per eigenvalue (JAMA's `tql2` loop has none — its
  comment says so explicitly: "Could check iteration count here"). Hitting the cap is
  not an error from `tred2`/`tql2` itself; the residual check below decides whether the
  result is usable. Ponytail: the cap can under-report an eigenpair's accuracy on an
  adversarial spectrum that has not separated after 30 iterations (JAMA/EISPACK's own
  literature default is effectively uncapped); direction is under-reporting (a still-bad
  value is returned, not flagged, until the residual gate below catches it); escape hatch
  is exactly that gate — `residual_converged` — which independently verifies `Lv = λv`
  and does not trust the solver's exit.
- **Cyclic Jacobi**: written test-only (`linalg/dense_sym/tests/jacobi.rs`), row-cyclic,
  `p < q` ascending, Rutishauser rotation `t = sgn(θ)/(|θ| + sqrt(θ²+1))`, only
  `+ − * / sqrt`. Cross-checks `tred2`/`tql2` on every closed-form fixture: eigenvalues
  agree to `1e-10` relative, eigenvectors of simple eigenvalues agree up to sign. Stops
  at `off(A)² ≤ (eps·‖A‖_F)²` or 50 sweeps (test-only; not part of the shipped solver, so
  no Ponytail marker on it — the rule is "not on exact code" but this is asserted-against
  code, not shipped, and the assertion itself states its own stopping rule).
- Entries with `|v| < eps·‖A‖_F` are flushed to exactly `0.0` after `tql2` (both
  eigenvalues and eigenvector components), `eps = 2⁻⁵²`, so the result does not depend on
  denormal handling.
- Eigenvalues ordered by a **stable** sort on `(value.total_cmp, index)` — replacing
  JAMA's own selection sort, which is not documented as stable — permuting eigenvector
  columns identically (D5).

## Tier I — iterative (`n_c > 256`): LOBPCG, ported from scipy `_lobpcg.py`

Reference: `/home/user/refs/scipy-1.16.2/lobpcg.py`, sha256
`09d3378487b4466ee97dd5c0225b7e75d0268498d32c91b1e054f69a0253b88b`, matches
`P56_SPEC.md` decision 4 exactly — verified on disk. File named `lobpcg.rs` (approved
rename from the phase text's `lanczos.rs`, decision 4 / devil ask 2).

- **Block size** `b = min(dims + 2, n_c − 1) = 4` (`dims = 2`; `n_c > 256` always makes
  `n_c − 1 ≥ 4`, so `b = 4` unconditionally on this branch). This *is* `sizeX` in the
  scipy signature — LOBPCG solves for 4 eigenpairs, and the caller keeps the smallest 2.
  Single-vector Lanczos is rejected outright (block ≥ 2 is the whole point, see R1 below).
- **Standard eigenproblem only (`B = None`)**: the reference's own call
  (`networkx_layouts.py:109-111`) never passes `B`, so this port implements only scipy's
  `B is None` branches — no secondary/mass-matrix operator, no `_b_orthonormalize`'s
  general Cholesky-of-Gram path. This is not a capability we are dropping so much as one
  the reference never exercises; recorded here as scope, not as a correctness deviation.
- **Constraint `Y = 1`** (project out the mean): with `B = None` and `Y` the constant
  column, `_applyConstraints` (`lobpcg.py:97-101`) reduces exactly to subtracting each
  column's mean — `YᵀY` is the scalar `n`, so `cho_solve` on a 1×1 factor is division by
  `n`. We implement this reduced form (`subtract mean, per column`) directly rather than
  the general Cholesky-of-`YᵀBY` machinery; the two are the same computation for this `Y`.
- **Preconditioner**: `diag(1 / max(dᵢ, 1e-9))`, `dᵢ` the component's degree sequence.
  Ponytail: the `1e-9` floor only matters for a degree-0 row, which cannot occur inside a
  connected component of size > 256 (every member has ≥ 1 neighbour); kept anyway because
  it is one line and matches the reference exactly, so no observed input exercises the
  under/over-reporting question either direction.
- **Caps and tolerances**: internal `tol = 1e-6` (`_LOBPCG_MAXITER`'s tolerance,
  reference constant, not re-tuned). `maxiter`: **corrected from the reference's `300`
  to a measured `1500`** — see "Corrections found during implementation" below; the
  reference's `300` is sized for its random start columns, and measurement showed it
  insufficient for every LOBPCG-tier fixture required here, not only large `n`, once the
  start-block deviation below replaced randomness with a deterministic sequence.
  Ponytail: on a spectrum whose gap above the requested block has not opened by
  iteration `1500` (e.g. a very large near-regular grid), the block may still be short of
  `tol` — under-reporting convergence; direction is under-reporting (a
  partially-converged block is returned unflagged by the loop itself); escape hatch is
  the same `residual_converged` + orthonormality gate used on Tier D, which marks the
  component unsolved rather than trusting the iteration count.
- **Rayleigh-Ritz**: on `span[X, W, P]` (at most `3b = 12` columns for the un-restarted
  step, `2b = 8` on restart), orthonormalised by Modified Gram-Schmidt in fixed ascending
  column order (`X` then `R` then `P`), then solved by Tier D (`eigh_dense`) on the
  resulting small dense symmetric Gram matrix. **Deviation from scipy**: scipy toggles
  between an "implicit" Gram (reusing the cached diagonal `_lambda` and identity blocks
  once residuals are small, `lobpcg.py:839-850`) and an "explicit" one recomputed from
  inner products, purely as a performance optimisation that does not change which
  vectors converge. We always compute the Gram matrix explicitly — bit-parity with
  scipy's own float trace is not a goal (only native/wasm32 bit-identity of *our* port
  is), and always-explicit is simpler to audit and to keep in gather form (D10). Recorded
  as a deviation, not invented: the surrounding algorithm (soft-locking, restart,
  Rayleigh-Ritz over `[X,R,P]`) is scipy's, only the implicit-Gram micro-optimisation is
  omitted.
  **Correction (this branch, post-first-draft)**: the small problem on `[X, R, P]` is not
  a single ordinary eigenproblem on `Xᵀ A X` — `P` is only ever MGS-orthonormalised
  within itself, never against `X`/`R` (matching scipy, which builds `gramA` *and*
  `gramB` block matrices at `lobpcg.py:~700-920` whenever `P` is present), so the basis
  is not orthonormal and the correct small problem is the **generalized** eigenproblem
  `gramA y = λ gramB y`. `linalg::lobpcg::ritz::generalized` solves this with a Cholesky
  reduction of `gramB` (`gramB = LLᵀ`, `C = L⁻¹ gramA L⁻ᵀ`, ordinary `eigh(C)`, `v = L⁻ᵀy`
  back-substituted), falling back to the ordinary solve on `[X, R]` alone (`gramB ≈ I`
  there by construction) whenever `P` is absent or the Cholesky step fails (a non-positive
  -definite `gramB`, same failure mode as scipy's own Cholesky-failure restart above).
  Missing this made every LOBPCG fixture converge to numerical garbage (near-zero
  eigenvalues on every column) — caught by the residual gate, not invented past it.
- **Soft-locking**: an `active` mask over the `b` columns, exactly as
  `activeMask &= (residualNorms > tol)` (`lobpcg.py:729-730`); a converged column stops
  contributing a residual/search direction but keeps updating through the shared
  Rayleigh-Ritz mix, ported, not invented.
- **Restart**: when Gram-Schmidt on the active `P` block hits a near-zero norm (the
  Cholesky-failure case in scipy, `lobpcg.py:802-810`), `P` is dropped for that iteration
  and Rayleigh-Ritz runs on `span[X, R]` only (`2b` columns) — ported, not invented.
  **Simplification, recorded rather than ported**: scipy's separate "jump-restart"
  (`residualNorm > 2**restartControl * smallest`, `lobpcg.py:709-728`, restarting from a
  freshly recomputed `AX`) is not implemented. It exists to recover from a residual that
  suddenly grows by six-plus orders of magnitude — a numerical-noise failure mode we have
  not observed on any fixed, deterministic start block here, closed-form or not — and
  adding it would mean inventing our own trigger tuning rather than a straight port of
  the one piece of the reference we are *not* including. If a fixture ever exhibits that
  divergence, this is the first place to look; today it is untested code we chose not to
  write, not a silently-dropped guarantee.
- **Best iterate**: as scipy's docstring states ("returns the block ... with the best
  accuracy rather than the last one"), we track the smallest residual norm seen and the
  block that achieved it, returning that block, not necessarily the final iterate.
- **Start block** (`_eig_start_vector`, `networkx_layouts.py:53-64`):
  - Column 0: `v_i = cos(i·0.9124345) + sin(i·0.3141593)`, minus its mean; if that is
    identically zero, `[-1, 1, 1, …]` (the reference's own fallback).
  - Columns 1..b−1: **corrected from the first draft's fixed-frequency cos/sin family**
    (see "Corrections found during implementation" below) to a Weyl equidistribution
    sequence, `v_i = 2·frac(i·α) − 1` for three mutually-incommensurate irrational `α`
    (`(√5−1)/2`, `√2−1`, `√3−1`), MGS-orthonormalised against column 0 and each other, in
    ascending column order.
  - **Deviation** (decision 4 / devil ask 8, approved): the reference draws columns
    1..k−1 from `rng.uniform(-1, 1, ...)` seeded by the layout seed
    (`networkx_layouts.py:103`); we use a fixed, seed-independent sequence for every
    column instead. This is strictly stronger determinism than the reference (which is
    itself seeded, just not fixed), and is the one explicitly pre-approved
    seed-independence deviation — only the *specific family* used for columns 1..b−1
    changed during implementation, not the deviation itself.
- **Rejected alternatives**:
  - *Single-vector Lanczos*, with or without full reorthogonalisation: misses
    multiplicity on exactly the inputs this phase tests (path/cycle/grid/tree, R1) —
    a block method is the only option on the allow-list that does not have this failure
    mode by construction.
  - *Block Lanczos*: no reference on disk (rule 0.6); needs its own deflation logic we
    would be inventing.
  - *Shift-invert* (`eigsh(..., sigma=-1e-3)`): needs a sparse direct factorisation; none
    is on the `libm`/`indexmap` allow-list and none is on disk (R2). Dropped per decision
    4/ask 7: the cascade is dense → LOBPCG → skip-and-report, never shift-invert.

## Fixed start and sign pinning

- The dense tier needs no start vector: it is deterministic purely from the input bits.
- **Sign pinning** (`_fix_eigenvector_signs`, `networkx_layouts.py:66-72`): for each
  column, scan `i` ascending with a *strict* `>` on `|vᵢ|`, so the lowest index wins a
  tie; if that entry is negative, flip the whole column. Applied to spectral's selected
  eigenvectors, and to Pivot MDS's *projected* `dist @ vectors` (reference `:198`, not
  the `k`-dimensional eigenvectors of `Cᵀ C` themselves).
- Ponytail: in a symmetric graph (a path, a cycle, a regular grid) two entries can tie or
  near-tie on `|v_i|`; the ascending-index rule is bit-reproducible under bit-identity,
  but the *chosen* sign is an artifact of solver internals — any change to the tridiagonal
  reduction, the QL shifts, or LOBPCG's iteration path can flip which entry is largest
  and so flip the sign, even though the eigenvector is otherwise unchanged (both signs are
  equally valid solutions). Direction: cosmetic (mirrors the layout, does not corrupt it).
  Escape hatch: none needed for correctness; a future canonicalisation against fixed probe
  vectors (decision 4, "beyond the reference", left off by default) would remove the
  residual non-uniqueness if ever required.

## Residual and orthonormality

- `_eig_converged` (`networkx_layouts.py:74-80`) ported unchanged: every value finite,
  and `max_j ‖Lvⱼ − λⱼvⱼ‖ ≤ 1e-2 · max(max|λ|, 1e-12)`, `_EIG_RESIDUAL_TOL = 1e-2`. Run on
  **both** tiers — the reference trusts `eigh` for dense and only checks the iterative
  tier; we check dense too, because it is cheap and it is the gate that catches a
  30-iteration-cap failure.
- Additionally require `‖VᵀV − I‖_max ≤ 1e-6` (decision 4 / C5) on both tiers.
- Per-fixture residuals are measured, not asserted a priori, and recorded in
  `docs/measurements/phase06-eigen.md`.

## Per component

- `n_c = 1`: no solve; the node's coordinate stays `(0, 0)` before packing, so it sits
  exactly at its lattice cell after `pack_components`.
- `n_c = 2`: `dims_eff = min(dims, n_c − 1) = 1`; the single eigenvector is copied into
  both output columns (`networkx_layouts.py:158-159`'s general
  `coords[idx, vectors.shape[1]:] = vectors[:, -1:]`, applied for any component that
  converges fewer columns than `dims` — not special-cased to `n_c=2` only).
- Otherwise: dense when `n_c ≤ 256`, else LOBPCG.
- A component whose solve fails the residual/orthonormality gate is **skipped**: its
  coordinate stays `(0, 0)` (collapses to a point at its packing cell), and its failure —
  tier, `n_c`, measured residual — is returned in a diagnostic value alongside the
  geometry (`SpectralReport`/`MdsReport` in `spectral.rs`/`pivot_mds.rs`). This is
  graph-core's half of C12; wiring it through `graph-cli` so it prints is the
  integration step's job (this branch does not touch `graph-cli`, per its CREATE list).
- **No silent random fallback (C12)**: if *no* component in the whole graph solves, the
  functions return `Err(SpectralError::NothingSolved)` /
  `Err(MdsError::NothingSolved)` rather than the reference's `_random_layout(...)`. The
  reference's own `n < 4` guard (`_spectral_layout_3d`, `:257-258`) is not ported either,
  for the same reason: fewer than 4 nodes is not "no algorithm can help", it is "the
  algorithm returns fewer columns than requested", which the `n_c` dispatch above already
  handles without a randomness fallback of any kind.
- **2D packing** (adapted from the reference's 3D cube lattice, `_pack_component_blocks`,
  `:218-236`, per decision 4's "component packing adapted to 2D as the devil describes"):
  lattice side `= max(1, ⌈√(num_components)⌉)` (cube root → square root), per-component
  scale `= √(n_c / biggest)` (cube root → square root), cell `= (slot % side, slot /
  side)` (drop the third cube coordinate), spacing `_COMPONENT_SPACING = 2.5` unchanged.
  Component placement order: stable sort on `(-size, min_index)`.

## Corrections found during implementation (this branch, post-first-draft)

Required tests (closed-form spectra, `n ≤ 256` and `n > 256`, `C_300` as a circle) were
run against the first-draft LOBPCG port and failed (observed RED, not assumed). Three
bugs, found in this order, each fixed and re-verified before moving to the next:

1. **Missing generalized Rayleigh-Ritz.** Every LOBPCG-tier fixture converged to
   near-zero garbage on every column. Root cause: `[X, R, P]`'s Rayleigh-Ritz was solved
   as an ordinary eigenproblem on `Xᵀ A X`, wrongly assuming the basis was orthonormal —
   `P` is not, by construction (see the Rayleigh-Ritz bullet's correction above). Fixed
   by `linalg::lobpcg::ritz::generalized`, validated in isolation against a hand-solved
   2×2 generalized eigenproblem before trusting it inside the full loop
   (`linalg/lobpcg/ritz/generalized/tests.rs`).
2. **Start-vector aliasing.** With (1) fixed, LOBPCG converged bit-stably to a *real but
   wrong* eigenvalue (path `n=300`: `≈0.1033`, mode `k≈31`, not the smallest nontrivial
   mode `k=1`, `≈0.00011`). Root cause: the first draft's "approved deviation" (above)
   replaced *every* start column with a fixed absolute-radian cos/sin frequency; a fixed
   radian frequency lands on a specific path/cycle eigenmode (`index ≈ frequency·n/π`,
   growing with `n`), so the start block was never broadband. Re-reading the reference's
   actual `_eig_start_vector` (not the first draft's paraphrase of it) confirmed only
   column 0 is a fixed formula in the reference too — columns 1..k−1 are genuinely random
   there. Fixed by keeping column 0 exactly as before and replacing columns 1..b−1 with
   the Weyl sequence in the Start block bullet above: deterministic (Weyl's theorem:
   `i·α mod 1` equidistributes for irrational `α`) but broadband at every `n`, restoring
   the coverage the reference's random columns provide without reintroducing a seed.
3. **`maxiter` too low.** With (1) and (2) fixed, `maxiter = 300` (the reference's own
   constant) failed even the *smallest* required LOBPCG fixture — a `257`-node path just
   above `_DENSE_EIG_LIMIT` needed `410` iterations, not merely the largest fixtures.
   Root cause: the Weyl start block, while broadband and unbiased, still converges
   somewhat slower than genuinely random vectors for this port's block-4,
   constant-diagonal-preconditioner setup on a path/cycle's `O(1/n²)` eigenvalue gap.
   Measured worst case among required fixtures: a `400`-node cycle needed `545`
   iterations. `maxiter = 1500` was chosen for headroom above that measured worst case,
   not guessed; a `4096`-node path was also measured and genuinely fails to converge even
   at a `5000`-iteration cap — an honest scaling limit of the simplified port, disclosed
   here and in `docs/measurements/phase06-eigen.md` rather than asserted past.

None of these three change the ADR's *decisions* (block size, `B = None` scope, the `Y =
1` constraint reduction, soft-locking, the restart rule, best-iterate tracking, sign
pinning, the residual/orthonormality gate, C12, the 2D packing) — only the start-block
*family* for columns 1..b−1, the small-problem solver's *generality* (ordinary → its own
necessary generalized form, both scipy's algorithm), and one *constant* (`maxiter`), all
found by running the tests the ADR already committed to and re-reading the reference text
the ADR already cites, per guardrail 3.

## Is native/wasm32 bit-identity achievable? Yes, by the same argument as the verdict

Both tiers use only `+ − * / sqrt` plus `libm::{sin, cos, hypot}` — already on, or added
to, the D1 probe list. With every reduction sequential (dot products, norms, means) and
no `f32/f64::mul_add`, `powi`, or std transcendental anywhere in `linalg/` or the two
layout files, the computation is a deterministic function of the input bits on both
targets. No per-platform exception is registered in advance (C16); the wasm32 arm of the
4-way hashgate is not run by this branch's own gate (hashgate.rs is out of the CREATE
list), so bit-identity here is an **engineering property established by construction and
the grep gate**, not yet a measured 4-way hash — that measurement is the integration
step's, once hashgate.rs is extended (devil ask 2).

## What could still break it (unchanged from the verdict, restated for this file)

`std` transcendentals anywhere (including `hypot`/`cbrt`), `mul_add`/`powi`/`algebraic_*`,
`f64::min`/`max` on `±0`, a reduction not in a fixed sequential/chunked order, a tie
broken by an unstable sort or bare `argmax`, any `NaN` reaching a comparison, denormal
(FTZ/DAZ) dependence, `cfg(target_arch)`/pointer-width branches in numeric code, an
uncapped loop, and — specific to this branch — the gate never exercising Tier I on
wasm32 at all (its per-tier hit count would then read `UNKNOWN`, not a pass, per C15).

A `libm` version bump changes both targets' bits together; that moves the baseline, it is
not a divergence.
