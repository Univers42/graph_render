# Phase 6 — eigensolver residuals, LOBPCG iterations and timings

Measured in the Docker toolchain (`/home/user/gr`, debug build, x86_64 host) against
`crates/graph-core/src/layout/spectral.rs`'s per-component solve
(`docs/decisions/eigensolver.md`). Every number below came from an actual test run this
session; none is estimated. The dense tier is `tred2`/`tql2` (JAMA); the LOBPCG tier is
`linalg::lobpcg::lobpcg_smallest` with block `b = 4`.

Residuals are the worst column's `‖Lv − λv‖` (unscaled, `f64`), captured by a temporary
scratch test in `layout/spectral/tests.rs` (`zz_scratch_measurement_dump`, added, run
once with `--nocapture`, then removed — the committed suite already checks these same
fixtures against the closed-form spectrum in `check_spectrum`; the scratch test only
exposed the residual and timing numbers for this report). To reproduce, re-add a test
that calls `solve_component` on each fixture below and prints
`residual_converged`'s internal worst-column residual.

## Per-fixture residual, tier and timing

| fixture | n | tier | iterations | worst residual | wall time |
|---|---|---|---|---|---|
| path | 100 | dense | — | 2.332e-15 | 60.9 ms |
| path | 256 | dense | — | 4.069e-15 | 993.2 ms |
| path | 300 | LOBPCG | 353 | 3.324e-7 | 680.7 ms |
| cycle | 100 | dense | — | 1.256e-14 | 58.1 ms |
| cycle | 256 | dense | — | 1.562e-14 | 958.5 ms |
| cycle | 300 | LOBPCG | 486 | 1.092e-6 | 784.6 ms |
| star | 100 | dense | — | 3.098e-15 | 24.0 ms |
| star | 256 | dense | — | 7.621e-14 | 407.0 ms |
| star | 300 | LOBPCG | 1 | 7.380e-15 | 1.0 ms |
| complete (`K_n`) | 50 | dense | — | 2.078e-13 | 3.4 ms |
| complete (`K_n`) | 300 | LOBPCG | 0 | 3.281e-13 | 8.0 ms |
| grid | 16×16 (256) | dense | — | 1.560e-14 | 826.9 ms |
| grid | 20×20 (400) | LOBPCG | 151 | 4.438e-7 | 265.6 ms |

All residuals pass the `1e-2 · max(max\|λ\|, 1e-12)` gate
(`_eig_converged`/`residual_converged`) by four to nine orders of magnitude — the LOBPCG
tier's `1e-6`—`1e-7` residuals are close to its own internal `tol = 1e-6`, exactly as
expected from an iterative method that stops once it clears that tolerance; the dense
tier's `1e-13`—`1e-15` residuals reflect direct (non-iterative) `tred2`/`tql2`, limited
only by floating-point rounding.

`star` and `complete` reach LOBPCG's block trivially (`0`–`1` iterations): both have a
massively degenerate nontrivial eigenspace at the smallest end (`star`: eigenvalue `1`
with multiplicity `n − 2`; `complete`: eigenvalue `n` with multiplicity `n − 1`), so the
very first Rayleigh-Ritz pass already lands inside the residual gate. `path`, `cycle` and
`grid` have a simple, tightly-clustered spectrum near zero (`path`/`cycle`'s gap is
`O(1/n²)`) and need genuine iteration.

Dense-tier wall time is dominated by `tred2`/`tql2`'s `O(n³)` cost, not by the residual
check; note `path_256`/`grid_16x16` (`n = 256`, the largest dense-tier fixture on this
branch) both cost roughly 1 second in this debug build — an unoptimized-build number, not
a claim about release performance.

## LOBPCG iteration count and timing scaling (path graph, `docs/measurements` requirement)

From `linalg::lobpcg::tests::measurement_lobpcg_iterations_and_timing`
(`--nocapture`), a path graph swept past `_DENSE_EIG_LIMIT = 256`:

| n | iterations | strict-converged (`tol=1e-6`) | wall time |
|---|---|---|---|
| 300 | 353 | true | 645.6 ms |
| 601 | 714 | true | 1992.2 ms |
| 1000 | 962 | false (loose gate still passes) | 5474.6 ms |

Additional path/cycle iteration counts measured while diagnosing the `maxiter` correction
below (`docs/decisions/eigensolver.md`, "Corrections found during implementation"):

| fixture | n | iterations |
|---|---|---|
| path | 257 (smallest LOBPCG-tier fixture) | 410 |
| path | 400 | 525 |
| cycle | 300 | 486 |
| cycle | 400 | 545 (worst among all fixtures measured) |
| path | 4096 | does not converge even at a 5000-iteration cap |

`n = 1000`'s strict flag (`out.converged`, the reference's own unscaled `tol = 1e-6`) is
`false` — the block has not tightened to `1e-6` in 962 iterations — but the loosely-scaled
caller gate (`residual_converged` at `1e-2`, `orthonormal` at `1e-4`) still accepts it; the
strict flag is diagnostic only (`LobpcgOutcome::converged`), never the acceptance
criterion (`layout::spectral`'s `converged` function, `1e-2`/`1e-6` on the actual
component residual). `n = 4096` fails outright even under the loose gate: a genuine,
disclosed scaling limit of this simplified port (deterministic broadband start
vectors converging somewhat slower than the reference's true-random ones, plus a weak
constant-diagonal preconditioner — see the ADR), not a bug, and not asserted past in the
committed suite.

## `C_300` circle layout

`layout::spectral::tests::end_to_end::c_300_lays_out_as_a_circle_equal_radii_uniform_angles`
(committed test, not scratch) checks the required property directly: a 300-node cycle's
two LOBPCG-tier eigenvectors (residual `1.092e-6`, table above) lay every node at equal
radius from the origin with uniformly-spaced angles. This test passes.

## Determinism

`linalg::lobpcg::tests::is_deterministic_run_twice` (path, `n=400`) and
`layout::spectral::tests::end_to_end::is_deterministic_run_twice_across_both_tiers` both
assert bit-identical (`f64` equality, not tolerance) output across two calls with the
same input bits, on the dense and LOBPCG tiers each. Both passed in every full-suite run
this session, including the two consecutive `cargo test -p graph-core --lib` runs done
back to back as the branch-local gate's determinism check.
