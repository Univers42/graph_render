# Job circo-dup-leaf (agent build, branch circo-dup-leaf, worktree ~/goinfre/wt/circo-dup-leaf)

Context: the job `circo-cross-fast` (this branch's base) reported a HIGH finding. In
`crates/graph-core/src/layout/graphviz/circo/skeleton/tree.rs`, `longest_path` (`:84-105`) climbs
from `leaves.best(common)` and then from `leaves.runner_up(common)`. When the thinned tree is a
forest, both climbs can start at the same leaf, so the returned order carries a node twice. Its
measured example: seed 68, a block of 44 nodes with an order of 45. The crossing reduction then
counts that node's edges twice.

Licence, hard rule: Graphviz is EPL-1.0. Read
`~/goinfre/refs/graphviz-16.1.0/graphviz-16.1.0/lib/circogen/blockpath.c` for behaviour only. Never
copy its text, comments or identifiers beyond the function names our port already uses, and never
translate it line by line.

Exact tasks:
1. Reproduce first. Write a failing test in `circo/tests.rs` (or a child `circo/tests/path.rs`)
   that builds the seed-68 block, or the smallest forest you can find that shows it, and asserts
   that the order from `longest_path` has no repeated node. Observe it RED and paste the failure.
2. Decide what the reference does with the same input. Read `blockpath.c`'s
   `find_longest_path`/`measure_distance` and, if it helps, run the oracle (`ge-graphviz-oracle`,
   through `scripts/orch/drun`) on that seed. Write down in your own words whether the reference
   can also repeat a node, or whether something we did not port prevents it (for example a
   different runner-up rule, a guard on the second leaf, or the forest never reaching this path).
3. Before editing the layout, record the circo differential: run the rows `circo-emit-1000`,
   `circo-oracle-1000`, `circo-merge-1000` and `circo-check-1000` from
   `scripts/orch/rows/p13-gv1-circo.rows` by hand and keep their output (paste the summary line).
4. Fix the root cause in `skeleton/tree.rs` (or the module that picks the runner-up) so the order
   is a permutation of the block's nodes, matching what step 2 found the reference does. Integer
   arithmetic, determinism rules D1-D10 (`prompt.md` §6), no `HashMap`, no new dependency.
5. Re-run step 3's four rows. Report before vs after: how many seeds agree with the oracle and the
   worst residual. If agreement drops, stop and report the seeds rather than forcing a fix.
6. Add a section "## 4c. A node placed twice (2026-10-04)" at the end of
   `docs/measurements/p13-gv1-circo.md`: the reproducing input, what the reference does, the fix,
   and the step-5 numbers with their commands. Do not edit any other section.

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, every heuristic
carries a `Ponytail:` line. Do not touch the registry.

Paths allowed: `crates/graph-core/src/layout/graphviz/circo.rs`,
`crates/graph-core/src/layout/graphviz/circo/**`, `docs/measurements/p13-gv1-circo.md`,
`prompts/jobs/circo-dup-leaf.md`. Not allowed: everything else.

Done when, each with its command and exit pasted:
- the step-1 test is GREEN and was observed RED;
- `scripts/orch/gate.sh target/gate-circo-dup scripts/orch/rows/p13-gv1-circo.rows` all PASS;
- the before and after differential numbers are in the measurement file.
