# Job circo-cross-fast (agent build, branch circo-cross-fast, worktree ~/goinfre/wt/circo-cross-fast)

Context: `layout.circular.circo` costs `O(k^3)` in its largest block's size `k`
(`docs/measurements/p13-gv1-circo.md` §4: 5 743 ms at 1 000 nodes, 206 131 ms at 3 520). The cost
is `crates/graph-core/src/layout/graphviz/circo/circle.rs`: `pass` (`:111`) tries two moves per
incident edge per node, and each try calls `count_all_crossings` (`:148`), which walks every open
edge for every closed one. The same cost makes the 1000-seed circo sweep take 4.7 h (§3).

`count_all_crossings` counts the pairs of chords that interleave around the circle: a pair
(e, f) is counted when e opens before f, f opens before e closes, and e closes before f does,
with "before" strict and pairs sharing a node never counted. That count can be taken in
`O(E log E)` with a Fenwick (binary indexed) tree over positions. The output must not change by
a single byte: same crossing numbers, so `pass` keeps exactly the same moves.

Exact tasks:
1. Before any edit, record the baseline into `target/circo-before/`: for every seed 0..=49, run
   `scripts/orch/gr cargo run -q --release -p graph-cli -- snapshot --seed <s> --layout layout.circular.circo --out-bin target/circo-before/<s>.bin`.
   Then the timing baseline: `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.circular.circo --past-ceiling --repeat 3`
   Read `bench --help` first and pass the sizes explicitly so no size runs past 3 520 nodes
   (§4's table is 64, 128, 220, 256, 440, 512, 880, 1 000, 1 024, 1 760, 2 000, 3 520). Run it under
   `timeout 1800`; a size that does not finish is recorded as such, never extrapolated.
2. In `circle.rs` (or a child module `circo/crossings.rs` if `circle.rs` would pass 300 lines),
   write the `O(E log E)` count. Integer arithmetic only, no `HashMap`, no new dependency, no float.
   Describe the method in your own words in a doc comment. Keep the old count as a test-only
   function (`#[cfg(test)]`) and call the new one from `reduce` and `pass`.
3. Tests, in `circo/tests.rs` or a new child `circo/tests/crossings.rs`:
   - a property test without a new dependency: a fixed-seed integer generator (an LCG written in the
     test) builds 2 000 random blocks (2..=40 nodes, random simple edges, no self-loops) and random
     orders, and asserts the new count equals the old one on every case;
   - closed cases: K4 in order 0,1,2,3 has exactly 1 crossing; a triangle has 0; two parallel chords
     have 0; K2,2 drawn as 0,2,1,3 vs 0,1,2,3 (work the expected numbers by hand and write them).
4. Also remove the per-candidate allocations in `pass` where it is free (for example reuse one
   scratch `Vec` for `candidate`), without changing which move is kept.
5. Prove the bytes did not move: rebuild, write seeds 0..=49 into `target/circo-after/`, and
   `cmp` every pair (paste: "50 of 50 identical" or the first differing seed — a difference is a
   stop, not a finding). Then `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8`.
6. Re-run the step-1 bench command. Add a section "## 4b. After the Fenwick crossing count
   (2026-10-04)" to `docs/measurements/p13-gv1-circo.md` with the command, the before and after
   tables, and one sentence on the new growth rate per doubling. Do not edit §4 or any other
   section, and do not change the registry entry's `scale_ceiling` (report what the numbers would
   support instead).

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, every heuristic
carries a `Ponytail:` line, determinism rules D1-D10 (`prompt.md` §6).

Paths allowed: `crates/graph-core/src/layout/graphviz/circo.rs`,
`crates/graph-core/src/layout/graphviz/circo/**`, `docs/measurements/p13-gv1-circo.md`,
`prompts/jobs/circo-cross-fast.md`. Not allowed: everything else, the registry included.

Done when, each with its command and exit pasted:
- `scripts/orch/gate.sh target/gate-circo-cross scripts/orch/rows/quick.rows` all PASS;
- `scripts/orch/gr cargo test -p graph-core circo` passes, with the property test's case count;
- step 5's `cmp` reports 50 of 50 identical;
- the before and after bench tables are in the measurement file with their command.
