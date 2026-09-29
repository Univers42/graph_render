# graph-motor studio

A graphical interface for the whole engine: provisional ingest goes in, the wasm
motor runs a layout, and an aurora-glass Canvas2D view shows the geometry that
came back. Every layout the module registers is in the picker, because the picker
asks the module.

The studio **imports** the engine; it does not copy it and it does not modify it:

| what it uses | from |
| --- | --- |
| `createMotor`, `Motor#build/#layouts/#layout/#column`, `ColumnId`, the `GraphMotorError` subclasses | `crates/graph-sdk-js/src/index.ts` |
| `Motor#posts/#post`, `Motor#analyses/#analysis`, `PostResult`, `AnalysisResult` — the POST and ANALYSIS surface | `crates/graph-sdk-js/src/index.ts` |
| the aurora field, the node sprite baker, the resolved `--osio-*` theme | `src/core/render/`, `src/core/theme/` |
| the camera maths (`zoomAt`, `panBy`, `visibleWorldRect`, the zoom clamps), the colour and shape vocabulary (`nodeFill`, `shapeOf`, `edgeStroke`) | `src/core/camera/`, `src/core/theme/` |

Nothing under `src/`, `crates/`, `tests/` or `verify/` is touched by this branch.

## Running it

```sh
scripts/studio.sh            # dev server on http://localhost:5173
scripts/studio.sh build      # production build into app/dist
scripts/studio.sh test       # the unit tests
scripts/studio.sh check      # typecheck + tests + render smoke + live motor + build
```

`scripts/studio.sh` builds the wasm through the Rust helper
(`gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release`, which
carries the 8 GB memory cap and the cargo registry volume), copies
`graph_wasm.wasm` and the repository's `fixtures/` into `app/public/`, then runs
`npm ci` and `npx vite --host 0.0.0.0 --port 5173` inside `node:22-slim` with
`-p 5173:5173` — the only difference from `orch/bin/node-slim.sh`, since a studio
you cannot open is not a studio. No npm, node or cargo runs on the host.

Opening the page runs the first registered layout over an opening graph, so there
is something on screen before anything is clicked.

## What it shows

**Data.** Three ways in, all of them the same provisional ingest document that
`gm_build` reads:

- a **synthetic graph** from a seed, a node count and a reference degree. The
  engine's own seed export (`gm_seed_ingest`) is deliberately not on the
  published SDK surface — it is a gate-only export at a fixed node count and
  degree, neither of which the studio can vary — so the studio generates the same
  *shape* of document from the same mulberry32 stream `src/core/model/synthetic.ts`
  documents, and hands it to `gm_build` like any other ingest. Same seed, same
  bytes, every time.
- a **JSON file from disk**, normalised into the ingest shape.
- the **engine's own fixtures** (`fixtures/**/*.json`, staged into
  `app/public/fixtures/`). None of them is in the ingest shape: they are
  shorthand (`{about, nodes:[{id}], edges:[{id,source,target}]}`) with
  annotations. The normaliser fills every missing member with a documented
  default, maps an edge's `type` to a kind, drops annotations, and **lists every
  default it filled in the panel** — a silent fill would be a lie about what the
  engine was given. What the contract refuses (a duplicate id, a dangling
  endpoint, a version other than 1, a record with no `id`) is refused loudly, with
  the file and the member named.

**Layout.** The picker is filled from `Motor.layouts()` — the module's registry, in
registry order — and no layout id is written down anywhere in `app/`. The status
card shows build ms, layout ms, node and edge counts, the node and edge geometry
kind, and every column the run carried or did not (`absent` in grey, per C3).

**Post-processing.** The pass picker is filled from `Motor.posts()`, the same way
(C1), and no post id is written down in `app/` either. A pass is **not a second
run**: it runs on the SAME handle as the layout above it and REPLACES that
handle's edge geometry, so the studio re-reads every column after the pass —
including the two geometry *kinds*, since `post.route.grid` over a `Line` layout
yields a `Polyline`. The status card then reports the pass's own id and duration
and restates the column list, so `edge.pts` stops reading `absent` exactly when a
routed arc is on screen.

