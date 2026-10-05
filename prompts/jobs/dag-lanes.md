# Job dag-lanes (agent build: the generic history layout `layout.dag.lanes`)

Why: `layout.dag.sugiyama` draws an 85,928-vertex history in 171 ms only by leaving 12,035 long
arcs (11%) unrouted past its dummy budget. `layout.dag.lanes` routes every edge in
O((n + m) log n):
- one row per vertex;
- reused lanes;
- at most two interior points per edge.

It is a generic motor layout. Nothing under `crates/` may name a data source (user rule,
2026-10-05).

Your instructions are the plan: `docs/superpowers/plans/2026-10-05-dag-lanes-layout.md`. Execute
Tasks 1 to 4 in order, step by step, with the code it gives. The design is in
`docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`.

Before Task 1, read `docs/decisions/dag-lanes.md`, the devil's verdict:
- BLOCK: stop and return `status: blocked` with its text.
- PROCEED-WITH-CONDITIONS: every numbered condition is an extra acceptance criterion. Meet each
  one and say how in the return block.

Facts (develop, 2026-10-05; re-check each on your branch before editing, and stop if one no
longer holds):
- `crates/graph-core/src/layout/mod.rs:31` has `pub mod sugiyama;`, and `crates/graph-core/src/lib.rs:61`
  has `pub use layout::sugiyama::{Sugiyama, SugiyamaParams};`.
- `crates/graph-core/src/layout/mod.rs:51-68`: `Geometry { nodes, edges, notes }` and
  `Geometry::planar(nodes, edges, notes)`.
- `crates/graph-core/src/columns.rs`:
  - `NodeColumns.version: Vec<f64>` (`:62`);
  - `EdgeColumns.source`, `.target: Vec<u32>` (`:125-127`);
  - `EdgeColumns.directed: Vec<bool>` (`:135`).
- `crates/graph-core/src/records.rs:182` `build::{node, edge}`: test-only builders. `edge` is
  undirected, so a directed one needs `EdgeRecord { directed: true, ..edge(..) }`.
- `crates/graph-contract/src/notes.rs:124-129` `Note { code: NoteCode, index: u32 }`;
  `NoteCode::EdgeReversed = 5`.
- `crates/graph-core/src/registry/tunable.rs:196` is the `SugiyamaParams` `tunable!` block, and
  `registry/params.rs` has `published!(SUGIYAMA, Sugiyama);`.
- `crates/graph-core/src/registry/three_d/random3d.rs:99` is the self-contained `Capability`
  const pattern that `registry/lanes.rs` follows.
- `crates/graph-cli/src/capabilities/registry/layout_row.rs:48` is `ROUNDTRIP_LAYOUTS`.

Rules (beyond `scripts/orch/common.md`):
- Every cargo call is `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo ...`.
- Never take `~/goinfre/orch/timed.lock`, and never run `hashgate --seeds 1000` or `mutants.sh`.
- The bench in Task 4 step 2 runs only under `flock ~/goinfre/orch/bench.lock`, with the
  preconditions the plan gives.
- Do not edit `server/`, `docs/measurements/service-caps.tsv`, `packages/`, `app/` or `src/`.
  The svc-floor rows that go red are reported, not fixed.
- A test that pins a layout count or id list gets exactly the new id or count, and nothing else
  in it changes.

Paths you may touch: those in the plan's Global Constraints, plus
`scripts/orch/rows/dag-lanes.rows` (already on develop; do not edit it).

Done when:
- `scripts/orch/gate.sh target/rows-dag-lanes scripts/orch/rows/dag-lanes.rows` writes a
  `summary.txt` with every row PASS;
- `docs/measurements/dag-lanes.md` holds the gate table, the bench table and the Caveat line.

Return: the plan's return block, filled.
