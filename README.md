# @osionos/graph-engine

A self-contained, **framework-agnostic** force-directed graph engine for visualizing data
relationships. Built for speed (Canvas2D + a Web-Worker `d3-force` layout) and dressed in an
**aurora-glass** visual language: a drifting indigo→violet background, glassy radial-gradient
nodes whose size grows with their degree, and neon links whose thickness tracks relationship
strength.

It is **decoupled from the host application** like an installed dependency: the engine knows
nothing about BaaS, Notion, routing, or stores. You hand it a `GraphModel` and a `Controls`
object; it renders and emits events. Data sourcing stays in the app.

## Boundary contract

- `src/core/**` is pure: **no React** — *this half is machine-enforced.* `eslint.config.js` applies a
  `no-restricted-imports` rule to `src/core/**/*.{ts,tsx}` that rejects `react`, `react-dom` and
  their subpaths, so the framework-agnostic half of the contract cannot rot silently.
  Only the data contract, layout, rendering, theme, and control state live here.
- `src/react/**` is the only place React appears — a thin adapter (`GraphView`, `GraphConsole`)
  that mounts the core engine into a `<canvas>` and binds the control panels.
- **No `@/…` host-app imports anywhere.** The host repo also banned the `@/*` alias for the same
  reason, and that half was dropped on extraction: there is no host app here, so the specifier has
  nothing to resolve to. It is therefore a *convention* in this package, not an enforced rule — if
  you add a `@/` import, nothing will stop you. Review by hand.

## Public API

```ts
import {
  // data contract
  type GraphModel, type GraphNode, type GraphEdge, type GraphStats, type GraphPatch,
  type NodeId, type EdgeId, type NodeKind, type EdgeKind,
  // model assembly + identity
  indexModel, emptyModel, nodesEqual,
  makeRecordNodeId, makeNoteNodeId, makeTagNodeId, makeEdgeId,
  applyDegreeWeights,
  // engine + control state
  GraphEngine, DEFAULT_CONTROLS, cloneControls,
  type Controls, type FilterState, type VisualState, type SearchState,
  type BackgroundStyle, type LayoutParams, DEFAULT_LAYOUT_PARAMS,
  // host-facing model helpers
  deriveLegend, type Legend, type LegendEntry, type KindEntry,
  neighborhood,
  // react adapter
  GraphView, GraphExplorer, Minimap, GraphConsole,
  useGraphEngine, useControls,
} from "@osionos/graph-engine";
```

Import the stylesheet once, if you want the console panels and canvas container styled:

```ts
import "@osionos/graph-engine/styles/graph.css";
```

## Layout

```
src/
  core/        framework-agnostic engine
    types.ts   GraphNode / GraphEdge / GraphModel data contract
    model/     ids + indexing + degree weights + tag/filter helpers
    camera/    world<->screen transform
    layout/    d3-force in a Web Worker, with live physics-param updates
    render/    aurora background, glassy nodes, conditional links, glow, reveal
    theme/     resolve --osio-* tokens + aurora palette
    state/     controls store (filters / physics / visual / search)
  react/       GraphView + GraphConsole + console panels + export
  styles/      graph.css (tokens only — CSP-safe, no inline styles)
tests/
  graph-engine-*.test.ts unit suite for core (node --test)
```

## Standalone usage

**Requires Node >= 22.6.** The test script uses `--experimental-strip-types`, which does not exist
on Node 20 — `node --test` there dies with `node: bad option: --experimental-strip-types`. The
package declares this in `engines`, but `engines` is advisory: without `engine-strict` in `.npmrc`,
`npm install` only warns and the failure lands at `npm test`, after you are already invested.

The reproducible path is the committed `Dockerfile` — a pinned `node:22-slim` that runs the gate as
the build step, so you never depend on the host's Node:

```sh
docker build -t graph-engine-check . && docker run --rm graph-engine-check
```

With a suitable local Node:

```sh
npm ci        # not `npm install` — see below
npm run check # typecheck + lint + test
```

`npm ci` rather than `npm install`: it installs exactly `package-lock.json` and fails if the lock and
`package.json` disagree, so a drifting transitive dependency cannot quietly change what the gate
checks. That is why the lockfile is committed and why the eslint toolchain is pinned to exact
versions instead of ranges.

