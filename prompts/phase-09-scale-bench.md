# Phase 9 — The SCALE stage, and the benchmarks that justify the whole project

**Read `prompt.md` first.** Phase 8's gate must be green.

## Goal

LOD, simplification, adaptive iteration budgets — and the measurement campaign that converts *"Python is
too slow"* from a premise into a number.

## Why this phase carries the project's justification

Every earlier phase gated on **correctness**. This one gates on the claim that started the project: that a
Rust motor makes real-time graph iteration possible where Python did not. Until Phase 9 that is an
assumption. `minimalism-ladder.md`'s performance override is explicit — the ladder loses to a measurement
and **only** to a measurement — so an unmeasured speed claim is not a weaker result, it is no result.

Two honesty requirements, both of which must survive into the report:

1. **d3's `forceManyBody` already uses a quadtree.** Our Barnes-Hut is a constant-factor win (SoA memory,
   no GC, no JS object overhead), not an asymptotic one. Do not let an O-notation argument stand in for the
   number.
2. **At N = 220 — today's real graph size — WASM may well be slower than the JS it replaces.** Boundary
   crossing and module init dominate at small N; the win appears at scale. A campaign that measures only
   100k will report a triumph and hide a regression for every actual user. So N = 220 is measured **first**
   and reported even when unflattering.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/scale/{mod.rs,lod.rs,simplify.rs,adaptive.rs}
crates/graph-cli/src/bench.rs
fixtures/scale/{n220.json,n10k.json,n100k.json,n1m.json}      (generated, seeded, committed as generators)
docs/measurements/phase09-{bench,lod,ceilings}.md
BENCHMARKS.md                                                  (the published summary)
```

**MODIFY — exactly these:**
```
crates/graph-core/src/lib.rs
crates/graph-core/src/registry.rs
crates/graph-cli/src/{capabilities.rs,main.rs}
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Any new layout or analysis. Anything in osionos.

## Reference material

- `SciGraphs/engine/scigraphs_engine/{lod.py,simplify.py,adaptive.py}` — the reference implementations.
- **The reference's own published ceilings**, which are our comparison baseline: raw mesh topology holds
  >60 FPS to ~200k nodes and >20 FPS at 10⁶; after Geometry-Nodes setup, interactivity holds to ~500k
  (`SciGraphs/docs/guide/architecture.qmd:48-51`, `README.md:97`). Its UI warns above 1,000/10,000 nodes
  for Spring layouts (`docs/panels/scigraphs/layout.qmd:7-10`).
- The existing TypeScript baseline: `osionos/scripts/graph-bench.mjs` targets ≥30 FPS at 10k. Read it for
  methodology; **do not modify it** (osionos is read-only).

## Steps

### 1. Fixtures as generators, not blobs

Commit the **generator and its seed**, not a 100 MB JSON. `n1m.json` as a literal file is absurd; a seeded
deterministic generator is reproducible and free. Record the generator's exact constants — a benchmark
whose input cannot be reproduced is an anecdote.

### 2. `lod.rs` — level of detail

Given a viewport and a node count, decide what is drawn: node culling, label suppression thresholds, edge
decimation, cluster collapse. Output is **hints as columns** (a `visible: Vec<u8>` mask, a `lod_tier`), not
a mutated topology. The motor never destroys data to make rendering cheaper — a front may ignore the hints
entirely.

Reuse Phase 8's `grid_index.rs` for spatial queries. Do not build a second spatial structure.

### 3. `simplify.rs` — topology reduction

