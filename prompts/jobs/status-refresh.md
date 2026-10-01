# Job status-refresh (agent build, docs only)

Why: `docs/reports/STATUS.md`, `prompts/CONTINUE.md` and the "new host" block of `prompts/RESUME.md` were last
written 2026-09-30. Since then develop gained (all landed 2026-10-01): p12-t2, p12-t3 (the five 3D SciGraphs
layouts, LAYOUTS = 34), p13-3d and its seam (a z column), the Graphviz ports twopi, circo, patchwork, osage,
neato, fdp, the osage differential gate, graphviz-verdict (the ledger reads any oracle record by name), the
wasm circo trap fix, studio-live, studio-watchdog, studio-edge-gradient and studio-smoke.
Still open: p12-t4a/p12-t4b (3D variants), p13-gv2-sfdp, p13-gv2-dot (rank, then mincross, then position),
p12-t3-knobs, osage-knob, trap-followups, studio-3d.

Do: derive every claim from the tree, never from this brief: `git log --oneline origin/develop`,
`git branch -r --no-merged origin/develop`, `scripts/orch/queue.sh status`, `cargo run -q -p graph-cli --
capabilities` (the row statuses), `docs/measurements/*.md` and `docs/decisions/*.md`. Then:
1. Rewrite `docs/reports/STATUS.md`: what is on develop by layer (motor, oracles, studio), each layout row's
   status from `capabilities`, the open branches and jobs, the known gaps (cite each measurement file).
2. Rewrite `prompts/CONTINUE.md` as the next session's first page: the merge floor, the orchestration
   (`scripts/orch/queue.sh`, `oc-job.sh`, `land`), what is in flight, the next work in order.
3. Add a dated "2026-10-01" block at the top of `prompts/RESUME.md`; do not delete older blocks.
4. Update the Status line at the top of each `prompts/phase-NN-*.md` whose state changed.

Paths: those files only. No code. Every factual line cites a commit, a file:line or a command.

Done when: `git diff --stat` touches only those paths, and the return block lists each claim you could not
verify. Leave everything uncommitted; the orchestrator commits.
