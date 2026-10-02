> **Status (2026-10-02):** MERGED into develop — develop commit b980ae8 is branch p8's head; slices p8-p8-bundle, p8-route and p8-styles came with it. Closed: the bundling and styles ledger rows are on develop (crates/graph-cli/src/capabilities/post.rs:2,111 — post.bundle.fdeb, post.bundle.mingle, post.style.straight/orthogonal/bezier/quadratic beside post.route.grid). Still owed: docs/reports/phase-08.md. See docs/reports/STATUS.md.

# Phase 8 — The POST stage: edge styles, routing, bundling

**Read `prompt.md` and `prompts/REFERENCES.md` first.** Phase 7's gate must be green.

## Goal

Edge geometry as a stage of its own: style presets (orthogonal, bezier, quadratic), force-directed edge
bundling (FDEB), multilevel ink-minimisation bundling (MINGLE), and obstacle-avoiding grid routing.

## Why this is a separate stage — the reference is explicit about it

In SciGraphs, layout returns **positions only**, and every edge-path concern lives downstream in
`core/scigraphs_core/mesh/edge_styles.py` and `engine/scigraphs_engine/bundling/*`. That separation is
deliberate and it is the architecture we copied into `prompt.md` §3.

It also means every algorithm here is **composable with every layout**: bundling a Sugiyama diagram, or
orthogonally routing a force layout, requires no new code per combination. That is the property that makes
this a motor rather than a collection of renderers.

And it is where `.sgraphs` falls short as a format: it stores no edge path data at all, so bundling and
routing output there is **non-persistable** — a rendering-time effect that vanishes. Our `Polyline` and
`Curve` kinds exist so this stage's output survives serialization.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/post/{mod.rs,styles.rs,fdeb.rs,mingle.rs,routed.rs,grid_index.rs}
fixtures/post/{parallel-edges.json,hairball.json,obstacles.json,long-span.json}
docs/measurements/phase08-{ink,routing}.md
```

**MODIFY — exactly these:**
```
crates/graph-core/src/lib.rs
crates/graph-core/src/registry.rs
crates/graph-cli/src/{capabilities.rs,main.rs}
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Any layout. SBEB (out of scope). Anything in osionos.

## Reference material

All on disk:

| Capability | Reference | Paper |
|---|---|---|
| Edge style presets | `SciGraphs/core/scigraphs_core/mesh/edge_styles.py:11-40+` — `GEPHI_DEFAULT`/CURVED, `CYTOSCAPE_BEZIER`/QUADRATIC, `SCHEMATIC`/ORTHOGONAL, `BUNDLED_DENSE`, `FLOW_DIAGRAM`, `MINIMAL` | — |
| FDEB | `SciGraphs/engine/scigraphs_engine/bundling/fdeb.py` | Holten & van Wijk 2009 |
| MINGLE | `.../bundling/mingle.py` | Gansner et al. 2011 |
| Grid routing | `.../bundling/routed.py` | Lambert et al. 2010, "winding roads" |

## Steps

### 1. `styles.rs` — the cheap, exact half

Straight, orthogonal (L/Z elbows), quadratic and cubic bezier control-point generation, plus parallel-edge
separation (two edges between the same pair must not overlay — offset them deterministically by edge
index).

Self-loops are a real case and easy to forget: they need a documented shape, not a zero-length path.
`fixtures/post/parallel-edges.json` covers both.

Output `Polyline` for orthogonal, `Curve` for bezier/quadratic. Exact, deterministic, O(m). **No Ponytail
markers here** — this is arithmetic.

### 2. `grid_index.rs` — the shared spatial substrate

A uniform grid over the node geometry, marking occupied cells as obstacles. Reused by `routed.rs`, and
reusable by Phase 9's LOD. Built into a **reused buffer** (`dsa-and-memory.md`), deterministic cell
assignment, explicit tie-break for a point exactly on a cell boundary.

### 3. `routed.rs` — obstacle-avoiding routing

Shortest path over the grid with nodes as obstacles. This is the closest thing in the corpus to true
orthogonal routing-around-obstacles.