**Clearing a pass re-runs the layout.** The motor has no "un-post": a pass
overwrote the handle's edge columns, and the layout's own geometry is reachable
again only by running it (`docs/contract/wasm-abi.md` "POST"). The panel says so
rather than pretending the old drawing is still in there. Each pass also reads
the **layout's** edges, never the previous pass's, so two passes in a row give
the same picture as the second alone — checked on real output, not assumed.

**Analysis.** The picker is filled from `Motor.analyses()` (C1). An analysis is a
pure function of the **topology**, so it needs no layout run at all: the studio
runs it against the live handle whatever is on screen. What the face does to the
canvas is decided by the face's own `kind`, never by its id:

| `kind` | what the nodes wear | what the legend says |
| --- | --- | --- |
| `f64` (a magnitude) | a five-stop sequential ramp across the face's **own** min..max | its two ends, in the ramp's own colours |
| `u32` (a labelling) | one palette colour per **group**, by rank among the ascending distinct groups | every group, up to eight, then a count |

Two decisions there are load-bearing. The ramp spans the values the run
produced, not the unit interval, so a betweenness face of 6..1145 reads exactly
like one of 0.06..11.45; and a flat domain paints the **middle** of the ramp
rather than its cold end, because every node scoring zero is a claim about the
data, and a 0..0 face does not mean it. Group colours follow the group's RANK, so
a Louvain run that numbered its communities 5, 9 and 17 colours exactly like one
that numbered them 0, 1 and 2 — a community id is an opaque number, and a legend
that read differently for the same partition would be a lie.

The panel lists the face's own scalars — `kind`, `nodeCount`, the range or the
group count — and the three optional members **exactly when the analysis handed
one back**: `converged` (an eigen-iterate that oscillated is three equal numbers,
not a centrality), `modularity`, `max` (the deepest hierarchy level). Hovering a
node adds that node's value to the tooltip, so a colour is never unexplained; an
index the face does not cover adds nothing rather than another node's value.

**Animation.** Switching layouts moves the graph to its new positions over 600 ms
on an ease-in-out cubic. The maths is pure (`src/core/transition.ts`,
`src/core/frame.ts`) and pinned by tests. Two cases cross-fade rather than morph,
because morphing them would be nonsense: a run with a different node count, and an
edge whose point count changed (a routed arc replacing a straight line).

**Rendering.** All six geometry kinds. A `Polyline`/`Curve` edge's `pts` row
holds its **interior** points — the endpoints are the two node positions, which
`paint.ts` supplies itself (`moveTo` source, the row, `lineTo` target). The
coordinates arrive in motor units and `core/worldScale.ts` normalises them once
per run in `GraphView.setData`; an analysis overlay passes through unscaled,
because it is a colour and a colour times 56 is nonsense.

| kind | how it is drawn |
| --- | --- |
| `Point` | a blitted sprite at one fixed radius, or a plain disc when it is too small to read |
| `Circle` | the same sprite at the run's own `r` |
| `Box` | a painted rounded rect (backing, body, hairline rim) at the run's own `w`/`h` |
| `Line` | a segment between the two node positions |
| `Polyline` | its own `pts` window, named by `offsets` |
| `Curve` | its sampled `pts` run, at the snapshot's own degree |

Pan by dragging, zoom on the wheel (anchored at the cursor), double-click to fit,
hover for a tooltip with the node's label, kind and dense index — plus the applied
analysis's value for that node when one is applied — click to highlight
its neighbourhood (click the background to clear it). **Compare mode** runs two
layouts over the SAME handle — one graph, two layouts, byte for byte — and shows
them side by side. **Export PNG** composites the aurora field under the graph
canvas.

**Errors.** A `GraphMotorError` subclass shows its own name, its wire code and
name, the ABI's message, and a hint about what to do; a degraded motor (the module
never loaded) gets a standing banner and the studio stays usable around it. A
refused ingest document is reported in the same banner. There is no path from a
failure to a blank page.

## Why the studio has its own renderer

