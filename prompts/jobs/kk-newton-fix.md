# Job kk-newton-fix (agent build; correct Kamada-Kawai's 2D Newton step, then re-measure)

Why: p12-t4b found `crates/graph-core/src/layout/force/kamada_kawai/solve.rs:47` HIGH: `solve_2` reads
`a=Hyy, c=Hxx` and returns `[c·g0 − b·g1, …]`, so the x component is built from `Hxx` where the inverse
of the 2×2 Hessian needs `Hyy`. The 3×3 path p12-t4b added is the correct adjugate. The transposed form is
pinned by the test `the_2d_step_is_pinned_as_transposed_against_the_true_inverse`, on purpose, so
p12-t4b moved no 2D byte. Orchestrator decision 2026-10-02: fix it now, before more ids land on it.
Related symptom: `docs/measurements/scigraphs-conformance.md` row 8 (`IGRAPH_KK`): "different shape:
grey is a blob, green is a near-straight line".

Do:
1. TDD: replace the pinning test with one that fails today: for a set of 2×2 symmetric positive-definite
   Hessians (and one near-singular one), `H · solve_2(H, g) == g` within a stated tolerance. Then fix
   `solve_2` to the true inverse. Same shape as the 3×3 path; no other change to KK.
2. Every 2D KK hash moves. Find each pinned byte or digest that names `layout.force.kamada_kawai` (2D)
   with `git grep`, regenerate it with the command its own test or doc names, and list every regenerated
   file in the return block. Do not touch a pin that does not involve KK: if one moves, the fix is wrong.
3. Re-measure, and record before/after in `docs/measurements/kk-newton-fix.md`:
   - the igraph differential (`docs/measurements/p12-igraph-ceilings.md` lines 6-10: `emit-igraph-fixtures
     --seeds 100`, `harness/oracle-igraph.py` in `ge-python-oracle`, `graph-cli oracle-igraph`); KK's
     ceiling today is worst 1.348 against reference 1.029. If the new worst is lower, propose the new
     ceiling with its margin; do not loosen a ceiling.
   - `scripts/scigraphs-conformance.sh` then `graph-cli scigraphs-conformance`, row 8. If the baseline
     (`crates/graph-cli/src/oracle_python/conformance/baseline/table.rs`) changes, change only KK's row.

Out of bounds: other layouts, the 3D path, `graph-contract`. Adding a dependency is a stop.

Done when: quick.rows green (hashgate-8 and its negctl included), the new test is RED on the old
`solve_2` and GREEN on the new one (paste both runs), both differentials re-run with their real exit codes,
and the measurement file committed.
