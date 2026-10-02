# Job ux-overlap (agent build: nodes never sit on each other after a 2D layout)

Why (user, 2026-10-03): "sometimes there are too many nodes all compressed in the same space,
it's unreadable". Most layouts place points, not discs. The motor has no node-overlap removal
anywhere: POST is edge-only by contract (`crates/graph-core/src/post/mod.rs:1-18`).

Facts (confirm before editing):
- The pipeline is INGEST → TOPOLOGY → ANALYSIS → LAYOUT → POST → SCALE → GEOMETRY. Each stage is
  pure, optional, hashed on its own (`crates/graph-core/src/stage.rs`).
- Node shapes are `Point`, `Circle` and `Box`, declared once per snapshot (`docs/contract/`).
- The live force already has a collide term, `collide_radius`
  (`layout/force/params.rs`). It is not a pass over a finished layout.
- Reference, read for behaviour only: Graphviz 16.1.0 at `$GM_SCRATCH/refs/graphviz-16.1.0`. It is
  EPL-1.0, so never copy text. Look at `lib/neatogen/overlap.c` (PRISM) and
  `lib/neatogen/adjust.c`. The paper is Gansner and Hu, "Efficient, Proximity-Preserving Node
  Overlap Removal", JGAA 14(1) 2010.
- The current oracle image has no triangulation library. In it, `-Goverlap=prism` runs no code
  (`harness/gv_plain.py:51-60`, measured).

Do, in order:
1. Write `docs/decisions/node-overlap.md`. Its first two points can stop the job; the rest are
   choices the doc records.
   - Placement: a new optional stage after LAYOUT, or a node-moving POST under a revised contract.
     Each choice keeps per-stage hashing.
   - Node sizes in layout units: where they come from before SCALE, and what a `Point` counts as.
   - The algorithm: a deterministic O(n·k) pass, PRISM-like or uniform-grid sweeps. No O(n²) on
     the user path; the target is 1 000 000 nodes (D1 to D10, `prompt.md` §6).
   - Opt-in: off by default, so today's hashes do not move.
   - The parameters: margin and the iteration cap.

   Route the decision through the `devil` agent; it is a contract change. Record the verdict. On
   BLOCK, stop and return.
2. Implement the pass with no new dependency. Use `libm`, fixed-order reductions and `IndexMap`
   only.
3. Exact invariant test: after the pass, a brute-force pair check finds no two nodes overlapping
   by more than the tolerance. The brute-force check is a test instrument, not product, and runs
   at 2 000 nodes at most. Cover these inputs:
   - every fixture under `fixtures/`
   - 100 seeds of random graphs
   - an adversarial case: all nodes on one point, and all on one line
4. Quality: the mean displacement, and the stress ratio before and after (layout distances against
   graph distances). Record them at 1 000, 10 000 and 100 000 nodes, with the time taken. Write
   them to `docs/measurements/ux-overlap.md`.
5. Oracle for quality: add the triangulation library to `docker/graphviz-oracle.Dockerfile` and
   check that `-Goverlap=prism` now changes neato's output. Then add a quality differential: our
   displacement and stress next to Graphviz's on the same input. It is a ceiling, not a bitwise
   match.

   If the library cannot be built from a pinned source, report that row as "not run" and say why.
   That is a stop for the row only, not for the job.
6. Wire-up:
   - a registry row with every `Metadata` field, including `ponytail`;
   - a hashgate entry;
   - a `GM_MUTATE_OVERLAP_*` knob that turns the invariant row red, added to the knob list in
     `crates/graph-cli/tests/common/mod.rs`;
   - wasm and SDK exposure, additive.

Paths:
- `crates/graph-core/src/**` (a new module)
- `crates/graph-wasm/src/**`
- `crates/graph-sdk-js/src/**`
- `crates/graph-cli/**`
- `crates/graph-contract/**` (only if the decision needs it)
- `docker/graphviz-oracle.Dockerfile`, and `harness/` (one new script)
- `docs/decisions/node-overlap.md`, `docs/measurements/ux-overlap.md`

Out of bounds: `packages/` and `app/`; the studio toggle is a follow-up job. Do not change the
behaviour of any existing layout or post. Edit the shared files (`lib.rs`, `registry.rs`,
`capabilities.rs`, `Cargo.lock`, `canonical_json/schema.rs`) additively only.

Done when:
- `scripts/orch/rows/ux-abi.rows` is green.
- The invariant row and its negative control behave: green with the pass, red with the knob.
- The measurement doc has the 1k, 10k and 100k table.
- The decision doc carries the verdict.
