> **Status (2026-09-30):** MERGED into develop — branch head 2538f0a is an ancestor of develop; p3 landed inside p6f, merge 45a653f. Its 1000-seed gate and mutants run as part of the develop gate. See prompts/RESUME.md.

# Phase 3 — The deterministic one-shot layouts

**Read `prompt.md` and `prompts/REFERENCES.md` first.** Phase 2's gate must be green.

## Goal

Four real layouts, all deterministic and one-shot: **tidy tree**, **squarified treemap**,
**circular/radial**, and **circle packing**. Between them they exercise the hierarchy CSR, and both the
`Box` and `Circle` geometry kinds.

## Why these four, in this phase

They are the layouts that **force the contract to be real** while staying fully deterministic. A
one-shot layout has no cooling schedule, no alpha, no random restarts — so if a hash diverges across
targets, the cause is arithmetic, not chaos. Force-directed layouts amplify a 1-ULP difference into a
visibly different picture; debugging the transport and the gate against *that* is far harder. Hence:
prove the machinery on the calm cases, then let Phase 6 bring the storm.

Each one also earns its place by exercising something no other layout does:

| Layout | Forces into existence |
|---|---|
| Tidy tree | the **hierarchy CSR** (built in Phase 1, unused until now) + `Polyline` edges |
| Squarified treemap | **`Box`** geometry |
| Circular / radial | BFS-depth ranking; concentric placement |
| Circle packing | **`Circle`** geometry — *the case SciGraphs' own contract could not hold* |

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/layout/{tidy_tree.rs,treemap.rs,circular.rs,circle_packing.rs}
crates/graph-core/src/layout/hierarchy.rs      (shared: root detection, BFS depth, forest handling)
harness/oracle-layouts.mjs                     (the d3-hierarchy / dagre arms)
fixtures/hierarchy/{tree-balanced.json,tree-degenerate.json,forest.json,cyclic.json}
docs/decisions/planarity-fallback.md
```

**MODIFY — exactly these:**
```
crates/graph-core/src/layout/mod.rs
crates/graph-core/src/registry.rs
crates/graph-cli/src/{main.rs,capabilities.rs}
package.json                                   (add d3-hierarchy as a direct devDependency)
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Force/iterative layouts (Phase 6). Sugiyama (Phase 5).
Anything in osionos.

## Reference material

`prompts/REFERENCES.md` is authoritative on what is on disk. For this phase, **everything you need is**:

- `node_modules/d3-hierarchy/` — `tree.js` (Reingold–Tilford) and `treemap/squarify.js`. On disk via
  mermaid's transitive dep, and **these are the oracles**.
- `SciGraphs/core/scigraphs_core/mesh/layouts/circle_packing.py:281-393` — Collins–Stephenson, natively
  implemented. Its non-planar fallback is `:407-542`.
- `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py` — the circular-hierarchy section.

Promote `d3-hierarchy` to a **direct devDependency** — oracle code, never shipped.

## Steps

### 1. `hierarchy.rs` — the shared substrate

Root detection, BFS depth, and **forest handling** (a graph with several roots, or none). Real data is
not a clean single-rooted tree, and every one of the four layouts needs the same answers:

- multiple roots → lay out each component, then place components (SciGraphs uses
  `_COMPONENT_SPACING = 2.5`; adopt a documented constant, do not improvise per-layout)
- **no root** (a cycle) → this is a decision, not an error to swallow. Options: refuse, or break the
  cycle deterministically and record it. Pick one, write it in `docs/decisions/`, and make the choice
  visible in the output so a consumer knows it happened.
- a node with several parents → the hierarchy CSR permits it; a tree layout does not. Same treatment:
  decide, document, surface.

`fixtures/hierarchy/cyclic.json` exists to pin this behaviour with a test.

### 2. Tidy tree (Reingold–Tilford)

O(n), two passes. Differential against `d3-hierarchy`'s `tree()`.

