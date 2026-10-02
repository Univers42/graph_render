# Round: catch up with develop (same session, same worktree)

Your branch no longer merges into `origin/develop`: the lander skipped it as "does not merge".
This round only merges; it adds no feature and repairs no finding.

1. `git fetch origin && git merge origin/develop -m updated`. Never rebase, never `git pull`.
2. Resolve every conflict keeping both intents (CLAUDE.md "Parallel branches"):
   - a list, table or registry is the union of both sides, in registry order; a count, length or
     index that a test asserts is recomputed from the merged list, never copied from one side;
   - a test or block that develop moved to another file is edited in its new place, and the old
     file takes develop's version;
   - a doc table (`docs/measurements/*.md`) keeps develop's rows plus yours;
   - a conformance `row(...)` keeps your branch's numbers only for the rows your job owns.
   Write down each conflicted file and the rule you applied.
3. Commit the merge (`updated`). Then run, on the merged tree, and paste the last lines of each:
   `scripts/orch/gr cargo fmt --all --check`, `scripts/orch/gr cargo clippy --workspace
   --all-targets -- -D warnings`, `scripts/orch/gr cargo test --workspace --no-fail-fast`,
   `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown`,
   `scripts/scigraphs-conformance.sh`, `hashgate --seeds 8` (exit 0) and its
   `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero).
4. A red row that the merge caused is fixed in this round, smallest change, inside your job's
   paths or the conflicted files. A red row that develop already has is reported, not fixed.

Done when: `git merge-tree --write-tree origin/develop HEAD` reports no conflict, every command in
step 3 is pasted, and your return block lists the conflicted files with their resolution.
