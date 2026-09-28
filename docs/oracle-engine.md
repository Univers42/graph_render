# The TypeScript engine (differential oracle)

How the TypeScript engine under `src/` builds, tests and fits together. Since Phase 0 it is the
differential oracle for graph-motor (`prompt.md` §2.1), never shipped code, never deleted.

## What this repo is

`@osionos/graph-engine` — a framework-agnostic force-directed graph engine (Canvas2D + d3-force in a
Web Worker) with a thin React adapter, extracted from the osionos host app's `packages/graph-engine`
(`EXTRACTION_REPORT.md`). It ships TypeScript source (`exports` → `src/index.ts`): **no build step, no
bundler, no dev server** — a consumer bundles it. `PARITY.md` is host history, not documentation of
this package.

Two git submodules: `.claude/` (the engineering config; its `rules/*.md` load automatically, see
`.claude/README.md` and `.claude/AGENTS.md`) and `SciGraphs/` (a Python Blender extension, the reference
design for the roadmap below). Run `git submodule update --init` if either is empty.

## Commands

Node **>= 22.6 is required**, not advisory: `npm test` uses `--experimental-strip-types`, which Node 20
does not have. Use `npm ci`, never `npm install` — the lockfile and the exact-pinned eslint toolchain
are load-bearing (a caret range once pulled a plugin version whose new rule failed the gate).

```sh
npm ci                  # once; tests fail with ERR_MODULE_NOT_FOUND on d3-force without it
npm run check           # THE gate: typecheck + lint + test (tsc --noEmit, eslint src --max-warnings=0, node --test)
npm run typecheck
npm run lint
npm test                # 27 tests in tests/graph-engine.test.ts

# one test, by name pattern (the loader flag is mandatory — src/ imports are extensionless)
node --test --experimental-strip-types --experimental-loader ./tests/ts-extension-loader.mjs \
  --test-name-pattern 'camera' tests/graph-engine.test.ts

# the same gate, reproducibly, without depending on the host's Node
docker build -t graph-engine-check . && docker run --rm graph-engine-check
```

- **Test framework is Node's built-in `node:test` + `node:assert/strict`**, one suite file.
  `.claude/tools/facts.sh` / `digest.sh` report "test framework: none" because they do not detect
  `node:test` — do not add vitest/jest on that basis. Layout tests run because `LayoutController`
  falls back to an in-process `LayoutEngine` when `Worker` is undefined.
- `.claude/tools/quality.sh` lints a **wider surface than the package gate** (eslint over `.`, plus
  shfmt/shellcheck). It currently fails on `verify/*.mjs` (rig scripts outside `eslint src`) and on
  `verify/run-parity.sh` formatting. `npm run check` is what the README and Dockerfile define as green.
- `./verify/run-parity.sh` is the pixel-parity rig against the host engine. It needs Docker and an
  osionos checkout at `HOST_APP` (default `/home/dlesieur/Documents/osionos`), which **does not exist on
  this machine** — it cannot run here. Exit 0 = identical, 2 = divergent, 1 = inconclusive (distinct on
  purpose: "could not tell" is not "they match").
- `.claude/settings.json` puts `npm`, `npx`, `docker`, `curl` on the permission "ask" list; the
  `.claude/tools/*.sh` scripts and `./node_modules/.bin/*` run without a prompt.

## Architecture

### The firewall: `src/core` vs `src/react`

`src/core/**` must never import React — enforced by a `no-restricted-imports` rule in
`eslint.config.js`. Only the data contract, layout, rendering, theme and control state live there.
`src/react/**` is the only place React appears. Everything public is re-exported from the
`src/index.ts` barrel; add exports there, not via deep paths. `@/…` host-app imports are banned by
convention only (nothing enforces it here).

### Data flow (host → pixels)

```
host builds GraphModel (src/core/types.ts; indexModel/diffGraph in core/model/)
  → GraphEngine (core/engine.ts)            facade; owns the three below, small imperative API
      ├─ LayoutController (core/layout/layoutBridge.ts)   main-thread side: stable nodeId↔index map,
      │     └─ layout.worker.ts ⇄ LayoutEngine (layoutEngine.ts)  d3-force ticks off-thread; the SAME
      │        LayoutEngine runs in-process when Worker is unavailable (tests). Physics lives once.
      ├─ CanvasScene (core/render/scene.ts)  thin façade over SceneState (columnar Float32/Uint8
      │     buffers), SceneCamera, SceneSelection, sprite + label caches; redraws only when dirty
      │     └─ renderFrame.ts composes the passes — one module per pass (links, nodes, glowPass,
      │        rings, labels, nodeCard, clusterBlobs for the far-zoom overview LOD)
      └─ AuroraBackground (core/render/background.ts)  separate stacked canvas
```

Positions travel as flat `Float32Array`s from the worker to the scene; React never participates in
the draw loop. `LayoutController.rebuild` preserves surviving nodes' positions and **skips the
simulation entirely when topology (id order + link count) is unchanged**, heating only by the fraction
of new nodes. The worker is created with `new Worker(new URL("./layout.worker.ts", import.meta.url),
{ type: "module" })` — a consumer's bundler must support that form.

### Controls are the single source of truth

`src/core/state/controls.ts` defines `Controls = { filter, physics, visual, search }` as plain
serializable data (arrays, not Sets — it round-trips through localStorage). The engine and the React
panels both bind to it. `GraphEngine.setControls` reheats the layout **only when `physics` changed**;
a filter or visual toggle must never re-run the simulation. Selection and focus (neighborhood
dimming) are owned by the host and pushed in via `setSelected` / `setFocus`; the engine only emits
`onSelect` / `onHover` / `onExpand`.

### Theme resolves at runtime from CSS custom properties

`resolveSceneTheme` (`src/core/theme/tokens.ts`) reads ~27 `--osio-graph-*` tokens off `themeRoot`
with `getComputedStyle`, each with a hardcoded fallback (`DARK_THEME`, `core/theme/palette.ts`).
Consequence: a missing or misspelled token yields a wrong-but-plausible graph with no error — silent
divergence, never a crash. Light/dark `mode` is inferred from the luminance of `--osio-graph-bg-1`, so
standalone it is always dark. `useGraphEngine` re-resolves on mutations of `data-theme`,
`data-palette`, `class`, `style` on the document root; `data-palette` is load-bearing.

### React adapter (`src/react/`)

`useGraphEngine` creates the engine once and syncs model/controls/selection/focus through effects
(ResizeObserver for size, MutationObserver for theme). `GraphView` is the bare canvas surface
(stateless about controls); `GraphConsole` is the panels (`console/*Panel.tsx`); `useControls`
persists `Controls` per `storageKey`; `GraphExplorer` composes `GraphView` + `GraphConsole` + `useControls`
and is the batteries-included entry point. `Minimap` is a separate export the host mounts itself, polling
`engine.getMinimapSignature()` / `getMinimapData()`.