**d3 normalizes to a unit square by default** — match its convention exactly or record the deviation
explicitly, because a differential that fails only on scale is indistinguishable from one that fails on
structure, and you will waste a day.

Emit `Polyline` edges (parent→child with the elbow bend), not `Line`. This is the first real exercise of
`Polyline`, so verify the CSR offsets are right at the boundaries: first edge, last edge, and an edge
with zero interior points.

### 3. Squarified treemap

O(n log n) — sort children by descending area, then the squarify row-packing. Differential against
`d3-hierarchy`'s `treemap().tile(treemapSquarify)`.

Node **area** comes from a declared `weight` role. Missing or non-positive weights are the real-world
case: clamp to a small epsilon and **name it in the Ponytail** — a zero-area subtree collapses to a
hairline rather than vanishing, and a consumer needs to know which it got.

This is the first `Box` geometry. Assert the invariant that children's boxes are contained in the
parent's and do not overlap, over the whole seed sweep. Squarify is numerically fiddly at extreme aspect
ratios and this invariant catches it.

### 4. Circular / radial

Concentric rings by BFS depth. Cheap, O(n). The decisions to record: angular ordering within a ring
(**must be deterministic** — index order, not traversal-incidental), radius progression (linear or
sqrt), and start angle.

### 5. Circle packing — the one that justifies the `Circle` kind

Collins–Stephenson. Reference `circle_packing.py:281-393`.

**It is exact only on planar graphs.** SciGraphs is explicit: non-planar input silently substitutes an
approximate force-relaxation packer that is **not** guaranteed tangent or non-overlapping
(`circle_packing.py:308-311`, `:389-391`), and it says so in its log.

Do better than a log line: the fallback must be **visible in the output** — a flag in the snapshot or a
distinct capability id — so a consumer can tell an exact packing from an approximation. A log message is
invisible to a downstream program. Write `docs/decisions/planarity-fallback.md` with the choice.

Planarity testing itself is non-trivial. If a correct test is out of reach this phase, **stop and ask**
rather than shipping a wrong one; an incorrect planarity test that reports "planar" for a non-planar
graph silently produces overlapping circles, and that is the dangerous direction.

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

# the layout oracles
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- emit-fixtures --seeds 1000       # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/oracle-layouts.mjs                 # 0

# geometry invariants (treemap containment/non-overlap, polyline offset bounds)
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core geometry_invariants               # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

## Ledger delta

To `gated`: `layout.tree.tidy` (Point+Polyline, O(n), oracle `d3-hierarchy@3.1.2`),
`layout.treemap.squarified` (Box, O(n log n), same oracle), `layout.circular.radial` (Point, O(n)),
`layout.packing.circle` (**Circle**, oracle `hand` + planarity check).

`layout.packing.circle` **must** declare its degradation as the non-planar approximate fallback. If the
fallback ships as a separate capability id, register both.

## Ponytail requirements

- **Treemap**: non-positive areas clamped to epsilon → a zero-area subtree becomes a hairline, not
  nothing. Direction: cosmetic under-representation, never a wrong containment.
- **Circle packing**: exact only for planar input; the fallback is not guaranteed tangent or
  non-overlapping. Name the failing input (any K₅ or K₃,₃ minor) and the direction (**overlap, which is
  the dangerous direction**), plus the escape hatch (read the fallback flag in the snapshot).
- **Circular**: radius progression is a convention; dense rings crowd at high depth.
- **Tidy tree**: none if the port is exact — say so plainly rather than inventing a caveat.

## Stop-and-ask

- A layout differential fails only by a uniform scale or translation → that is probably d3's
  normalization convention. Investigate before declaring divergence; **do not** add a tolerance to make
  it pass.
- You cannot implement a correct planarity test → **stop**. Shipping a wrong one produces silently
  overlapping circles.
- A hierarchy fixture (cyclic, multi-parent, forest) has no obviously right answer → stop and ask. These
  are product decisions, and guessing bakes an arbitrary choice into a hash forever.
