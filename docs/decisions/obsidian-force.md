# Obsidian's force algorithm at every size

Status: accepted, 2026-10-03. Asked for by the user the same day: 1M nodes, live, "the same
algorithm as in Obsidian".

## Context

- Obsidian does not publish its source. Its graph view has four force sliders: center, repel,
  link force and link distance. These are d3-force's forces, and the studio already exposes them
  under those names (`packages/graph-studio/src/motor/live.ts:7-16`, `actions/forces.ts:1-5`).
- The motor's live session is a port of the pinned d3-force oracle
  (`crates/graph-core/src/layout/force/session.rs:15-26`, `prompts/REFERENCES.md` Tier 1). It
  makes two declared deviations: link and collide are Jacobi gathers, and `jiggle` is a hash
  (`session.rs:28-31`). The many-body force is d3's Barnes-Hut with θ = 0.9
  (`params.rs:15-17`, `barnes_hut/charge.rs:1-12`).
- Above `LIVE_NODES` = 5 000 the studio runs the particle mesh instead
  (`packages/graph-studio/src/motor/settle.ts`, `docs/measurements/perf-pm-live.md:14`). The mesh
  keeps Barnes-Hut's link, center, collide and integrator, but replaces the many-body force with a
  mesh solve that smooths everything closer than about two cells
  (`docs/measurements/perf-p2-pm.md:9,95`). That is not Obsidian's algorithm.
- Cost: one Barnes-Hut tick at 1M is 1 635 ms serial native, and charge plus collide take 95.5 %
  of its instructions (`perf-p2-pm.md:7,30`). The walk is already a parallel gather
  (`charge.rs:39-51`). The tree build and the bottom-up aggregate are serial
  (`charge.rs:61-70`, `quadtree/build.rs`).

## Decision

1. **The reference is d3-force**: the pinned oracle, not Obsidian's binary. The session already
   ports it. The two declared deviations stay.
2. **"The same algorithm" has a measurable meaning.** At every size, the forces, the integrator
   and the cooling are d3's, and the many-body force is Barnes-Hut θ (`manyBody.js`). Another
   many-body solver (the particle mesh, a GPU f32 walk) may stand in only if its force error
   against the exact all-pairs sum is no larger than Barnes-Hut θ = 0.9's own error on the same
   positions. The `mb-fidelity` gate measures this (job `perf-mb-fidelity`). Until a solver
   passes, the studio labels it "approximate" and does not present it as Obsidian's.
3. **The engine for each size comes from a measurement, not a constant.** Barnes-Hut runs
   wherever its tick fits the live budget on the device: native ≤ 25 ms, browser ≤ 100 ms
   (`prompts/perf-plan.md`). The studio's threshold is that measured ceiling. Above it, the
   particle mesh runs, labelled approximate, until Barnes-Hut covers 1M through:
   - (a) the CPU: a parallel tree build and aggregate that produce the same bytes (job
     `perf-bh-1m`);
   - (b) the GPU tier (`docs/decisions/gpu-force-tier.md`).
4. **Drawing at 1M covers every node kind** (Point, Circle, Box), **every edge kind** (Line,
   Polyline, Curve), gradient edges, line width and arrows, all on the GPU path, at ≥ 30 fps for
   1M nodes and 1.5M edges.
   - Today the WebGL2 layer draws curved and routed edges straight, draws lines 1 px wide and
     draws no arrows (`packages/graph-render/src/webgl2/layer.ts:8-10`). Gradient edges and
     Box/Circle nodes already work there (`webgl2/draw.ts:76,94`).
   - `packages/graph-render` belongs to a peer. The brief for this work is
     `prompts/jobs/render-1m-kinds.md`.

## Consequences

- `LIVE_NODES` stops being the engine switch. It becomes the measured Barnes-Hut ceiling once
  `perf-mb-fidelity` and `perf-bh-1m` report.
- The particle mesh stays registered, keeps its own hash-gate entry and remains selectable.
- Collide is not one of Obsidian's sliders. It is an osionos constant (`collideRadius: 16`,
  `params.rs:9`) and costs one quadtree per tick. An `obsidian` preset with collide off is a
  parameter preset, not a code fork. `perf-bh-1m` measures its share before anyone proposes
  the preset.

## What this does not do

- It does not reproduce Obsidian's pixels or Obsidian's default values, which are unpublished.
  The defaults stay the cited osionos and d3 values (`params.rs:1-20`).
- It does not make Canvas2D draw 1M nodes: the bulk path is WebGL2 (`webgl2/plan.ts`,
  BULK_THRESHOLD).
