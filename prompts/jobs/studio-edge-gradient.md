# Job studio-edge-gradient (agent build, studio)

Why: the user wants each edge coloured from its source node's colour to its target's, as SciGraphs draws
them. In SciGraphs, with `nodes_only` off, the edge tubes carry the node colour attribute, and Blender
interpolates it along each edge in **linear light** (`SciGraphs/SciGraphs/ui/coloring/functions.py:298-330`,
`ui/coloring/properties.py:195-215`, `api/render.py:288-345`). Today every edge is stroked in one flat
`theme.edge` (`packages/graph-render/src/canvas2d/edges.ts:129`), batched per CHUNK.

Facts:
- A node's colour is `style.palette[style.colours[node]]` (`canvas2d/nodes.ts:71`). The SciGraphs look
  uses a per-bucket base colour instead (`nodes.ts:104`). Edge endpoints come from the frame's
  source/target columns.
- Every drawn colour goes through `colour/srgb.ts` (`Rgb` is linear, `cssOf` encodes it once).
- Canvas2D gradients interpolate in **sRGB**, not linear light. A two-stop gradient is therefore not
  SciGraphs' blend, and the midpoint comes out too dark.
- Gate row `perf-edge-batch` (`deploy/perf/rows.py`, `EDGE_CHUNK`) holds the stroke count down. A
  `createLinearGradient` per edge on a large graph would lose the frame budget.

Do:
1. Read `canvas2d/edges.ts`, `nodes.ts`, `input.ts`, `style.ts`, `colour/*.ts`, `look/*.ts` and the
   studio's settings and actions (`packages/graph-studio/src/state/settings.ts`,
   `src/actions/registry.ts`) yourself first.
2. A renderer primitive in `packages/graph-render/src/colour/` (e.g. `blend.ts`). It mixes two linear
   `Rgb`s at t, and returns the gradient stops for one edge: K stops (K a named constant, ≥ 3) whose
   colours are the linear-light mix, each encoded once with `cssOf`. Unit-test it against hand-computed
   values: endpoints exact, midpoint equal to the linear mean, encoded.
3. An edge colour mode on the render style: `"flat"` (today's behaviour, the default, so existing
   bytes and goldens do not change) or `"gradient"`. In gradient mode:
   - Edges whose two endpoints share a colour are batched per colour, one stroke per CHUNK as today.
   - Edges with two different colours get a per-edge `createLinearGradient` from source to target with
     the stops from step 2.
   - Above a named budget of mixed edges per frame, or while the view moves (`input.moving`), mixed edges
     fall back to the flat linear mean of their two colours, batched per colour pair. Name the budget
     constant and give it a `Caveat:` line: it gives up the gradient on large or moving graphs, and a
     curved or routed edge's gradient follows its chord, not its path.
   - The focus pass keeps its behaviour: non-incident edges dimmed by `dimAlpha` (still in colour), and
     incident edges in `theme.edgeLit`.
   - Arrows take the target's colour.
4. Studio: a setting `edgeColour: "flat" | "gradient"`, registered once in `src/actions/registry.ts`,
   with a toggle in the dock's appearance section and a console command. It persists like the other
   settings. The SciGraphs look presets default it to `"gradient"`. Every other look keeps `"flat"`.
5. Tests:
   - A unit test for the primitive.
   - A render test: a 2-node graph with two palette buckets reads the source colour near the source,
     the target colour near the target and the linear mean at the middle, within one byte.
   - A test that the mixed-edge budget falls back to a flat colour and that the stroke count stays bounded.
   - A row `edge-gradient` in `scripts/studio-nav.sh` (or the closest sibling gate): switch the mode over
     CDP and read pixels along one known mixed edge. Its negative control must turn it red.
   - Screenshot the studio in both modes with the `pw` MCP and record the paths in the return block.

Paths: `packages/graph-render/**`, `packages/graph-studio/**`, `scripts/studio*.sh`, `deploy/nav/**`,
`deploy/perf/**`, `docs/measurements/studio-edge-gradient.md`. Nothing else. Do not touch `crates/`.

Done when:
- `scripts/studio.sh check` exits 0.
- `scripts/studio.sh build` then `scripts/studio-nav.sh` exits 0 with the new row PASS, and its negative
  control exits non-zero.
- `scripts/studio-perf.sh` (the `perf-edge-batch` row) still passes in both modes.
- `docs/measurements/studio-edge-gradient.md` records the stroke counts and fps for flat and gradient
  at 2000 nodes.
- The return block pastes each command's real exit code.
