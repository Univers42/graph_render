# Job circo-tie-order (agent build, branch circo-tie-order, worktree ~/goinfre/wt/circo-tie-order)

Context: `docs/measurements/p13-gv1-circo.md` §5 says circo agrees with Graphviz on only 16 of 1000
seeds because glibc 2.41's `qsort` is not stable, so `LIST_SORT(&dl, cmpDegree)` breaks degree ties
in an order nobody can reproduce. **That cause is false.** The orchestrator measured it on
2026-10-04 in the oracle image itself:

    scripts/orch/drun --rm --pull never --user 0:0 -v "$GM_SCRATCH/circo-trace:/s" ge-graphviz-oracle python3 /s/qsort_stable.py
    glibc 2.41
    n=5 stable 20/20  ... n=552 stable 20/20 ... n=100000 stable 20/20

(libc `qsort` called through `ctypes`, descending int keys from 4 values, 0 on ties; every trial kept
equal keys in input order.) glibc 2.41 sorts with a stable merge sort. So the order among equal
degrees is the order the list held before the sort, and that is deterministic. The port's
`sort_by_degree` (`circo/skeleton.rs:168`) is stable too. The 984 failing seeds come from
something else. Find it, fix it, measure it.

Licence, hard rule: Graphviz is EPL-1.0. Read
`~/goinfre/refs/graphviz-16.1.0/graphviz-16.1.0/lib/circogen/*.c`, `lib/cgraph/*.c` and
`lib/util/list.{c,h}` for behaviour only. Never copy their text, comments or identifiers beyond the
function names our port already uses, and never translate them line by line.

What the reference does (read and confirmed by the orchestrator, check it yourself):
- `getList` (`blockpath.c:91-100`) appends nodes in `agfstnode(g)` order, then sorts.
- `remove_pair_edges` (`blockpath.c:182-222`) pops the back, removes each neighbour's first
  occurrence with an order-preserving shift (`list.c:228-246`), appends neighbours in
  `agfstedge(g, n)` order, then sorts.
- In cgraph, `agfstnode` follows node sequence (creation order), and a node's edge row is its
  out-edges and then its in-edges. Each half is ordered by the other endpoint's node sequence, then
  by edge sequence: read `lib/cgraph/edge.c` and confirm or correct this.

Hypotheses, test them in this order and stop at the first one the evidence confirms:
- H1, node order: the clone graph (`clone_graph`, blockpath.c) creates its nodes in an order that
  is not our block-local `0..n`. That order follows the root graph's node sequence, which is the
  order the DOT file written by `harness/oracle-graphviz.py` first mentions each node. Our dense
  index may sort ids differently (byte order, or the fixture's order).
- H2, edge row order: `half_key` (`circo/skeleton.rs`) assumes node sequence equals our local id,
  and that tail and head are what the DOT file wrote (`a -- b` keeps `a` as tail). Check both, and
  check which end is the tail for the edges `find_pair_edges` adds with `agedge`.
- H3, downstream: the skeleton tie was inferred, never shown. The order difference could come
  from the circle placement after the skeleton.

Exact tasks:
1. Take seed 8 (`n = 10`): the block `{0,1,5,8,9}` agrees, but our circle order is `[1, 9, 5, 0, 8]`
   and Graphviz's is `[0, 1, 9, 5, 8]` (§5). Get the DOT file the harness gives Graphviz for that
   seed. Write a test in `circo/tests` that prints (with `--nocapture`) our list after each
   `remove_pair_edges` step and the edge rows used. Derive by hand, from the DOT file and the
   reference's rules above, what Graphviz's list holds at each step. Name the first step where
   they diverge, and which hypothesis that confirms.
2. If hand derivation cannot settle it, you may build an instrumented Graphviz 16.1.0 from
   `~/goinfre/refs/graphviz-16.1.0/graphviz-16.1.0.tar.gz` in a scratch container started with
   `scripts/orch/drun` (base `debian:trixie-slim` plus apt build deps; no prebuilt vendor language
   image). Put it under `$GM_SCRATCH/circo-trace/`, add `fprintf(stderr, ...)` traces there, and
   commit none of it to the repo. Say in the report that you did this.
3. Baseline before any edit, on a strided 50-seed subset:
   `scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine circo --seeds 1000`,
   then `scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-sub --differential --shards 20 --shard 0`.
   Record how many of those 50 cases agree, and the worst gap.
4. Fix the root cause in the circo port: the node order, the edge row order, or whatever step 1
   found. Integer and index arithmetic only. Determinism rules D1-D10 (`prompt.md` §6). No
   `HashMap`, no new dependency. The fourteen closed cases in `circo/tests` must stay green
   unchanged. Never weaken or delete a test to get green.
5. Re-run step 3's subset and report before against after. If agreement does not rise, stop and
   report: name what you ruled out, with its evidence.
6. If the subset rises: run the full sweep once, as the four rows `circo-emit-1000`,
   `circo-oracle-1000`, `circo-merge-1000` and `circo-check-1000` from
   `scripts/orch/rows/p13-gv1-circo.rows`, with the oracle row prefixed by `scripts/orch/timed`
   (the host-wide lock; it is 8 containers for ~35 min). If `circo-check-1000` fails only because
   the measured ceiling it checks against is now too loose or too tight, update that ceiling where
   the check reads it (inside the allowed paths) to the new measured worst, and say so.
7. Fix the claims that are now false:
   - the `Ponytail (tie order)` doc on `sort_by_degree`;
   - the circo entry's `ponytail` and `degradation` text in `crates/graph-core/src/registry/graphviz_circo.rs` (text only, nothing else in the registry);
   - in `docs/measurements/p13-gv1-circo.md`, one line under the §5 heading reading
     "Superseded by §8: glibc 2.41's qsort is stable." Then add a new section
     "## 8. The real cause (2026-10-04)" at the end, with the falsification evidence above, what
     step 1 found, the fix, and the before/after numbers with their commands. Leave the other
     sections as they are.

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, no type casts
that truncate silently, every heuristic carries a `Ponytail:` line. LAYOUTS in the registry is
append-only and must not change.

Paths allowed: `crates/graph-core/src/layout/graphviz/circo.rs`,
`crates/graph-core/src/layout/graphviz/circo/**`, `crates/graph-core/src/registry/graphviz_circo.rs`,
`crates/graph-cli/src/oracle_python/circo.rs` (only the circo
ceiling, only if step 6 requires it), `docs/measurements/p13-gv1-circo.md`,
`prompts/jobs/circo-tie-order.md`. Not allowed: everything else, osage included (it gets its own
job once this one lands).

Done when, each with its command and exit pasted:
- step 1 names the first divergent step and the hypothesis it confirms;
- the subset numbers before and after are in §8;
- `scripts/orch/gate.sh target/gate-circo-tie scripts/orch/rows/quick.rows` all PASS on the final tree;
- if step 6 ran, its four rows' output lines are in §8.
