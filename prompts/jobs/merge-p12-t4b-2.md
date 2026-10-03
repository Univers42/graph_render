# Job merge-p12-t4b-2 (agent build: finish the p12-t4b merge on today's develop)

Read `scripts/orch/common.md` first.

Why: `merge-p12-t4b` (2026-10-02) resolved p12-t4b against an older develop, but its second round copied
develop's files by hand instead of merging, so git now sees 40 add/add conflicts there and the branch can
never land. This job redoes the merge as a real merge commit. p12-t4b brings the 3D arms of the force
kernels and spectral's z axis (`registry/arms_3d.rs`, `layout/spectral/z_axis.rs`, the `*.3d` / `*.2z` ids).

State of your worktree: a branch cut from `origin/develop`, with `git merge --no-commit origin/p12-t4b`
already run by the orchestrator. `MERGE_HEAD` is set. Never run a git command that changes state; the
gate commits the merge.

The 11 conflicted files (`git diff --name-only --diff-filter=U`):
- `crates/graph-cli/src/capabilities/registry/unproven.rs`
- `crates/graph-cli/src/capabilities/tests/registry.rs`
- `crates/graph-cli/src/hashgate/tests/report.rs`
- `crates/graph-cli/src/oracle_python.rs`, `crates/graph-cli/src/oracle_python/cli.rs`
- `crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs`
- `crates/graph-core/src/layout/force/kamada_kawai.rs`
- `crates/graph-core/src/layout/forceatlas2.rs`, `crates/graph-core/src/layout/forceatlas2/state.rs`
- `crates/graph-core/src/registry.rs`
- `docs/measurements/scigraphs-coverage.md`

Reference: the first round's resolutions are on branch `merge-p12-t4b`. Read them with
`git show origin/merge-p12-t4b:<path>`. They were checked against an older develop, so take the
reasoning, not the bytes. What that round established (its return block, 2026-10-02):
- FA2 gravity must stay develop's association `g*(centred/norm)`. p12-t4b's `g*centred/norm` moves
  484 of 4102 coordinates; the rewrite is load-bearing. Pin it with a unit test this time (that round
  pinned it by comment only).
- `layout/forceatlas2/state/barnes_hut.rs` and its tests had to be ported to p12-t4b's row layout:
  develop's BH arm does not compile against p12-t4b's `Fa2State`.
- `layout/spectral/z_axis.rs` must call develop's 3-argument `ComponentGraph::build`.
- The D9 jiggle guard at `state.rs` became `norm2(&d, dim) == 0.0` where develop had
  `dx == 0.0 && dy == 0.0`; keep it only if a test shows 2D bytes unchanged.

Do, in order:
1. Resolve every conflict by editing files, keeping both intents (CLAUDE.md "Parallel branches"):
   registry and test lists are unions; 2D behaviour is develop's, byte for byte; p12-t4b's 3D arm
   sits on top. A conflict whose two sides cannot both hold is a stop: quote both sides under
   "decisions needed" with your recommendation.
2. Fix what the merge breaks without conflicting (build errors, new registry rows the tests count).
3. House limits. After the union these are over 300 lines: `registry.rs` (338), `capabilities/tests/registry.rs`
   (340), `forceatlas2/state.rs` (357). Split each into child modules. Pure moves: no behaviour change.
   develop already split part of the registry test into `capabilities/tests/registry/`. Follow that.
4. The 2D golden is already re-pinned (orchestrator, 2026-10-03): `harness/golden-2d-before.txt` now
   holds `harness/golden-2d.sh`'s output on develop f1a2dd4. Against the 2026-10-02 pin only
   `layout.dag.sugiyama`'s 8 lines moved, and they moved on develop, not in this merge. Do not edit the
   pin. If `golden-2d` goes red, the merge moved a 2D byte: find it and fix it.
5. Run the rows in `scripts/orch/rows/p12-t4b.rows`, plus `cargo build -p graph-core --target
   wasm32-unknown-unknown` and `hashgate --seeds 8` with its `GM_MUTATE_REFERENCE_DEGREE=9` negative control.

Paths: the 11 files above, their new child modules, `layout/forceatlas2/**`, `layout/spectral/z_axis.rs`,
`docs/measurements/p12-t4b.md`.

Out of bounds: every other file. If the build needs one, report it as a deviation.

Done when:
- No conflict markers: `git diff --check` is clean, and `git grep -n -e '^<<<<<<<' -e '^>>>>>>>'` is
  empty.
- Every row of `p12-t4b.rows` passes. `golden-2d` passing means the merge moved no 2D byte of
  today's develop.
- `job-check.sh lint` shows 0 ERROR.
- The return block pastes each real exit code.
