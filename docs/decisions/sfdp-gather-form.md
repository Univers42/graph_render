# The sfdp spring-electrical iteration is in gather form, and the prolongation is not

**Status:** decided. 2026-10-04, job `sg-sfdp-collapse` (defect f).
**Subject:** `crates/graph-core/src/layout/graphviz/sfdp/{solve,prolongation,quadtree}.rs`.

## The decision

Two halves, and they go opposite ways:

| kernel | reference | this port | D10 |
|---|---|---|---|
| `spring_electrical_embedding`'s vertex loop | sequential — normalise and move `i`, then compute `i+1`'s force from the moved positions | **gather** — every node's force from the positions at the start of the iteration, then every node moved | satisfied |
| `interpolate_coord` in `prolongate` | Gauss-Seidel — writes `x[i]` in the loop that reads `x[j]` | **Gauss-Seidel, unchanged** | **deviation, recorded here** |

## Why the vertex loop is gather

Before this repair the loop updated `self.x[i]` in place while later nodes read it — the
reference's order — and the module doc claimed the opposite:

> **Gather form (D10).** Node `i`'s displacement is computed entirely from the positions at the
> *start* of the iteration, so the result does not depend on the order nodes are updated in.

That was a false claim about the code, and a false claim about the code is the defect: D10 exists
so that a reader can trust that a node's value depends on its inputs and not on visit order. A
doc that asserts gather over a sequential loop is worse than no doc, because it invites the next
reader to reorder the loop and get different bytes.

So the repair made the **code** match the **doc** rather than the reverse, and this record says
what that costs.

**The cost, measured.** Graphviz output is the oracle for this stage (user, 2026-09-30;
`docs/decisions/graphviz-oracle.md`). A gather vertex loop and the reference's sequential one are
different functions of the same inputs. Over the p13 differential's 1000 gate seeds the worst
coordinate gap moved **3.881e+02 → 3.887e+02 points** (`docs/measurements/p13-gv2-sfdp.md`), i.e.
it did not move materially — and it could not, because the only term that sees a moved position
is the attraction `CRK·(x_i − x_j)·‖x_i − x_j‖`, whose contribution to a unit-length total force
is second order.

**Why D10 wins anyway.** The three reasons, in order:

1. The row this port is measured on (`GRAPHVIZ_SFDP`, SciGraphs conformance) is at the `shape`
   tier with cause `algorithm`. Its residual is the coarsening permutation stream, not the
   vertex-loop order, so gather costs nothing measurable there either.
2. D10 is a house rule for **every** kernel in this repository, not a per-layout preference. A
   stage that opts out needs a reason that is not "the oracle would rather". This one has one
   (`sfdp-gather-form` is the escape hatch: `run_seeded` plus `Solve`'s own step order), and the
   rule exists precisely so that a future reader does not have to re-derive it.
3. The sequential form was, in this tree, never pinned by a test. `two_relaxations_are_bit_identical`
   passes for both forms, and `edge_direction_does_not_change_the_drawing` passes for both. A
   behaviour with no test that detects its removal is not worth a deviation record.

**What would reverse it.** A port that drew glibc's `gv_permutation` stream for its coarsening
would be strictly closer to the oracle if it were also sequential. That is a real engineering
argument and it is deferred, not dismissed: it belongs to `sg-sfdp-step`, which owns coarsening
order, and the reversal is one loop in [`solve::Solve::relax`].

## Why `interpolate_coord` stays sequential

`interpolate_coord` (`spring_electrical.c:814-835`) is not a force kernel; it is a smoothing pass
over the prolonged positions, and it is Gauss-Seidel in the reference:

```c
for (size_t i = 0; i < A->m; i++){ ...
    for (j = ia[i]; j < ia[i+1]; j++){ ... y[k] += x[ja[j]*dim + k]; }
    if (nz > 0){ beta = (1-alpha)/nz; x[i*dim+k] = alpha*x[i*dim+k] + beta*y[k]; }
}
```

`x` is both the source and the destination. Collecting the sums first (Jacobi) is a **different
algorithm**, not a reordering of the same one: on a three-node path it gives node 1 `15.0` where
Gauss-Seidel gives `11.25`, and node 2 `17.5` against `15.625`. That is a different drawing, not a
different rounding.

There is no gather form of this pass that preserves its result, so the deviation is recorded
rather than removed. `docs/decisions/sfdp-gather-form.md` is that record; this paragraph is the
cross-reference from the code, and `prolongation.rs`'s own doc says the same.

**Determinism is unaffected.** Gauss-Seidel over CSR rows in dense index order is a fixed
sequential order, so the pass is reproducible bit for bit native and wasm32; what D10 protects —
one node's value depending on its inputs rather than on visit order — is genuinely given up, and
it is given up only where the reference gives it up.

## What is checked

| property | where |
|---|---|
| the iteration really is gather: forces are computed before any node moves | `solve.rs::gather` / `solve.rs::advance`, and `solve.rs`'s two bit-identity tests |
| a node's force does not depend on its own position having been moved | `quadtree.rs::repulsion_is_a_pure_function_of_the_positions` |
| the whole drawing is reproducible and seed-moving | `tests.rs::run_twice_is_bit_identical_and_the_seed_moves_it` |
| `interpolate_coord` is Gauss-Seidel and not Jacobi | `prolongation.rs::interpolation_is_in_row_order_not_jacobi`, `contract.rs::prolongation_pulls_a_node_toward_its_neighbours_mean` |