Edge/node reduction that preserves structure: degree-1 leaf folding, chain contraction, community-level
collapse (reusing Phase 7's `analysis.communities`). Every simplification is **reversible** — it records
what it removed, so a front can drill back in. An irreversible simplification is data loss disguised as an
optimisation.

### 4. `adaptive.rs` — iteration budgets

Per the reference: scale iteration counts to graph size so a layout returns in a bounded time rather than
running to a fixed tick count regardless of n.

**This directly conflicts with determinism** and the conflict must be resolved explicitly, not discovered:
if the budget depends on wall-clock time, the output is no longer reproducible (**D8** — no wall-clock in
the motor). So the budget must be a **pure function of graph size**, never of elapsed time. Write that
down; a "stop after 16 ms" budget is forbidden here, however tempting.

### 5. Verify every ceiling already in the ledger

Phases 1–8 each declared `scale_ceiling` values. Some were reasoned, not measured. This phase **measures
them** and corrects the ledger. `docs/measurements/phase09-ceilings.md` is the before/after table.

A declared ceiling that turns out to be wrong is a finding worth reporting, not an embarrassment to
quietly fix — and per rule 0.5 it must appear as a correction, with both numbers.

### 6. The benchmark campaign

For each gated layout, at N = **220, 10k, 100k, 1M**:

| Metric | Notes |
|---|---|
| Build time (ingest → indexed topology) | the O(n+m) claim, checked |
| Layout time (one-shot) or tick time (iterative) | the headline number |
| Peak memory | against the SoA/CSR budgets in `prompt.md` §5 — ~33 B/node and ~4.8 MB for 3 CSRs at 100k/300k |
| Snapshot bytes (binary and JSON) | the transport cost |
| WASM vs native, same N | the boundary cost, measured not assumed |
| Rust vs the TypeScript oracle, same N | **the project's justification** |

Report medians over repeated runs with the run count stated. A single timing is noise. Under 3% is noise
(`benchmarker.md`) — do not report a 2% win as a win.

### 7. `BENCHMARKS.md` — published, honest, complete

Include the losses. If WASM loses at N=220, the table says so and the SDK documents the crossover. A
benchmark table with no unflattering rows is marketing, and `minimalism-markers.md` requires saying what a
thing does *not* do.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

# determinism survives the SCALE stage (LOD hints are part of the hash)
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO

# adaptive budgets are a function of size, never of the clock
docker run --rm -v "$PWD:/w" ge-rust sh -c \
  'grep -rnE "Instant::now|SystemTime|elapsed" crates/graph-core/src && exit 1 || exit 0'       # 0

# simplification is reversible
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core simplify_reversible               # 0

# the campaign
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- bench --n 220,10000,100000,1000000 \
  --repeat 5 --out docs/measurements/phase09-bench.md                                          # 0

# every ledger ceiling is now measured, not reasoned
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check --ceilings-measured  # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                      # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

## Ledger delta

To `gated`: `scale.lod`, `scale.simplify`, `scale.adaptive`. **Every existing capability's
`scale_ceiling` is updated to a measured value**, and the ledger gains a flag distinguishing measured
ceilings from reasoned ones.

## Ponytail requirements

- **LOD thresholds are heuristic.** Name the failing input (a graph whose important nodes are low-degree,
  so a degree-based LOD hides exactly what matters), the direction (**hiding meaningful nodes — the
  dangerous direction**), and the escape hatch (ignore the hints; they are advisory).
- **Simplification** changes apparent structure: a contracted chain looks like an edge. Name it and name
  the reversal path.
- **Adaptive budgets** mean a large graph gets fewer iterations and a less settled layout. Direction:
  cosmetic. Escape hatch: an explicit iteration override.
- **The benchmark harness** is itself a sampler — a fixed seed set on one machine. Name the machine class
  and that results are not portable across hardware.

## Stop-and-ask

- Rust is **not** faster than the TypeScript oracle at any N → **stop and report it prominently**. That
  invalidates the project's premise and is a decision for a human, not a number to bury.
- A measured ceiling is dramatically below a declared one → stop and report before correcting the ledger;
  it may indicate a defect rather than an over-optimistic estimate.
- An adaptive budget seems to require wall-clock time → stop. That breaks D8 and the whole hash gate with
  it.
