# Job p13-gv2-dot-mincross (agent build, branch p13-gv2-dot-mincross, worktree ~/goinfre/wt/p13-gv2-dot-mincross)

Context: the dot rank pass is on develop (`crates/graph-core/src/layout/graphviz/dot.rs` and
`dot/*`; 692 of 1000 seeds agree node for node with the oracle). Read
`docs/measurements/p13-gv2-dot.md` first, in full: "What is ported", "Blocker 2" and item 4 of
"The remaining passes, as a draft" are this job's specification. The next pass is mincross: the order
of the nodes inside each rank.

Licence, hard rule: Graphviz is EPL-1.0. Read
`~/goinfre/refs/graphviz-16.1.0/graphviz-16.1.0/lib/dotgen/mincross.c` for behaviour only. Never copy its text, comments or identifiers
beyond the function names the measurement file already uses, and never translate it line by line.
Describe each step in your own words in a doc comment before you write it.

Exact tasks:
1. Commit the probe. `target/probe/rank_oracle.py` was never committed, so the 1000-seed number is not
   reproducible. Write `harness/oracle-dot-probe.py` (it runs inside `ge-graphviz-oracle`), which reads
   the oracle's `-Tplain` output and writes one row per seed: the rank of every node and, new, the
   order of the nodes inside each rank (sort the nodes of a rank by their x, ties by name). Update
   `oracle_probe.rs` to read it. Every container goes through `scripts/orch/drun`, never a bare
   `docker run`; fix the command block in the measurement file the same way.
2. Mincross, as child modules of `crates/graph-core/src/layout/graphviz/dot/` (for example
   `mincross.rs`, `mincross/{build,median,transpose,crossings}.rs`): `build_ranks` (BFS from the
   in-sources, then from the out-sinks), the median pass with `MC_SCALE = 256` and its weighted-median
   branch, `reorder`, `transpose`, the crossing count, and the three passes: passes 0 and 1 with at most
   4 iterations, pass 2 with `MaxIter = 24`, `MinQuit = 8`, `Convergence = 0.995`, keeping the best
   order seen. Integer arithmetic where the reference uses it. Determinism rules D1-D10 in
   `prompt.md` §6 apply: `IndexMap`/`BTreeMap`, never `HashMap`; no float reduction in hash order.
3. Tests: one closed-case test per step (median of an even and odd neighbour count, a transpose that
   removes one crossing, a crossing count on a hand-drawn K2,2), the six closed cases' order as the
   oracle prints them (the table under "The six closed cases"), and the 20 fixture seeds.
4. The 1000-seed sweep: an `#[ignore]`d test `order_agreement_over_1000_seeds` beside
   `rank_agreement_over_1000_seeds`, asserting its counts. Report, over the seeds whose ranks already
   agree: how many have the same order in every rank, and how many have the same total crossing
   count. Paste the exact commands and the counts in `docs/measurements/p13-gv2-dot.md` under a new
   "Mincross" heading. A disagreement is a finding with its seed, not a reason to stop.
5. `dot/rank_fixture_edges.rs` is 398 lines: split it so every file is at most 300.
6. Do not register `layout.dag.dot`: it needs the position pass. No `LAYOUTS` change.

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, no `HashMap`, no new
dependency. Every heuristic carries a `Ponytail:` line.

Paths allowed: `crates/graph-core/src/layout/graphviz/dot.rs`, `crates/graph-core/src/layout/graphviz/dot/**`,
`harness/oracle-dot-probe.py`, `docs/measurements/p13-gv2-dot.md`, `prompts/jobs/p13-gv2-dot-mincross.md`.
Not allowed: everything else, in particular the registry, `crates/graph-cli`, `crates/graph-wasm`,
`packages`, `app`, `server`, `deploy`.

Done when, each with its command and exit pasted: `scripts/orch/gate.sh target/gate-dot-mincross
scripts/orch/rows/quick.rows` all PASS; `scripts/orch/gr cargo test -p graph-core dot::` passes; the
step-4 counts are in the measurement file with their commands.
