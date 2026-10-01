# Job merge-tier-settle (agent build, merge and resolve)

Why: `tier-settle` (pushed, head 6ab44f9) gives yifan_hu, and barnes_hut's settle, the thread tier:
`hashgate --tiers all`, bench rows, `docs/measurements/tier-settle.md`. It passed quick.rows on its own
tree. It conflicts with `tier-closed-form`, which is already on develop and gives grid, ring and spiral
their tiers plus the new knob `GM_MUTATE_SPLIT_RESCALE`. The conflicts are in
`crates/graph-cli/src/bench/tiers.rs`, `bench/tiers/{markdown,sweep,tests}.rs` and
`crates/graph-cli/src/hashgate.rs`.

First, in this worktree (a branch made from develop): run `git merge --no-commit origin/tier-settle`.
Resolve every conflict by editing files. After the merge, never run git commands that change state.

Rules:
- Keep both intents (CLAUDE.md "Parallel branches"). Every layout that either branch put in the bench's
  tier sweep and in `hashgate --tiers all` stays in it, and both branches' knobs stay in the single knob list
  (`crates/graph-cli/tests/common/mod.rs`, `Knob::ALL`). If both branches added a knob at the same position,
  both knobs stay and the count tests follow by id.
- House limits: at most 40 lines per function, 300 lines per file (split into child modules), 4 params.

Checks (paste each last line): the quick.rows set; `hashgate --seeds 8 --tiers all` exits 0; with
`GM_MUTATE_SPLIT_SUM=1` it exits 1; with `GM_MUTATE_SPLIT_RESCALE=1` it exits 1; and
`bench --layout layout.force.yifan_hu,layout.circular.ring --n 220 --tiers scalar,threads --workers 2
--repeat 1` exits 0. Check each command's flags with `--help` first.

Paths you may touch: the conflicted files, `crates/graph-cli/src/bench/**`, `crates/graph-cli/src/hashgate/**`,
`crates/graph-cli/tests/**`.

Done when: `git grep -n -e '^<<<<<<<' -e '^>>>>>>>' -- crates` prints nothing, the checks pass, and the
return block lists each hunk's resolution, one line per hunk.
