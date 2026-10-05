# `layout.dag.lanes`: one row per vertex, reused lanes (piece 2 of 3)

Status: draft, 2026-10-05. Direction approved by the user on 2026-10-05 ("B and A"), and the
user granted autonomy on the same day. Piece 1 and the probe numbers are in
`2026-10-05-git-history-source-design.md`.

This layout adds a public capability (a registry entry, a capabilities row, server caps), so a
devil risk verdict is required before any code.

**The motor stays unaware of every data source.** User rule, 2026-10-05: the engine gives
generic tools and never names another project. Nothing under `crates/` names git, a
repository or a test data set. Only the measurement doc names its inputs, because a
measurement has to.

## Why

`layout.dag.sugiyama` draws an 85,928-vertex version history in 171 ms, but only because it
degrades: 12,035 long arcs (11%) are left straight through vertices once the 200,000-vertex
dummy budget is spent. Long-lived lines of development are exactly what produce long arcs, so
the budget fails on real histories. A history drawing needs no dummy vertices:
- one row per vertex;
- each line keeps a column (a lane), and a lane is reused once it is free;
- every edge is a short polyline.

That costs O((n + m) log n) and never leaves an edge unrouted.

## Reference

The reference design is the history graph of version-control tools, `git log --graph` among
them: rows in topological order, columns reused, the first parent kept straight. git's
`graph.c` is GPL-2.0, so it is **not ported**. The algorithm below is stated independently.
git's output serves as a quality measurement in the measurement doc (lane width), not as a
byte oracle. The byte oracle is a hand oracle, as for `layout.dag.sugiyama`, grid, circular and
packing (`ROUNDTRIP_LAYOUTS`, `crates/graph-cli/src/capabilities/registry/layout_row.rs`).

## Algorithm

**Input.** Any `Topology`: the hash gate runs every layout on its seeded model. Rows do **not**
come from admission order. The graph hub materializes nodes in qualified-id order (hub spec
§5.3), and a layout that trusted admission order would draw the same history differently
depending on how it arrived.

1. **Arcs and rows.**
   - Every directed non-loop edge is an arc `source → target`. Undirected edges and self-loops
     impose no order.
   - Rows are a topological order of the arcs (Kahn). The ready vertices wait in a heap keyed by
     `version` descending (`f64::total_cmp`), then dense index ascending. For a history whose
     arcs point from newer to older records, the newest ready record therefore comes first.
   - When the heap is empty and vertices remain, the arcs have a cycle. The unplaced vertex with
     the lowest dense index is placed next, found by a forward pointer scan (amortized O(n)).
   - `r(v)` is `v`'s row.
2. **Orientation.**
   - Every non-loop edge is oriented from its earlier row to its later row.
   - A directed edge whose source has the later row is drawn head to tail, and carries a
     `dag.edge_reversed` note (code 5), exactly as Sugiyama notes one.
   - A vertex's forward edges are taken in edge admission order (dense edge index), so a source
     that writes a record's first parent first keeps that line straight.
   - Building the per-vertex forward lists is a counting sort by the earlier endpoint's row:
     O(n + m).
3. **Lanes.** Two structures:
   - `free`: a min-heap of released lanes.
   - `reserved[v]`: the lanes edges are carrying down towards `v`.

   For each vertex `v` in row order:
   1. `lane(v)` is the smallest lane in `reserved[v]`. If `reserved[v]` is empty, it is the
      smallest lane in `free`, or a new lane `width` (then `width += 1`). Every other lane in
      `reserved[v]` is released to `free`, because its edge ends at `v`.
   2. For each forward edge `v → p`, in the order of step 2:
      - **The first edge** pushes `lane(v)` into `reserved[p]`: the first-parent line continues
        straight.
      - **A later edge**, when `reserved[p]` is non-empty, shares its smallest lane: two lines
        into one vertex converge. When `reserved[p]` is empty, the edge takes the
        smallest free lane (or a new one) and pushes it into `reserved[p]`.
   3. If `v` has no forward edge, `lane(v)` is released after row `v`.
4. **Geometry.**
   - **Nodes:** `Point` at `x = lane(v) · lane_spacing`, `y = r(v) · row_spacing`.
   - **Edges:** a `Polyline` from `v` (lane `a`) to `p` (lane `b`), carried in lane `l`, through
     `(a, r(v))`, then `(l, r(v) + ½)` if `l ≠ a`, then `(l, r(p) − ½)` if `l ≠ b`, then
     `(b, r(p))`. That is at most two interior points.
   - **Self-loop:** no interior points, the same convention as Sugiyama.
   - **Reversed edge:** drawn head to tail, with its note.

**Why no edge crosses a node.** A lane in some `reserved[p]` is not in `free` until row
`r(p)`. A vertex placed at a row strictly between `r(v)` and `r(p)` therefore never takes lane
`l`.

**Determinism.**
- Integer lanes and integer rows. The only float operation is one multiply per coordinate.
- No hash container: `reserved` is a `Vec` indexed by dense node, and `free` is a
  `BinaryHeap<Reverse<u32>>`, whose pop order is total over `u32`.
- Wire integers are `u32`.

D1–D10 hold by construction. The hash gate checks them.

