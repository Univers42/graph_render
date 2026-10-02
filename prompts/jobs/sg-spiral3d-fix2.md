# Job sg-spiral3d, review round 1 (same session, same worktree): FIX, 1 MAJOR, 7 MINOR

The review is `docs/reviews/rv-sg-spiral3d.md` on branch `rv-sg-spiral3d`. Read it with
`git show origin/rv-sg-spiral3d:docs/reviews/rv-sg-spiral3d.md`. It confirms the spiral port, its
tests and D10 (hashgate 8 is 4-way equal). Fix every item below, then return a new block. Your old
block does not count: this round must end in a commit past the current head.

This round folds in job `knobs-3d-new` (`prompts/jobs/knobs-3d-new.md`), which the orchestrator
retires: its precondition "sg-spiral3d is on develop" is replaced by "this branch". It widens your
paths to the files that brief names in its step 1, its step-2 test, `docs/measurements/sg-spiral3d.md`,
`crates/graph-core/src/registry/three_d.rs`, `crates/graph-core/src/layout/basic_3d.rs` (doc comments
only) and `crates/graph-cli/src/oracle_python/conformance/gaps.rs` (the two `at:` fields only).

MAJOR
1. S3-1. Do `knobs-3d-new.md` steps 1-3 here, for both `layout.basic3d.spiral` and
   `layout.bipartite_3d` (`bipartite_3d` is already on develop; merge develop first if your branch
   lacks it). The step-2 coverage test is RED before step 1 (paste it naming both ids) and GREEN after.
   Each new knob turns `hashgate --seeds 8` red: paste both runs' exit codes and last lines.

MINOR (the review's "What to change", items 2-6)
2. `docs/measurements/sg-spiral3d.md:196`: `70 rows` -> the count your tree prints.
3. `docs/measurements/sg-spiral3d.md:292`: `LAYOUTS: 35 -> 36` -> the true before/after count.
4. `registry/three_d.rs:84`: "five layouts" -> the true count.
5. `registry/three_d.rs:33` and `layout/basic_3d.rs:47`: name the three placements that were
   measured `O(n)`; "each of the three functions it calls" -> four. Do not contradict
   `three_d/spiral3d.rs:79-84`.
6. `conformance/gaps.rs:39` and `:45`: both `at:` fields -> the line they name today.
7. Write `docs/measurements/knobs-3d-new.md` (the knob runs, the coverage test's allow list with a
   reason per entry).

Proof (paste each command and its last lines):
- `hashgate --seeds 8` exit 0, and its `GM_MUTATE_REFERENCE_DEGREE=9` control non-zero.
- The per-stage digests of every layout are unchanged from the review's run (knobs touch only
  the mutated arm); paste the `layout.basic3d.spiral` and `layout.bipartite_3d` lines before and after.
- `codegen --check` exit 0; `capabilities --check` shows no new problem naming `basic3d` or `bipartite`.
- `scripts/scigraphs-conformance.sh` exits 0 with no row moved.
- The merge floor: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`.
