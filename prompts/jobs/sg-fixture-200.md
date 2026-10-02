# Job sg-fixture-200 (agent build, SciGraphs conformance: every row)

Read `prompts/jobs/sg-common.md` first. Runs LAST, after every other `sg-*` job has landed.

Why: the conformance fixtures total 340 nodes; the largest is lesmis (77). Several kernels
change path above a size (SciGraphs' spectral switches from dense `eigh` to sparse above 256
nodes; sfdp's `quadtree` and its level count grow with n). A fixture of about 200 nodes
exercises the multilevel and Barnes-Hut paths that 77 nodes barely reach.

Do:
1. Pick a public graph of 150-250 nodes already on disk (`$GM_SCRATCH/refs`, the networkx
   bundled datasets in `ge-python-oracle`, or SciGraphs' own examples); name its source and
   licence. A missing one is a stop. Do not generate a random graph: a fixture is data.
2. Add it through the existing fixture path (`conformance/fixtures.rs`, `fixtures/`), the way
   the 24 current fixtures are emitted; both arms load the same file.
3. Run the gate; every row moves. Re-pin ALL rows from
   `conformance-baseline-proposed.rs` in one commit; paste the old/new disparity table.
   A row whose tier gets worse on the new fixture is a finding: write it in the doc with the
   first divergent node, do not chase it in this job.
4. `--break` must exit 1.

Paths: `fixtures/`, `conformance/fixtures.rs`, `baseline/table.rs` (all rows),
`docs/measurements/scigraphs-conformance.md` (matrix), `docs/measurements/sg-fixture-200.md`.

Done when: the full old/new table is pasted, each worsened row is named with its first
divergent node, and the sg-common done-when holds.