Use the Phase 7 path machinery over the grid graph — **do not write a third shortest-path implementation**
(`library-first.md`). The grid is a graph; our CSR and Dijkstra already handle graphs.

Determinism: equal-cost paths are common on a uniform grid, so the tie-break must be total and explicit
(by cell index). Without it this is non-reproducible by construction.

When no route exists (a node fully enclosed), fall back to the straight segment and **flag it in the
output**. Not a log line — a downstream program cannot read stderr.

### 4. `fdeb.rs` — force-directed edge bundling

Holten & van Wijk 2009: subdivide each edge into points, then iteratively attract subdivision points of
compatible edges. Compatibility is a product of angle, scale, position and visibility terms.

Cost is the thing to watch: naive FDEB is **O(m² · subdivisions · iterations)** on edge pairs. The
reference's compatibility threshold prunes most pairs — port that pruning, and declare the resulting
ceiling honestly. Use the Phase-8 grid index to limit candidate pairs spatially if the measurement
demands it, and record that as a deviation with its number.

Gather form (D10, `docs/decisions/compute-tiers.md`): each subdivision point computes its own
displacement from its compatible edges' points, reading start-of-iteration state and writing only
itself — so FDEB can later run on the SIMD, threaded and GPU tiers unchanged (FDEB is the other
GPU candidate). MINGLE's greedy merge is inherently sequential; keep it single-threaded and say so.

Determinism: fixed subdivision schedule, fixed accumulation order over compatible pairs (sort the pair
list by (edge index, edge index)), **no parallel reduction** (D3), no `mul_add` (D2).

`docs/measurements/phase08-ink.md` records the **ink reduction** achieved on `hairball.json` — the quality
claim for a bundler, and the number that says whether it did anything.

### 5. `mingle.rs` — multilevel ink minimisation

Gansner et al. 2011: hierarchical clustering of edges, merging bundles when the merge reduces total ink.
Cheaper than FDEB and a genuinely different trade-off, which is why both exist.

Deterministic cluster merge order; total tie-break on equal ink gain.

### 6. Composability is the gate, not a nice-to-have

Test the cross-product on a small matrix: each POST capability over at least three different layouts
(force, Sugiyama, tidy tree). A bundler that only works on force output is not a stage.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- roundtrip --seeds 1000           # 0

# Polyline/Curve CSR offsets are well-formed at every boundary
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core edge_geometry_invariants          # 0

# every POST capability works over every layout it claims to support
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core post_composability                # 0

# bundling actually reduces ink
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- ink --fixture hairball            # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                      # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

## Ledger delta

To `gated`: `post.style.{straight,orthogonal,bezier,quadratic}`, `post.route.grid`,
`post.bundle.fdeb`, `post.bundle.mingle`.

`post.bundle.fdeb` must carry a real `scale_ceiling` derived from the measurement, not a guess — it is the
most expensive capability in the project.

## Ponytail requirements

- **FDEB** is an approximation controlled by a compatibility threshold: name the failing input (edges just
  below threshold that a viewer would expect bundled), the direction (under-bundling — cosmetic), and the
  escape hatch (the threshold parameter). If spatial pruning was added, its error is a **second** marker.
- **MINGLE** is greedy: a non-optimal bundle hierarchy. Cosmetic.
- **Grid routing** resolution is a trade: a coarse grid misses narrow gaps and routes around them; a fine
  grid is quadratically more expensive. Name the failing input (a gap narrower than one cell) and the
  direction (**a route that detours or falls back to a straight line through a node — visually wrong, the
  dangerous direction**), plus the flag that reports it.
- **No marker** on the style generators — exact arithmetic.

## Stop-and-ask

- FDEB cannot meet a usable time budget even with the reference's pruning → stop and report the
  measurement before adding approximations beyond it.
- A POST capability cannot compose with a layout it should support → stop; that means the stage boundary
  leaked layout-specific assumptions, which is an architecture problem, not a bug to patch.
- You are about to write a third shortest-path implementation → stop and reuse Phase 7's.
