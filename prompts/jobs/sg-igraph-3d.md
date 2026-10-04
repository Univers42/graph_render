# Job sg-igraph-3d (agent build, branch sg-igraph-3d, worktree ~/goinfre/wt/sg-igraph-3d)

CLEAN ROOM, hard rule: never open, read, grep or list `/goinfre/dlesieur/refs/igraph-0.11.9`,
`~/goinfre/refs/igraph*`, or any path matching `sg-igraph-dims*` (on any branch). Use git only
through the paths named below.

Context: branch `sg-igraph-clean` (06ad8ea1, base 02d5c2ad) re-derived FR/KK 3-D kernels that
develop already registers as `layout.force.fruchterman_reingold.3d` and
`layout.force.kamada_kawai.3d` (`crates/graph-core/src/registry/arms_3d.rs`). The assessment is
`git show origin/assess-3d:docs/measurements/assess-3d-branches.md` (read "Branch 2", "Overlaps"
and "Recommendation"). Decision taken: **Option A, develop's ids and kernels win.** Port only the
oracle half; never `git merge`/`cherry-pick` the branch whole. Read it with
`git show 06ad8ea1:<path>` and `git diff 02d5c2ad 06ad8ea1 -- <path>`.

Exact tasks:
1. Port `crates/graph-cli/src/oracle_python/conformance/motor/fit.rs` (+ `fit/tests.rs`) and wire
   it into `conformance/{motor,rows}.rs`, keyed to develop's dotted ids.
2. Port into `harness/oracle-igraph.py` the `dim=3` `REFERENCES` entries and
   `start_layout(initial, dim)`, re-keyed to develop's dotted ids; and the `_3d` rows / `columns_3d`
   of `crates/graph-cli/src/oracle_python/igraph.rs`, re-keyed the same way.
3. Port the `conformance/gaps.rs` changes: the corrected `G_IGRAPH_SEED` note, `G_IGRAPH_FIT`,
   `G_KK_NON_FINITE`, `G_DRL_NO_3D`.
4. Settle the contradiction by measurement: develop's `harness/oracle-igraph.py:32` says
   `layout_kamada_kawai(dim=3)` refuses ("Invalid start position matrix size"); the branch passes a
   3-column start. Run the 3-D KK and FR references in `ge-python-oracle` (through
   `scripts/orch/drun`; find the existing invocation with `git grep -n oracle-igraph -- scripts crates`)
   and record which claim holds, with the command and output. Fix the wrong comment.
5. Run `scripts/scigraphs-conformance.sh` and the igraph differential; paste each 3-D row's
   measured deviation and its ceiling.
6. Docs: bring the append-only supersets with `git checkout 06ad8ea1 -- docs/decisions/layouts-igraph.md
   docs/layouts/layout.force.fruchterman_reingold.md docs/layouts/layout.force.kamada_kawai.md`, then
   re-key any `_3d` id in them to develop's; update `docs/measurements/scigraphs-conformance.md`
   from the branch's version and your step-5 numbers.
7. Do NOT port: `layout/force/fr_kernel.rs`, `kk_kernel*`, `{springs,gradient,step}.rs`,
   `fruchterman_reingold_3d.rs`, `kamada_kawai_3d.rs`, their tests, the FR/KK rewrites, the two `_3d`
   registry entries. No `LAYOUTS` change at all.

Paths allowed: `crates/graph-cli/**`, `harness/oracle-igraph.py`, `docs/decisions/layouts-igraph.md`,
`docs/layouts/layout.force.{fruchterman_reingold,kamada_kawai}.md`,
`docs/measurements/scigraphs-conformance.md`, `prompts/jobs/sg-igraph-3d.md`. Not allowed:
`crates/graph-core/**` (except a test-only fix you justify), `crates/graph-wasm/**`, `packages/**`,
`app/**`, `server/**`, `deploy/**`.

Done when, each with its command and exit pasted: `scripts/orch/gate.sh target/gate-sg-igraph-3d
scripts/orch/rows/quick.rows` all PASS (it includes `scigraphs-conformance` and its negative control);
the step-4 measurement is recorded in `docs/measurements/scigraphs-conformance.md`.
