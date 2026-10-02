# Job sg-igraph-clean (agent build, clean-room: igraph's 3-D FR and KK from the spec alone)

Read `prompts/jobs/sg-common.md` first. Rows: `IGRAPH_FR`, `IGRAPH_KK`.

Why. Job `sg-igraph-dims` wrote the 3-D variants of Fruchterman-Reingold and Kamada-Kawai, but the
same agent also opened igraph's C sources (`kamada_kawai.c`, `fruchterman_reingold.c`,
`circular.c`). `docs/decisions/layouts-igraph.md` rule 1 lets a spec author read the C and lets the
implementer read only the spec and the papers. One agent in both roles voids the clean room, so its
graph-core code was discarded. Its spec text, harness pass, conformance fit and registry metadata
were kept; they are on this branch already (one squashed commit over develop).

## The clean room (a breach discards the work again)

- Never open anything under `/goinfre/dlesieur/refs/igraph-0.11.9` (C sources, tests, `.out`
  files, the tarball) or any other igraph or python-igraph source.
- Never read branch `sg-igraph-dims` or `origin/sg-igraph-dims`: no `git show`, `git diff`,
  `git log -p` or checkout of it, no `git log --all -p`.
- You may read: `docs/layouts/layout.force.fruchterman_reingold.md`,
  `docs/layouts/layout.force.kamada_kawai.md`, the papers they cite (see `prompts/REFERENCES.md`
  for what is on disk), networkx 3.6 `drawing/layout.py` (BSD, lines 452 and 876), and every file
  in this repo except the branch above. The 2-D ports on this branch
  (`layout/force/fruchterman_reingold.rs`, `layout/force/kamada_kawai.rs`) are clean; build on them.
- The spec is silent on a step you need: stop. End your report with `blocked: spec gap <spec file>
  <step>` and what is missing. Do not fill the hole by guessing what igraph does.

## Do

1. The tree does not build: `layout/force/mod.rs` and `registry.rs` name
   `fruchterman_reingold_3d::FruchtermanReingold3D` and `kamada_kawai_3d::KamadaKawai3D`, and
   `registry/igraph.rs` holds their `Metadata`. Write both modules: ids
   `layout.force.fruchterman_reingold_3d` and `layout.force.kamada_kawai_3d`, defaults as the
   specs state them (FR `niter = 500`, no grid, random start per axis; KK sphere start from the
   section "The 3D start: the sphere", `maxiter = 50 n`). Share the kernel across dimensions with
   a `const D` the way `layout/force/spring/forces.rs` does.
2. The 2-D ids stay byte-identical. Before you touch a 2-D file, run `hashgate --seeds 8` on the
   untouched branch (after the modules exist, or on develop) and keep the per-stage hashes of
   `layout.force.fruchterman_reingold` and `layout.force.kamada_kawai`; paste them before and after.
3. Your own tests (no test from another branch): the sphere start matches the spec table (poles at
   rows 0 and n-1, interior `z`, `r`, `phi`); 3-D output has a non-zero z spread; same seed, same
   bytes; `kamada_kawai_3d` gives finite output on the `gate-19` fixture where igraph returns 9
   non-finite coordinates (`docs/measurements/scigraphs-conformance.md`, the IGRAPH_KK row), and
   the test names the input property that breaks igraph.
4. `docs/layouts/layout.force.fruchterman_reingold.md`, the paragraph "Resolved for this tree":
   its last sentence names `kernel.rs`'s `repel`, which no longer exists. Point it at your code.
5. Re-measure, since the carried pins came from the discarded code: run the conformance script,
   re-pin `IGRAPH_FR` and `IGRAPH_KK` in `baseline/table/igraph.rs` and the matrix, and update the
   stress figures in `crates/graph-cli/src/oracle_python/tests.rs` (the two doc comments that cite
   `sg-igraph-dims.md`) from a run of `harness/oracle-igraph.py` on this tree. Run the reference
   twice and paste the byte diff of the two runs (expected: none).
6. Write `docs/measurements/sg-igraph-dims.md` (four files cite it): the measurements of steps 2-5
   and a section `## Provenance` listing every source you read for the algorithm, and the sentence
   "The implementer did not open igraph's sources or branch sg-igraph-dims."

Paths: `crates/graph-core/src/layout/force/**`, `crates/graph-core/src/registry/igraph.rs` (only
if a `Metadata` field is now untrue), `crates/graph-cli/src/oracle_python/**`,
`docs/layouts/layout.force.fruchterman_reingold.md` (step 4 only),
`docs/measurements/{sg-igraph-dims,scigraphs-conformance}.md`.

Done when: the sg-common done-when; `capabilities --check` and `codegen --check` exit 0; the 2-D
hashes are equal before and after; the provenance section is written.