`npm run check` is the gate: `tsc --noEmit`, `eslint src --max-warnings=0`, and
`node --test tests/*.test.ts`. It needs no host app, no bundler config and no network.

The package ships TypeScript source (`exports` points at `./src/index.ts`), so a consumer must
resolve it through a bundler or a TypeScript-aware resolver rather than plain Node `require`.

## Verifying parity against the host

The extraction is checked against osionos' in-tree engine in a real browser:

```sh
./verify/run-parity.sh
```

It mounts the host's engine and this package's engine in one document, feeds both the identical
model, and diffs their canvases pixel by pixel for every host palette in light and dark. Current
result: **0 differing pixels of 744,000 per combination, 16/16 combinations**, with node positions
bit-identical (0/220 differ) and no console or page errors.

Two things make that number mean something. A third panel mounts the host engine a *second* time as a
control, because a time-driven force layout makes even identical code render differently between two
panels — without that floor, a difference is uninterpretable. And the rig is shown to be capable of
failing: perturbing one hex in this package's palette flips the verdict to `DIVERGENT` while the
control stays at zero.

`EXTRACTION_REPORT.md` §4 records the result, the exit codes, and three false passes the rig
initially reported and had to be fixed for (a blank canvas that compared "identical", a token
stylesheet that 404'd into an HTML fallback with a 200 status, and a palette sweep that was not
actually discriminating). Exit `1` means *inconclusive*, which is deliberately distinct from exit
`0`: "I could not tell" is not "they match".

## Limitations

**The theme is coupled to the `--osio-*` CSS custom properties by name.** `resolveSceneTheme()`
(`src/core/theme/tokens.ts`) reads ~27 tokens off the document root with
`getComputedStyle` at runtime — `--osio-graph-ink`, `--osio-graph-bg-1`, `--osio-graph-edge`,
`--osio-graph-card-bg`, and so on. Those names are an osionos convention, not a standard.

What actually happens, precisely: **every one of those reads has a hardcoded fallback**
(`value || fallback`), so a standalone consumer does not get an unstyled or broken graph. It gets
the engine's baked-in warm-dark aurora theme (`DARK_THEME`, `AURORA_BG_TOP`/`_BOTTOM`, and aurora
bands derived from the accent and note hues, because no palette ever defined
`--osio-graph-aurora-*`).

The real cost is therefore **silent divergence, not a crash**:

- A host that defines `--osio-graph-*` renders in its own palette. A host that does not renders in
  the hardcoded dark theme. Both look intentional.
- There is no error, warning, or log when the tokens are missing — a typo in a token name, or a
  host that renames them, produces a wrong-but-plausible graph with nothing to detect it.
- Theme switching is likewise unavailable standalone: the light/dark `mode` is inferred from the
  luminance of `--osio-graph-bg-1`, so with the fallback it is always `dark`.
- `src/styles/graph.css` mirrors the coupling on the CSS side (`var(--osio-graph-bg-1, #15140f)`),
  so it degrades the same way rather than failing. Its three fallback values were stale until this
  extraction: they were cold-indigo leftovers (`#160c30`, `#e7e9f5`, `#a5b4fc`) from a pre-"Warm
  Constellation" design, disagreeing with both the host and this package's own TypeScript fallbacks,
  so a standalone consumer got cold-indigo chrome around a warm-charcoal canvas.

Escape hatch, if you need to theme this from a non-osionos host: define the `--osio-graph-*` tokens
yourself before mounting (`--osio-graph-ink`, `--osio-graph-bg-0`/`-1`, `--osio-graph-select`,
`--osio-graph-note`, and the edge/card/node families). Only the tokens you care about need
defining — each falls back independently. Shipping neutral default token values *in this package*
would remove the coupling, but that is new scope, not extraction, so it is deliberately not done
here.

The other boundary worth stating: `src/core/layout/layoutBridge.ts` constructs its worker with
`new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" })`. That form is
portable, but it means the consumer's bundler must understand `new URL(..., import.meta.url)` as a
worker entry — a constraint several older bundler configurations do not satisfy.

## Why a package and not a folder

A real package boundary (own `package.json`, `exports`, README, an import firewall, and its own
test suite) keeps the graph swappable and independently testable — it behaves like a plugin you
installed, even though a consumer may still choose to wire it in by Vite alias or tsconfig `paths`
instead of a registry install. How it gets resolved is the consumer's choice; what the package
guarantees is the boundary, not the wiring.
