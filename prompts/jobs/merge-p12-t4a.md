# Job merge-p12-t4a (agent build; merge p12-t4a onto develop, drop the arms develop already has)

Why: p12-t4a (`origin/p12-t4a`, 971318d) conflicts with develop in the capability tests and
`crates/graph-core/src/registry.rs`. It also brings two layouts develop now carries under other ids,
each from the same SciGraphs function:

| p12-t4a | develop (landed from) | SciGraphs |
|---|---|---|
| `layout.spiral.3d` (`layout/spiral/spiral_3d.rs`) | `layout.basic3d.spiral` (sg-spiral3d) | `_spiral_layout_3d` |
| `layout.bipartite.3d` (`bipartite.rs` `run_3d`) | `layout.bipartite_3d` (sg-bipartite3d) | `_bipartite_layout_3d` |

Released only after merge-p12-t4b, sg-spiral3d and sg-bipartite3d are on develop. Check that first
(`git merge-base --is-ancestor origin/<b> origin/develop` for each); if one is not, stop and report.

Do:
1. In this worktree (a branch made from develop): `git merge --no-commit origin/p12-t4a`. Resolve every
   conflict keeping both intents (CLAUDE.md "Parallel branches"): registry and test lists are unions.
2. Drop p12-t4a's two duplicate arms: `layout/spiral/spiral_3d.rs` and its tests, the `run_3d`/`ID_3D`
   arm of `bipartite.rs`, their registry entries, ledger rows and oracle cases. Keep develop's two ids.
   Before deleting, compare each dropped test with develop's for the same layout; a property only
   p12-t4a tests (name it) is ported onto develop's id, otherwise list it as dropped in the report.
   Keep `layout.random.3d`, `layout.spectral.3d`, `layout.mds.pivot.3d` and the lobpcg changes.
3. No other edits. A conflict whose two sides cannot both hold is a stop: report it under
   "decisions needed" with both sides quoted and your recommendation.
4. Run fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, the wasm32 build of
   graph-core, `capabilities --check` (paste the problem list filtered to the three kept ids: empty),
   and `hashgate --seeds 8` with its `GM_MUTATE_REFERENCE_DEGREE=9` negative control.

Report: `docs/measurements/merge-p12-t4a.md` (conflicts and how each was resolved; the dropped-arm
table with each test kept, ported or dropped).

Done when: no conflict markers (`git diff --check` clean, `git grep -n '^<<<<<<<\|^>>>>>>>'` empty),
`git grep -n 'layout.spiral.3d\|layout.bipartite.3d' crates` is empty, every command above exits as
required, and the return block pastes each real exit code. After the merge, never run git commands
that change state; the gate commits.