**Cost.** O((n + m) log n) time (the ready heap and the lane heap) and O(n + m) memory. There are no dummy vertices and no
crossing reduction, so neither phase needs a budget.

## Parameters

| name | type | default | rule |
|---|---|---|---|
| `lane_spacing` | `f32` | `1.0` | finite and above 0 |
| `row_spacing` | `f32` | `1.0` | finite and above 0 |

They are published through `published!` like `SugiyamaParams`, so the studio's parameter panel
lists them without a studio change.

## Registry metadata

| field | value |
|---|---|
| `id` | `layout.dag.lanes` |
| `tier` / `stage` | 1 / `layout` |
| `nodes` / `edges` | `Point` / `Polyline` |
| `oracle` | the hand oracle `roundtrip` (below); lane width measured in `docs/measurements/dag-lanes.md` (no tool or data set is named under `crates/`) |
| `complexity` | O((n + m) log n) time, O(n + m) memory |
| `scale_ceiling` | the largest n measured within the speed targets below, written as measured |
| `degradation` | none in time or routing. Width grows to n on an antichain: the seeded model's random graph draws wide, which is legal and reported here |
| `ponytail` | `Ponytail (row tie-break)`: ready vertices go newest `version` first, then lowest dense index. That is a convention, not a crossing minimiser. Failing input: two independent lines with equal `version`s interleave by index. Direction: more lane switches, which is cosmetic. Cycles are broken by placing the lowest-index unplaced vertex, and each reversed edge is noted (code 5). `Ponytail (lane choice)`: lowest-free-lane is greedy, and minimal width is not claimed. Failing input: two branches whose lanes could interleave narrower. Direction: wider drawing, which is cosmetic. Escape hatch: none needed, every edge is still routed |

## Hand oracle (`snapshot_cmd::hand_oracles::lanes`, gated on `roundtrip`)

An independent restatement, compared bit for bit. It uses ordered sets where the layout uses
heaps and intrusive lists, so the two share the convention and not the code:
- the ready set is a `BTreeSet` keyed by the integer image of `f64::total_cmp`, reversed, then
  the index;
- the free lanes are a `BTreeSet<u32>`;
- `reserved` is a `BTreeMap<u32, BTreeSet<u32>>`.

The restatement must produce the motor's exact `x`, `y` and polyline columns per seed. Its
invariant checks (unit tests with a perturbed snapshot each, as `snapshot_cmd/dag.rs` does)
must fail on:
- a node off its row, or two rows out of topological order;
- an edge's interior run sharing a lane with a node strictly between its endpoints;
- a note 5 on an edge that is not reversed.

## Gates (exit codes in the phase report)

| gate | expect |
|---|---|
| fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, wasm32 build | 0 |
| `hashgate --seeds 8`, and its `GM_MUTATE_REFERENCE_DEGREE=9` negative control | 0, non-zero |
| `roundtrip --seeds 100` (the hand oracle), `capabilities --check`, `codegen --check` | 0 |
| `scripts/orch/rows/svc-floor.rows`: a caps row and digest rows | 0, with the `server/` diff sent to its owner (graph-render-4f) first |
| studio picker | lists `layout.dag.lanes` with no studio change (`Motor.layouts()`) |

## Measurement (`docs/measurements/dag-lanes.md`)

- **Commit DAGs.** Children-first `git log --all --topo-order` exports of:
  - contributor-stats, activitywatch and aw-server-rust (ActivityWatch);
  - graph_render;
  - git/git, 85,928 commits.
- **Synthetic.** A seeded 1M-commit synthetic: a first-parent chain with branches and merges,
  from `graph-cli`.
- **Per input:** native and wasm32 time (three rounds, alternated, medians, load printed); lane
  width; git's width (`git log --graph --format=%H`, the column of `*` halved, maximum over
  rows).
- **Targets:**
  - git/git ≤ 50 ms in wasm32;
  - 1M synthetic ≤ 1 s in wasm32;
  - every edge routed (zero note 4 by construction).

  A missed target is reported with its number, not rounded into a pass.

> Caveat: the host is shared, so medians under load are upper bounds.

## What it does not do

- No crossing reduction and no compaction across lanes. The width is greedy, not minimal.
- No time-proportional y. Rows are topological ranks, as in `git log --graph`. A date axis is a
  piece 3 question.
- No layered drawing of non-git DAGs better than Sugiyama's. It accepts any graph, but it is
  built for histories.

## Files

| new | changed (additively) |
|---|---|
| `crates/graph-core/src/layout/lanes.rs` (+ `lanes/` children if past 300 lines) | `layout/mod.rs`, `registry/layouts.rs`, `registry/params.rs` |
| a metadata constant beside `SUGIYAMA` (or `registry/lanes.rs` if `grid.rs` passes 300 lines) | `crates/graph-cli/src/capabilities/registry/layout_row.rs` (`ROUNDTRIP_LAYOUTS`) |
| `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs` | `snapshot_cmd/hand_oracles.rs` |
| `docs/measurements/dag-lanes.md` | generated schema and TypeScript declarations (`codegen`), `server/` caps and digests (4f) |

House limits apply: ≤ 40 lines a function, ≤ 4 parameters, ≤ 300 lines a file, nesting ≤ 3, no
`unsafe`, and no new dependency (`std::collections` only).
