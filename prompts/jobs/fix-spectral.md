# Job fix-spectral (agent build: review-layout-force LF-09, LF-10, LF-11, LF-25)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-force.md` (ids `LF-NN`,
verified on `74f8994`; merge-p12-t4b has since touched `spectral.rs`, so confirm each defect first).
Reference: SciGraphs (`SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py`, pin in
`.gitmodules`) and scipy/networkx under `/home/dlesieur/goinfre/refs` (read tool;
`scripts/orch/gr` does not mount them). These rows are `SPECTRAL_3D`/`MDS_3D` = `algorithm` in
`docs/measurements/scigraphs-conformance.md`; a fix that moves them is reported, never re-pinned.

1. **LF-09, MAJOR.** `spectral.rs` has one LOBPCG tier; the reference retries with
   `eigsh(sigma=-1e-3)` (`networkx_layouts.py:120-127`) and runs `maxiter=300, tol=1e-6` (`:111`).
   RED: a 30×10 grid whose component misses the residual gate today. GREEN: the shift-invert tier
   with the existing `crate::linalg` (no new dependency: a stop), `maxiter`/`tol` aligned. A
   component that still misses the gate is reported, never silently left at the origin.
2. **LF-10, MAJOR.** `spectral_stage.rs` drops the `ComponentReport`s. GREEN: a component that
   misses the gate becomes a `StageError` naming it and its peak residual (or another surfaced
   channel the stage API already has; adding a public type is a stop).
3. **LF-11, MAJOR, determinism.** `pivot_mds.rs` `top_eigenpairs` copies `eigh`'s basis for a tied
   non-zero eigenvalue, so the orientation is the solver's. RED: `C4` and `C8` 2-D embeddings pinned
   against a hand-computed orientation (and a native-only check that the output is invariant under
   a rotation of the tied basis). GREEN: canonicalise each tied group before projection, by a rule
   written in the module doc. Paste `hashgate --seeds 8` and its negative control.
4. **LF-25, MINOR.** `registry/spectral.rs` states 700 as the node ceiling while the same row
   measures 100 000-node grids converging. Separate the per-component spectral-gap threshold from
   the node ceiling; say what each bounds. `capabilities --check` must stay as it is for these rows.
5. **LF-26 (pivot part).** `pivot_mds/tests.rs` `is_deterministic_run_twice` is
   `run(&t) == run(&t)`; replace it with the LF-11 pin.

Paths: `crates/graph-core/src/layout/{spectral.rs,spectral/**,spectral_stage.rs,pivot_mds.rs,pivot_mds/**}`,
`crates/graph-core/src/registry/spectral.rs`, `docs/layouts/layout.spectral*.md` and
`docs/layouts/layout.*mds*.md` (doc fixes only), `docs/measurements/fix-spectral.md`.

Done when: fix-common's done-when; every id above has a row; `scripts/scigraphs-conformance.sh`
exits 0, or a moved row is listed under "decisions needed" with before/after.
