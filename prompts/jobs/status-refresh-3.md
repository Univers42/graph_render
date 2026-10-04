# Job status-refresh-3 (agent build, branch status-refresh-3, worktree ~/goinfre/wt/status-refresh-3)

Context: `docs/reports/STATUS.md` and `prompts/RESUME.md` name items as open that have since closed
on develop. This job is documentation only. **Run no cargo, npm, bench or gate command**: a peer's
timed gate holds the CPU. Every fact you write is re-checked first with `git grep -n`, `git log`,
`git tag -l` or `git for-each-ref`, and cited as `file:line` or as the command. If a claim below is
wrong when you check it, say so in your report and leave that text alone.

Facts measured on develop 43945a7c (2026-10-04), to re-check:
1. `scripts/orch/rows/develop-full.rows:190` has a `negctl-node-z` row. STATUS.md §5 ("There is
   still no `negctl-node-z` row") and RESUME.md:46 say it is missing.
2. `studio-switch-fit` is done: `deploy/nav/nav.py:78` passes `expect_switch_stale=broken`, and
   `deploy/nav/switchrows.py` is the probe. STATUS.md §4.1 and RESUME.md:44,104 say it is open.
3. `p12-t4a` is archived: `git tag -l 'archive/*'` lists `archive/p12-t4a`, and no
   `origin/p12-t4a` branch exists. STATUS.md §2 and §4.1, RESUME.md:101,114,139 still list it.
4. `layout.force.yifan_hu`, `.2z` and `.3d` now carry the `stress` record
   (`crates/graph-cli/src/capabilities/registry/unproven.rs`, the three arms naming them). STATUS.md
   §5 and RESUME.md:45,171 say "no oracle at all". The accurate text: a stress differential, no
   coordinate oracle.
5. `docs/decisions/live-force-session.md:3` now reads "M1–M3 landed; M4 not started". STATUS.md §5
   says "M2–M4 are not started".
6. Check `docs/decisions/contract-3d.md:3` and correct STATUS.md §5's "still marked proposed" only
   if the line changed.
7. STATUS.md §2's branch table is stale. Rebuild it from
   `git for-each-ref --no-merged=origin/develop --format='%(refname:short)' refs/remotes/origin`
   and `git rev-list --count origin/develop..<branch>` (run `git fetch -q origin` first). Keep the
   table's columns. For each branch, one short "What / why" from `git log -1` and
   `git diff --stat origin/develop...<branch> | tail -n 1`. Do not judge whether a branch should land.
8. A `scratch/` hygiene change landed: the tracked `g.dot`, `g.dot.dot` and `scratch/*/` run outputs
   were removed and `/scratch/` is in `.gitignore` (check `git log -3 --stat -- .gitignore`). Add
   one line to STATUS.md's "Closed since" paragraph.

Exact tasks:
1. Re-check facts 1-8 and paste each command with its output (or its first 5 lines).
2. Edit `docs/reports/STATUS.md`: update the date line, §2 (fact 7), §4.1 (facts 2, 3) and §5
   (facts 1, 4, 5, 6), and the "Closed since" paragraph (facts 1, 2, 3, 8). Change only those lines.
3. Edit `prompts/RESUME.md`: lines 44-46, 101-107, 114, 139 and 171 for facts 1-4. Change only the
   words that are now false; keep each line's citations if they still hold.
4. Commit nothing that is not in the allowed paths. The orchestrator gates the tree with
   `scripts/orch/rows/docs.rows` (no code path changed) after you finish.

Paths allowed: `docs/reports/STATUS.md`, `prompts/RESUME.md`, `prompts/jobs/status-refresh-3.md`.
Not allowed: everything else.

Done when, each with its command and exit pasted: every fact 1-8 is re-checked with its command;
`git diff --stat origin/develop` lists only the three allowed paths.
