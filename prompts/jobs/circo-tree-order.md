# Job circo-tree-order (agent build, branch circo-tree-order, worktree ~/goinfre/wt/circo-tree-order)

Context: read `docs/measurements/p13-gv1-circo.md` §8 first, all of it. Job circo-tie-order found
the first cause of circo's gap against Graphviz (the reference's crossing set never shrinks, §8.3)
and fixed it. §8.6 found a **second, separate** cause and stopped there. This job finds and fixes
that one.

The evidence §8.6 left: seed 100 (`n = 102`), one block of 74 nodes. The reference's longest path
and ours share their branch node `n6` and their whole `DISTTWO` half, and differ on the `LEAFONE`
half. The reference climbs `38-17-28-95-71-48-1-21-16-20-5-26-4-86-63-37-6`; ours climbs the same
middle chain the other way, `30-81-37-63-86-4-26-5-20-16-21-1-48-99-6`. So the two spanning trees
are made of different edges, and the divergence is upstream of the crossing reduction: in
`remove_pair_edges`, in the tree built from what it leaves (`circo/skeleton.rs` and the files it
calls), or in `find_longest_path`'s choice of leaf. §8.6 rules out the degree sort: glibc 2.41's
`qsort` is stable (measured, §8.1).

Licence, hard rule: Graphviz is EPL-1.0. Read
`~/goinfre/refs/graphviz-16.1.0/graphviz-16.1.0/lib/circogen/*.c`, `lib/cgraph/*.c` and
`lib/util/list.{c,h}` for behaviour only. Never copy their text, comments or identifiers beyond the
function names our port already uses, and never translate them line by line.

Tools already built: an instrumented Graphviz 16.1.0 with `fprintf` traces in
`lib/circogen/blockpath.c` is at `$GM_SCRATCH/circo-trace/` (`bin/dot_static`, sources in `src/`).
Add the traces you need there (rebuild inside a `scripts/orch/drun` container from
`debian:trixie-slim`, as the previous job did). Commit none of it.

Exact tasks:
1. Seed 100, the 74-node block. Trace both arms step by step: the reference's working list after
   every `remove_pair_edges` round (41 rounds), the pair edges it adds, the edges it deletes, the
   rows `spanning tree` walks, the tree's parent of every node, and `DISTONE`/`DISTTWO`/`LEAFONE`
   at the branch node. Print the same from the port in a `circo/tests` test (`--nocapture`). Name
   the **first** step where the two differ, with both arms' values at that step.
2. Explain that step from the reference's rules: what the reference does that the port does not
   (an order, a tie, a pointer identity, a list operation). Say which source lines show it.
3. Baseline before any edit, on the strided 50-seed subset:
   `scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine circo --seeds 1000`,
   then `scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-sub --differential --shards 20 --shard 0`.
   §8.5's "after" row is the expected baseline (4 of 50 within 1 point, 12 within 1 000); if yours
   differs, say so and use yours.
4. Fix the cause step 1 named, in the circo port. Integer and index arithmetic only. Determinism
   rules D1-D10 (`prompt.md` §6). No `HashMap`, no new dependency. Every existing circo test stays
   green unchanged, including the closed cases and `tests/seed8.rs`. Never weaken or delete a test
   to get green. Add a test that pins the step you fixed on the smallest block that shows it.
5. Re-run step 3's subset; report before against after per seed that moved. If agreement does not
   rise, stop and report what you ruled out, with its evidence.
6. Do **not** run the 1000-seed rows (`circo-oracle-1000` and the rest): they need the host-wide
   timed lock and the orchestrator runs them after landing.
7. The SciGraphs conformance row pins circo's motor bytes. When your fix changes them,
   `scripts/scigraphs-conformance.sh` fails on `GRAPHVIZ_CIRCO` and writes a proposed row to
   `target/scigraphs-conformance/conformance-baseline-proposed.rs`. Copy **only its motor sha**
   (the second argument) into the `GRAPHVIZ_CIRCO` row of
   `crates/graph-cli/src/oracle_python/conformance/baseline/table/graphviz.rs`. Leave the
   reference sha, the `1e0` ceiling, the tier and the cause as they are.
8. Docs: add "## 9. The second cause (2026-10-05)" at the end of
   `docs/measurements/p13-gv1-circo.md`, with step 1's divergent step and both arms' values, the
   explanation, the fix, the before/after subset numbers and their commands, and what still does
   not match. Add one line under §8.6's heading: "Narrowed in §9." Update the circo entry's
   `ponytail` and `degradation` text in `crates/graph-core/src/registry/graphviz_circo.rs` if they
   are now false (text only).

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, no type casts
that truncate silently, every heuristic carries a `Ponytail:` line. LAYOUTS in the registry is
append-only and must not change.

Paths allowed: `crates/graph-core/src/layout/graphviz/circo.rs`,
`crates/graph-core/src/layout/graphviz/circo/**`, `crates/graph-core/src/registry/graphviz_circo.rs`
(text only), `crates/graph-cli/src/oracle_python/conformance/baseline/table/graphviz.rs` (the
`GRAPHVIZ_CIRCO` motor sha only), `docs/measurements/p13-gv1-circo.md`,
`prompts/jobs/circo-tree-order.md`. Not allowed: everything else.

Done when, each with its command and exit pasted:
- step 1 names the first divergent step with both arms' values;
- the subset numbers before and after are in §9;
- `scripts/orch/gate.sh target/gate-circo-tree scripts/orch/rows/quick.rows` all PASS on the final tree.
