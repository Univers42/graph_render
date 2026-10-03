# Job rv-sg-igraph-clean (agent build, review only, docs)

Review the sg-igraph-clean branch before it is merged (job `merge-igraph-clean` lands it). This
worktree is that branch; its delta is `git diff origin/develop...HEAD` (or against the merge-base
when develop has moved). Brief it answered: `prompts/jobs/sg-igraph-clean.md`. Spec:
`docs/layouts/layout.force.fruchterman_reingold.md`, `docs/layouts/layout.force.kamada_kawai.md`.

Rules, checks and output shape: as `prompts/jobs/review-core-post.md` says (read it first), with
`docs/reviews/rv-sg-igraph-clean.md` as the report and the only path you write. You review against
the spec docs; you never open `/goinfre/dlesieur/refs/igraph-0.11.9`, any igraph source, or branch
`sg-igraph-dims`. Check in particular:

1. Clean room: `docs/measurements/sg-igraph-dims.md` has `## Provenance` listing every source read
   and the sentence the brief requires; every comment in the new modules cites the spec or a paper,
   never an igraph file or line.
2. `fruchterman_reingold_3d.rs`, `kamada_kawai_3d.rs`, `fr_kernel.rs`, `kk_kernel.rs` against the
   spec step by step: defaults (FR `niter = 500`, KK `maxiter = 50 n`), the start (the spec's
   sphere table: poles at rows 0 and n-1), temperature schedule, the Newton step. A step the spec
   does not state and the code performs anyway is a FIX (it is a guess or a breach).
3. The 2-D ids are byte-identical: run `hashgate --seeds 8` here and on `origin/develop`, paste the
   `layout.force.fruchterman_reingold` and `layout.force.kamada_kawai` per-stage lines of both.
   Also the `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero).
4. Determinism §6: `libm` for every transcendental, no FMA, fixed-order sums, no `HashMap`.
5. `scripts/scigraphs-conformance.sh`: paste the `IGRAPH_FR` and `IGRAPH_KK` lines; any other row that
   moved is a FIX. The `gate-19` finite-output test names the input property it relies on.
6. `capabilities --check` (no new problem), `codegen --check` (exit 0); house limits (40-line
   functions, 300-line files, 4 parameters); a `Ponytail:` on every heuristic.

Known and not a finding: `merge-p12-t4b` (now on develop) registers the same two layouts as
`layout.force.{fruchterman_reingold,kamada_kawai}.3d`; job `merge-igraph-clean` resolves the
duplicate after this review. Note any fact that job will need.
