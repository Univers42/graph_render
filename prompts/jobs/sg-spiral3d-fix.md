# Job sg-spiral3d, review round 1 (same session, same worktree): FIX, 4 MAJOR, 4 MINOR

The review (graph-render-3e, 2026-10-02) confirms the math: the formulas match `basic.py:36-63`, and the pinned
hex for n=1, 2 and 7 was regenerated in ge-python-oracle with numpy 2.3.3 and is equal. The registry split is
byte-identical, and D1-D10 hold. The evidence and some claims are wrong. Fix every item, then return a new block.
Your old block does not count: this round must end in a commit past the current head.

MAJOR
1. `capabilities/registry/unproven.rs:43-49,155-168` claims oracle-basic-3d covers spiral. It does not:
   ARMS in `harness/oracle-basic-3d.py` and `oracle_python/basic_3d.rs:35-40` cover sphere, helix and cube only.
   Take spiral off the oracle claim and name the handoff in the report. Job `sg-basic3d-spiral-oracle` adds
   the arm after this branch lands.
2. The new stage has no hash-gate knob (THREE_D_LAYOUT_STAGES `knobs.rs:127`, KNOBS). Do not add one here:
   state in the report that the orchestrator's follow-up job adds the spiral and bipartite_3d stages and their
   negative controls once both branches are on develop.
3. `docs/measurements/sg-spiral3d.md:32-35,58-67,105-123` has exit codes only. Paste the real output lines
   for 120/1020, 1020/1020, 2.68e+08, 2.35e-07, 3.34e-16 and 5.59e-16, or commit the script that produced
   them and paste its run. Line 67 says "68 node counts", but 1..64 plus 77, 100, 101, 600 and 601 is 69.
   `scigraphs-conformance.md` row 14 and repair 10 cite this report, so check them too.
4. `spiral.rs:130-138`: at n=0 the port returns empty geometry. The reference returns a (1,3) array, then
   `_check_positions` (`common.py:175-185`) raises and `apply_graph_layout` returns False. Document this
   divergence in the code and in the report.

MINOR
5. "0.9999999999999986" is false: 65535*step == 1.0 exactly (0x3ff0000000000000), so the `j==GRID-1`
   branch in `grid_at` is dead. Fix `spiral.rs:28-31,88-93`, `structure.rs:68-74,181-198` and `spiral3d.rs:33-34`.
   Show the exact product with a test or a pasted run.
6. The `max(2,…)` floor matters only for n=1..5. At n=6..14 the rounded value is already 2, and at n=15..16
   it is 3. Correct the comments that cite n=7, 9 and 14: `structure.rs:111-116`, `reference.rs:75-76`,
   `spiral.rs:67-70`, `spiral3d.rs:53-55`, and report lines 156-157.
7. `spiral3d.rs:51`: BASIC_3D_CEILING was never benched for spiral, which runs a fixed 65 536-point pass.
   Mark it unmeasured, with a `Ponytail:` line.
8. `basic_3d.rs:46` and `cli.rs:101-123` still say "three".
(9, a note: no test separates round_ties_even from round(). Optional.)

Done when: quick.rows green (hashgate-8 and its negctl included), `scripts/scigraphs-conformance.sh` exit 0
and its `--break` exit 1, every exit code pasted, and a commit past the head this round started from.
