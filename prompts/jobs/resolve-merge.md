# Round: finish a merge of develop that the orchestrator started (same worktree)

`origin/develop` is already being merged into this branch: `git status` shows the conflicted files
and `MERGE_HEAD` is set. This round only finishes that merge; it adds no feature.

Do not run `git merge`, `git merge --abort`, `git rebase`, `git reset`, `git checkout -- <file>` or
`git pull`. Do not commit: `scripts/orch/job-check.sh commit` concludes the merge after the gate.

1. `git diff --name-only --diff-filter=U` lists the conflicted files. For each one, read both sides
   (`git show :2:<file>` is this branch, `git show :3:<file>` is develop) and keep both intents:
   - a list, table or registry is the union of both sides, in registry order; a count, length or
     index that a test asserts is recomputed from the merged list, never copied from one side;
   - code that develop refactored (moved, renamed, a helper extracted into a shared module) takes
     develop's shape, and this branch's addition is re-expressed in it — never a second copy of a
     helper develop already has;
   - a test or block that develop moved to another file is edited in its new place;
   - a doc table (`docs/measurements/*.md`) keeps develop's rows plus this branch's;
   - a conformance `row(...)` keeps this branch's numbers only for the rows this branch owns.
   Then `git add <file>`. Write down each file and the rule you applied.
2. No file over 300 lines and no function over 40 lines after the merge; split into a child module
   if the union crosses a limit.
3. Gate with `scripts/orch/job-check.sh start <rows>` and `scripts/orch/job-check.sh wait 580`
   (repeat `wait` while it answers 3). A red row the merge caused is fixed in this round, smallest
   change, inside the conflicted files or the files they touch. A red row develop already has is
   reported, not fixed.
4. `scripts/orch/job-check.sh commit` on a PASS.

Done when: `git rev-parse -q --verify MERGE_HEAD` prints nothing, `git merge-tree --write-tree
origin/develop HEAD` reports no conflict, the gate PASSed over the committed tree, and the return
block lists every conflicted file with its resolution.
