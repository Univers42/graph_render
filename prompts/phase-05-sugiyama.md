> **Status (2026-09-30):** MERGED into develop, merge 7788d85 (branch p5 head 4caf184). Still owed: mutants and docs/reports/phase-05.md. See prompts/RESUME.md.

# Phase 5 — Sugiyama layered DAG

**Read `prompt.md` and `prompts/REFERENCES.md` first.** Phase 4's gate must be green.

## Goal

The layered-DAG layout: cycle breaking, layer assignment, crossing reduction, coordinate assignment, and
**dummy vertices materialised as `Polyline` edge routes**.

## Why it is its own phase

Sugiyama is four algorithms in a pipeline, and the third one is **heuristic**. That makes it structurally
different from everything in Phase 3:

- it is the first layout whose output is *not* uniquely determined by the input — crossing reduction is a
  local-search heuristic, so "correct" means "deterministic and no worse than the reference", not
  "identical to the mathematically optimal answer";
- it is the first layout that **invents nodes** (dummy vertices for edges spanning more than one layer),
  and those dummies are what turn an edge into a multi-point route;
- it has a real **scale ceiling with a defined degradation**, which the reference documents honestly and
  we must too.

It also covers Graphviz's `dot` family, which we are explicitly not porting
(`prompts/REFERENCES.md`) — so this is the phase that discharges that gap.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/layout/sugiyama/{mod.rs,acyclic.rs,layering.rs,ordering.rs,coords.rs,routing.rs}
fixtures/dag/{chain.json,diamond.json,cyclic.json,multi-span.json,wide-layer.json,disconnected.json}
docs/decisions/sugiyama-heuristics.md
docs/measurements/phase05-crossings.md
```

**MODIFY — exactly these:**
```
crates/graph-core/src/layout/mod.rs
crates/graph-core/src/registry.rs
crates/graph-cli/src/capabilities.rs
harness/oracle-layouts.mjs           (add the dagre-d3-es arm)
package.json                         (add dagre-d3-es as a direct devDependency)
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Force/iterative layouts (Phase 6). Anything in osionos.

## Reference material

- **`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:638-691`** — the full pipeline, natively
  implemented. This is the primary reference and it is on disk. It names its own algorithm choices:
  greedy feedback-arc-set (Eades/Lin/Smyth 1993), longest-path layering, median + transpose crossing
  reduction, priority-method X-coordinates.
- **Its documented budgets** — `_DUMMY_BUDGET = 200_000` (`hierarchical.py:7`), beyond which long arcs
  are left **straight/unrouted and reported as such**; transpose rounds throttled to **0** above 150,000
  layered nodes (`:670-672`). These are not suggestions; they are the degradation contract.
- **Oracle:** `node_modules/dagre-d3-es/` (on disk via mermaid). Promote to a direct devDependency.

## Steps

### 1. `acyclic.rs` — cycle breaking

Greedy feedback-arc-set (Eades/Lin/Smyth 1993). Reversed edges must be **recorded**, so the final routes
can be flipped back to the original direction. An edge silently left reversed is a wrong diagram that
looks right.

Determinism: the greedy order must be a **total** order over dense indices, never traversal-incidental
(D5).

### 2. `layering.rs` — layer assignment

Longest-path layering, matching the reference. Then edges spanning more than one layer get **dummy
vertices**, one per intermediate layer.

Enforce `_DUMMY_BUDGET`. When exceeded: leave long arcs straight/unrouted, and **surface it in the
output** — a flag or a count in the snapshot, not only a log line. A downstream program cannot read your
stderr.

### 3. `ordering.rs` — crossing reduction (the heuristic)

Median heuristic then transpose, alternating up/down sweeps for a fixed iteration count. Two hard
requirements:

- **Deterministic tie-breaking.** Medians tie constantly. Break by dense index, never by hash order or
  iteration accident (D4/D5). This is where a naive port becomes non-reproducible.
- **Throttle at scale** per the reference: transpose rounds → 0 above 150k layered nodes. Declare it.

`docs/measurements/phase05-crossings.md` records the **actual crossing count** for our implementation
versus `dagre-d3-es` across the fixture set. The claim is "no worse by a stated margin", and it must be a
measured number.

### 4. `coords.rs` — X coordinates

Priority method, per the reference. Y is the layer index times a documented spacing constant.

### 5. `routing.rs` — dummies become `Polyline`

Each edge's route is its dummy chain, in layer order, as a `Polyline`. Edges within one layer-step get a
straight `Line`-equivalent (an empty interior point list). Reversed edges are un-reversed here.

Verify CSR offset correctness at the boundaries: first edge, last edge, an edge with zero interior
points, and an edge with the maximum span in the fixture set.

### 6. Fixtures

`chain` (trivially layered) · `diamond` (a real crossing decision) · `cyclic` (exercises FAS) ·
`multi-span` (exercises dummies) · `wide-layer` (exercises ordering at width) · `disconnected` (several
components — decide and document placement, reusing Phase 3's `hierarchy.rs` component spacing rather
than inventing a second convention).

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

# structural invariants: acyclic after FAS, layers monotonic, dummy chains contiguous
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core sugiyama_invariants               # 0

# crossings measured against dagre, not asserted
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/oracle-layouts.mjs --dag           # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                      # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

## Ledger delta

`layout.dag.sugiyama` → `gated`. Required fields, and they are substantive here:

- `geometry`: `Point` + `Polyline`
- `complexity`: `O(n+m)` per phase; **crossing reduction is heuristic** — say so in the field, do not
  imply optimality
- `scale_ceiling`: `200000` (dummy budget)
- `degradation`: long arcs left unrouted above the dummy budget; transpose disabled above 150k layered
  nodes

## Ponytail requirements

- **Crossing reduction is a heuristic, not a minimiser.** Name the failing input (a graph whose optimal
  ordering the median/transpose local search cannot reach) and the direction (more crossings than
  optimal — cosmetic, never incorrect).
- **The dummy budget** degrades to unrouted arcs. Name the failing input (a graph with very long spans),
  the direction (routes become straight lines that may pass through nodes — **visually wrong, the
  dangerous direction**), and the escape hatch (read the budget-exceeded flag in the snapshot).
- **FAS is greedy**, not minimum. Name it; more reversed edges than necessary is cosmetic.
- **No marker** on layering or coordinate assignment if they are exact.

## Stop-and-ask

- Our crossing count is materially worse than `dagre-d3-es` on a fixture → stop and report the number.
  Do not tune constants until the gate passes; a tuned-to-pass heuristic is not verified, and the
  measurement file is the deliverable.
- A tie-break has no obviously deterministic resolution → stop. Guessing bakes an arbitrary choice into a
  hash permanently.
- Disconnected-component placement conflicts with Phase 3's convention → stop; one convention, not two.