The engine's `CanvasScene` was the first choice and is reused for everything it
can express: the `AuroraBackground` field, the `NodeSpriteCache` that bakes the
aurora-glass node, the resolved theme, and the camera maths. What it cannot
express is the geometry:

- `src/core/render/nodes.ts` blits a pre-baked **disc** sprite per node, chosen by
  the node's *kind* (`shapeOf`), with the size coming from a degree-derived radius.
  A `Box` node's `w`/`h` never reach it, and neither does a `Circle`'s own `r`.
- `src/core/render/links.ts` draws every edge as `moveTo`/`lineTo` between two node
  positions from `SceneState`. It has no path, no `offsets`, no `pts` and no
  degree — a `Polyline` or `Curve` edge cannot be drawn by it at all.
- `CanvasScene.setPositions(x, y)` takes node positions only, so a routed edge's
  points have no path into the scene even by hand.

So `app/src/render/` paints the six kinds itself (`paint.ts`, 227 lines) over the
engine's camera, theme, sprite baker and background. The alternative — using `CanvasScene`
and accepting that two of the six kinds render wrong — was not worth it for a
studio whose job is to show what the engine produced.

## Tests

```sh
scripts/studio.sh check
```

- `app/tests/*.test.ts` — `node:test` unit tests for the pure helpers, runnable
  with plain `node --test --experimental-strip-types` (no bundler, no loader): the
  column → draw list mapping for all six kinds and every refusal, fit-to-view, the
  transition, hit-testing and neighbourhoods, the synthetic document's exact bytes,
  the ingest normaliser, the frame the transition produces, the error text, what a
  POST pass does to a run report, the two colour ramps, the analysis projection
  (fills, legend, readout, hover value) and the hover line. 136 tests.
- `app/tests/paint.dom.ts` — the render path against a stubbed DOM and a recording
  2D context: one frame per geometry kind, asserting the primitives each kind must
  produce, plus an applied analysis overlay over each kind that takes a fill
  (`Point` sprite, `Circle` sprite, `Box` body, far-zoom disc) and a partial
  overlay that falls back. A script, not a suite entry, because it needs the
  engine's own resolution shim for the render layer's extensionless imports.
- `app/scripts/verify-motor.ts` + `app/scripts/checkStages.ts` — the real wasm
  module through the real SDK: all ten registered layouts over one synthetic graph
  and three fixtures, each run's columns mapped through the studio's own draw-list
  builder; then all seven registered POST passes, each from a FRESH layout (a pass
  reads the layout's edges, so a chained one would hide a pass that read its
  predecessor), each checked for what it must NOT change (node kind, node count,
  edge count), the composition of two passes against the second alone, and all
  eight registered analyses over a handle that has run NO layout. It is the check
  the unit tests cannot be, and it fails loudly rather than skipping when
  `graph_wasm.wasm` is missing.

The first two run in the browser's absence on purpose: the studio's own
verification does not depend on a human opening it.

## What it does not do yet

- **A pass in compare mode.** Compare runs two layouts over the ONE handle, so
  only the last is the handle's geometry; a pass would replace one pane's edges
  and leave the other showing a drawing the handle no longer holds. The
  post panel is closed while compare is on, and says why. The analysis overlay is
  per-graph, so it does show on both panes.
- **Layout parameters.** `Motor#layout` runs each registered layout at its
  defaults, because the ABI's params argument is refused as non-empty this phase
  (C2). The studio therefore has no parameter sliders, and will not until the ABI
  accepts them.
- **Editing.** The studio reads and draws. It does not add, move, delete or persist
  nodes and edges; the one thing a pointer can change is which node is selected.
- **Multiple graphs, sessions, snapshots.** One ingest, one handle, one motor, one
  browser tab.
- **A11y and keyboard navigation.** The canvas is pointer-driven; the panels are
  ordinary form controls. `prefers-reduced-motion` is honoured (the transition is
  skipped), and nothing else is.
- **A light theme.** The studio publishes one dark `--osio-*` token set
  (`app/src/ui/studioTokens.ts`). The engine's theme resolver reads whatever tokens
  are on the document root, so a light set is a change to that one table.
