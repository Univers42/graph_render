# Job context-refresh (agent build, docs only)

Why: the "where are we" documents disagree with the tree. `docs/reports/STATUS.md` and
`prompts/CONTINUE.md` describe 2026-09-28 (phases 3–11 "not merged", a 4-CPU host, `/home/user/...`
helpers, a strict per-branch merge rule the user replaced on 2026-09-29). The Status line at the top of
each `prompts/phase-NN-*.md` is also from 2026-09-28. `prompts/RESUME.md` (its HANDOFF 2026-09-30 00:40
section first) is the newest source; the tree and `git` are the final authority.

Facts to use (verified 2026-09-30 by the orchestrator; re-check each with the command given):
- develop holds p0–p11, p12-t1, the studio (S1–S5), `limits`, and the orchestration move. Check:
  `git log --oneline origin/develop | head -40`, `git branch -r --merged origin/develop`.
- Pushed, not merged: `followups2`, `sim`, `p12-igraph`, `studio-force`, `studio-s7`, `studio-ux`. Check:
  `git branch -r --no-merged origin/develop`. `studio-ux` and the studio plan S8 were DROPPED by the user
  on 2026-09-30: say so, and do not plan work on them.
- Host `dlesieur42` since 2026-09-30: 20 cores, 31 GB RAM, no `/goinfre` or `/sgoinfre`; host-local
  state is under `$GM_SCRATCH` (`scripts/orch/scratch.sh`). The `/sgoinfre` orch prompts and rows were
  lost; rows and briefs now live in `scripts/orch/rows/` and `prompts/jobs/`.
- Merge floor (user, 2026-09-29): fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast` on
  the merged tree; one full gate on develop afterwards, red rows become repair tasks. Merge floor
  rows: `scripts/orch/rows/quick.rows`.
- User decisions 2026-09-30: free-model outages mean the job queue waits (no paid fallback); the
  Graphviz engines must match Graphviz's own output (full ports against a Graphviz oracle); a GitHub
  Actions merge-floor workflow with an arm64 hashgate arm is approved.
- Work queue, in order: RESUME first tasks (wasm32 build, hashgate 8 and its negctl, the full gate);
  the merge train `followups2` → `sim` → `p12-igraph`; live forces (force-wasm from `sim`, then
  `studio-force`); `studio-s7`; no-cargo lanes (SciGraphs coverage, thread-tier audit, the 3D devil
  verdict, the Graphviz oracle, CI).

Do:
1. Rewrite `docs/reports/STATUS.md` as the current status: the branch table (merged / pushed-not-merged /
   dropped), the merge rule above, the host facts, the work queue, and the decisions list (keep its §5
   "decisions already taken" items that are still true, and add the 2026-09-30 ones). ≤ 120 lines.
2. Rewrite `prompts/CONTINUE.md` as the orchestrator's how-to for this host: the merge floor, the
   OpenCode worker loop (`wt-new.sh` → `oc-job.sh` with a `prompts/jobs/<id>.md` body → re-gate →
   merge), the token rule (free-model workers do the edits, the orchestrator verifies), the standing
   rules pointer to CLAUDE.md. Drop the stale 4-CPU, `/home/user`, `claude/sharp-turing-er9nve` and
   strict-sequence text. ≤ 100 lines.
3. Update ONLY the `> **Status (...)**` line at the top of each `prompts/phase-NN-*.md` to its true state
   (merged into develop, with the merge commit from `git log --merges --oneline origin/develop | grep`
   where you can find it; or "partly merged, see RESUME" for p9–p11). Do not touch anything else in
   those files.
4. Append to `prompts/RESUME.md` a section `## New host 2026-09-30` (≤ 20 lines) with the host facts,
   what was lost, where it lives now, and that the first tasks still stand.

Paths you may touch: `docs/reports/STATUS.md`, `prompts/CONTINUE.md`, `prompts/phase-*.md` (Status
line only), `prompts/RESUME.md` (append only). Nothing else.

Done when: every fact you write is backed by a command you ran (list them in the return block);
`git diff --stat` shows only the paths above.
