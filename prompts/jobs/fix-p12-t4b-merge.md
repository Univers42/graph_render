# Job fix-p12-t4b-merge (agent build: finish the p12-t4b merge, two red tests)

`origin/develop` (93d5ed4c) is merged into this branch with every conflict resolved and staged;
`MERGE_HEAD` is set. Follow `prompts/jobs/resolve-merge.md` for the git rules: no `git merge`,
`reset`, `checkout --` or commit; `scripts/orch/job-check.sh commit` concludes the merge.

This branch registers five ids develop does not know: `layout.force.yifan_hu.2z`,
`layout.force.fruchterman_reingold.3d`, `layout.force.kamada_kawai.3d`, `layout.force.drl.3d`,
`layout.forceatlas2.3d`. Develop added two tests that list every registered layout, and neither
lists these five. Measured on this tree (`target/job-check/gate/test.log`):

1. `crates/graph-core/tests/geometry_invariants.rs`: `every_registered_layout_has_its_own_sweep`
   and 26 `layout_*` sweeps fail, because the `per_layout_sweep!` rows carry registry indices and
   the five ids shift every index after `layout.force.yifan_hu`. Add one row per new id at its
   registry position and renumber every row from the merged `registry::LAYOUTS` order (read it,
   never copy an index from either side). The file is at 300 lines: move the rows list (or the
   macro) into a child module under `crates/graph-core/tests/` so each file stays at or under 300.
2. `crates/graph-cli/src/hashgate/tests/knob/coverage.rs:89`: the five ids have no per-stage
   negative control. Prefer adding them to the family table beside
   `knobs::THREE_D_LAYOUT_STAGES` (`crates/graph-cli/src/hashgate/knob.rs`) when the 2D arm's knob
   reaches the 3D arm; otherwise add them to `NO_PER_STAGE_CONTROL` with the `Gap` kind that is
   true, and resize the array. Whichever you pick, the hash gate's negative control must still turn
   red for that id: run `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli --
   hashgate --seeds 8` and expect non-zero.

Out of bounds: motor math, any tolerance, any oracle ceiling, `harness/golden-2d-before.txt`.

Gate: `scripts/orch/job-check.sh start scripts/orch/rows/p12-t4b.rows`, then `wait 580` while it
answers 3. Before starting it, `scripts/orch/gr cargo test -p graph-core --test geometry_invariants`
and `scripts/orch/gr cargo test -p graph-cli --bin graph-cli knob::coverage` must be green. On PASS,
`scripts/orch/job-check.sh lint` (0 errors) and `scripts/orch/job-check.sh commit`.

Done when: `git rev-parse -q --verify MERGE_HEAD` prints nothing, the gate PASSed over the
committed tree, and the return block lists each file changed and the knob-or-gap choice per id.
