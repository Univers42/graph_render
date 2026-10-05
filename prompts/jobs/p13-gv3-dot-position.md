# Job p13-gv3-dot-position (agent build, branch p13-gv3-dot-position, worktree ~/goinfre/wt/p13-gv3-dot-position)

Context: the dot rank pass and the mincross pass are on this branch (it starts from
`p13-gv2-dot-mincross`). Read `docs/measurements/p13-gv2-dot.md` in full first. Its "Blocker 1"
(node width), item 5 ("position.rs") and item 6 ("The frame") of "The remaining passes, as a
draft", and "The six closed cases, as the oracle prints them" are this job's specification. Also
read `docs/decisions/graphviz-oracle.md` §"Node width: one table, measured not derived". The width
table it names is `crates/graph-core/src/layout/graphviz/text_width.rs` (`text_width`,
`node_width`), and it is the only source of node widths you may use.

Licence, hard rule: Graphviz is EPL-1.0. Read
`~/goinfre/refs/graphviz-16.1.0/graphviz-16.1.0/lib/dotgen/position.c` for its behaviour only. Never
copy its text or comments, and never use its identifiers beyond the function names the measurement
file already uses. Never translate it line by line. Before you write each step, describe it in your
own words in a doc comment.

Exact tasks:
1. The position pass, as child modules of `crates/graph-core/src/layout/graphviz/dot/` (for example
   `position.rs` and `position/{ycoords,aux,xcoords}.rs`). It has these steps:
   - `set_ycoords`: rank heights and `ranksep`.
   - The auxiliary graph: `make_LR_constraints`, where the edge between two neighbours on a rank has
     length `rw(u) + lw(v) + nodesep`, and every real node's width is
     `node_width(text_width(<node id>))`. Then `make_edge_pairs`.
   - A second run of the existing simplex (`dot/simplex.rs`) with the LR balance. It is a call
     through `simplex::Params`, not a copy.
   - `set_xcoords`, then removal of the auxiliary graph.
   - The frame: translate so the drawing's lower-left node-box corner is the origin, with rank 0 at
     the largest y.

   Integer arithmetic where the reference uses it. The determinism rules D1-D10 in `prompt.md` §6
   apply.
2. Register `layout.dag.dot` (the family convention: `layout.dag.sugiyama`, `layout.force.neato`).
   APPEND one entry at the END of `LAYOUTS` in `crates/graph-core/src/registry/layouts.rs`. Never
   insert or reorder. Give it the full `Metadata`: oracle, complexity, scale_ceiling, degradation and
   ponytail. The ponytail names what is not ported (same-rank edge precedence, ports, clusters,
   splines) and the measured order disagreements.

   Follow how `layout.packing.osage` is registered: `git grep -n 'packing.osage' -- crates`. Count
   literals and id lists in tests that fail only because a layout was appended are regenerated from
   the test output, never guessed.
3. Tests:
   - the six closed cases, byte for byte against the table in the measurement file (points, in the
     `-Tplain` frame);
   - one closed-case test per step;
   - the 20 fixture seeds.

   Extend `harness/oracle-dot-probe.py` additively so its digest also carries each node's printed
   x and y. Keep every existing column, so `oracle_probe.rs`'s rank and order sweeps still read it.
4. The 1000-seed sweep. Add an `#[ignore]`d test `position_agreement_over_1000_seeds` beside the
   two existing sweeps, asserting its counts. Over the seeds whose order already agrees in every
   rank, report how many have every node centre equal to the oracle's at the plain format's printed
   precision (five significant digits, in inches). Paste the exact commands and the counts under a
   new "Position" heading in the measurement file. A disagreement is a finding with its seed, not a
   reason to stop. Also retitle the file's first line to `layout.dag.dot`.
5. Do not wire the graph-cli differential (`oracle_python/dot.rs`) or a hashgate knob: those are
   the next job. The layout's ledger status stays whatever `capabilities` derives without them.

Limits:
- Files at most 300 lines.
- Functions at most 40 lines and 4 parameters.
- No `HashMap`. No new dependency.
- Every heuristic carries a `Ponytail:` line.

Paths allowed:
- `crates/graph-core/src/layout/graphviz/dot.rs`
- `crates/graph-core/src/layout/graphviz/dot/**`
- `crates/graph-core/src/registry/**` (append-only for `LAYOUTS`)
- the count-literal and id-list test files that fail only because of the append, wherever they are
  (`crates/graph-cli/**`, `crates/graph-core/tests/**`), changed only at the failing literal
- `harness/oracle-dot-probe.py`
- `docs/measurements/p13-gv2-dot.md`
- `prompts/jobs/p13-gv3-dot-position.md`

Not allowed: everything else, in particular `crates/graph-wasm`, `packages`, `app`, `server` and
`deploy`, and `crates/graph-core/src/layout/graphviz/text_width.rs` (read only).

Done when, each with its command and exit pasted:
- `scripts/orch/gate.sh target/gate-dot-position scripts/orch/rows/quick.rows` all PASS;
- `scripts/orch/gr cargo test -p graph-core dot::` passes;
- `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` exits 0;
- the step-4 counts are in the measurement file with their commands.
