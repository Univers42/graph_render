# Extraction report — `@osionos/graph-engine`

**Date:** 2026-09-27
**Source:** `/home/dlesieur/Documents/osionos/packages/graph-engine`
**Destination:** `/home/dlesieur/Documents/graph-engine` (new standalone git repo, outside the host tree)
**Status:** Phase 1–3 complete. Phase 4 (wiring back as a submodule) **not started** — awaiting human.

---

## 1. What was copied, and what was rewritten

`src/` is a **byte-for-byte copy** — 71 files, 5,819 LOC (57 in `core/`, 12 in `react/`). No file
under `src/` was edited, so the §4.1 and §4.7 "identical copy" gates hold:

```
$ diff -rq /home/dlesieur/Documents/osionos/packages/graph-engine/src src
(no output)
```

| File | Verbatim? | What changed |
|---|---|---|
| `src/**` (71 files) | **byte-for-byte** | nothing |
| `tests/graph-engine.test.ts` | copied, 2 edits | 18 import paths rewritten (`../../packages/graph-engine/src/` → `../src/`); 2 lines replaced by 6 to fix a real type error (§4) |
| `package.json` | rewritten | new `exports`, `peerDependenciesMeta`, `devDependencies`, `engines`, scripts |
| `tsconfig.json` | rewritten | was 3 lines (`extends ../../tsconfig.json`) whose parent does not exist standalone; now self-contained |
| `eslint.config.js` | new | ported from the host's flat config |
| `tests/ts-extension-loader.mjs` | new | ported (partial — §3) |
| `README.md` | edited | Limitations + Standalone usage added; stale API block fixed |
| `Dockerfile`, `.dockerignore`, `.gitignore`, `package-lock.json` | new | reproducible containerised gate |
| `PARITY.md` | byte-for-byte | nothing |

### Deviations from the runbook, and why

1. **`.gitignore` added** (not in the runbook). Phase 3's `git add -A` would otherwise have
   committed `node_modules` — 125 packages. One line, `node_modules/`.
2. **Template versions overridden by the template's own instruction.** §4.3's template pins
   `typescript ^5.9.3` / `@types/node ^22.10.0`, but its text says *"use its actual output — don't
   guess version numbers"*. The host pins `~5.7.3` / `^22.0.0`, so those were used.
3. **ESLint port is larger than §4.5's template.** The template carries only the React firewall. That
   version reports two errors that are artifacts of an incomplete port, not defects (§2). The port now
   includes the rule *definitions* the copied code depends on.
4. **`tests/ts-extension-loader.mjs` added** (not in the runbook). Without it the test cannot run at
   all (§3).
5. **Node 22 provided by Docker, not the host.** The host Node is v20.19.2; `--experimental-strip-types`
   requires ≥22.6. The package therefore ships a `Dockerfile` and declares `engines.node >=22.6`.

---

## 2. The gate, and the four failures it found

`npm run check` = `typecheck` + `lint` + `test`. Run in a clean container from `npm ci`:

```
$ docker build -t graph-engine-check . && docker run --rm graph-engine-check
> tsc -p tsconfig.json --noEmit          # exit 0
> eslint src --max-warnings=0           # exit 0
> node --test --experimental-strip-types --experimental-loader ./tests/ts-extension-loader.mjs tests/*.test.ts
# tests 17
# pass 17
# fail 0
                                                          GATE_EXIT=0
```

Four failures surfaced on the way. **Every one was config/version drift, not a defect in the engine.**

**(a) Latent type error, invisible in the host.** `tests/graph-engine.test.ts:154-155` read
`state.visible[state.idToIndex.get("a")]` — `Map.get` returns `T | undefined`, so the index is
`undefined`. This has *never* been caught upstream, because osionos's root `tsconfig.json` has
`include: ["src", "packages"]` — `tests/` is not in the program — and canvas tests only ever run
through `node --experimental-strip-types`, which *strips* types without checking them. Adding
`tests` to the standalone tsconfig is what surfaced it.

Fixed by narrowing, not suppression (no `!`, no `eslint-disable`, `strict` untouched). Both original
assertions preserved verbatim in meaning:

```ts
const aIndex = state.idToIndex.get("a");
const tIndex = state.idToIndex.get("t");
assert.ok(aIndex !== undefined);
assert.ok(tIndex !== undefined);
assert.equal(state.visible[aIndex], 0);
assert.equal(state.visible[tIndex], 1); // tag has no databaseId
```

Test count is unchanged: 17 `test(` calls before and after. Only import-path and these 6 lines differ
from the host original.

**(b) ESLint plugin drift — the exact trap a stale lockfile causes.** §4.3's `^7.0.1` range resolved
to `eslint-plugin-react-hooks@7.1.1`, which added a `react-hooks/refs` rule. It failed
`src/react/useGraphEngine.ts:35` (`cbRef.current = args.callbacks` — "Cannot update ref during
render"). osionos's `pnpm-lock.yaml` pins **7.0.1**, which has no such rule, so the host is clean.
The whole eslint toolchain is now pinned **exact** to the host's resolved versions
(`eslint 9.39.4`, `typescript-eslint 8.58.1`, `eslint-plugin-react 7.37.5`,
`eslint-plugin-react-hooks 7.0.1`).

**(c) Incomplete ESLint port.** The minimal §4.5 config produced two more errors, both artifacts:
`_style` unused in `core/render/nodeGlassDark.ts:141` (the host allows it via
`"@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }]`), and
"Definition for rule 'react-hooks/exhaustive-deps' was not found" (the source carries a disable
comment for a plugin that must be registered). Fixed by porting the rule *definitions* — deliberately
**not** by touching the code or relaxing a rule.

**(d) `node: bad option: --experimental-strip-types`.** Host Node is v20.19.2. Fixed by running the
gate in `node:22-slim` and declaring `engines.node >=22.6`.

---

## 3. The test could not run without a ported loader

`src/` uses bundler-style imports (`./core/math`, no extension) — fine for Vite and tsc
(`moduleResolution: "bundler"`), fatal for Node's ESM resolver:
`ERR_MODULE_NOT_FOUND: file:///w/src/core/math`.

The host solves this with `tests/canvas/ts-extension-loader.mjs`. That loader does two jobs: it
mirrors the host's Vite/tsconfig aliases (`@/` → `src/`, `@osionos/*` → `packages/*`) **and** resolves
extensionless relatives. Only the second half is reusable — porting the first would add dead
references to directories that do not exist in the standalone repo. `tests/ts-extension-loader.mjs`
is the second half alone, ~40 lines, with a comment saying which half was dropped and why.

---

## 4. Visual parity — **PASSED, in a real browser, in every palette**

§5.1 asked for render evidence. It now exists, and it is reproducible with one command:

```sh
./verify/run-parity.sh          # exit 0 = PIXEL-IDENTICAL, 2 = DIVERGENT, 1 = INCONCLUSIVE
```

### What it measures

`verify/parity/` mounts **three** engines in **one** document, all fed the identical
`buildSyntheticModel(220)` (220 nodes, 329 edges):

| Panel | Source |
|---|---|
| `host` | osionos `packages/graph-engine` — what the app renders today |
| `standalone` | this repo's `src` — the extraction |
| `host2` | osionos' copy **again** — the control |

The control is the point. The force layout is time-driven, so two panels built microseconds apart
sit at different tick counts and their pixels differ *even when the source is identical*. The first
run of this rig measured 66,067 differing pixels between two instances of the same code. A
host-vs-standalone number is meaningless without that floor beside it.

One document means one rAF clock, so the aurora background is phase-aligned and cannot itself be a
source of difference. Geometry is additionally compared **numerically** via `getMinimapData()`
(positions, camera, world bounds) rather than only visually, so a disagreement is a number in world
units instead of an adjective.

### Result

16 combinations = 7 host palettes + the default, each in light and dark:

| Measure | Control (host vs host2) | Signal (host vs standalone) |
|---|---|---|
| Differing pixels, graph layer | 0 / 372,000 | **0 / 372,000** |
| Differing pixels, background layer | 0 / 372,000 | **0 / 372,000** |
| Node positions differing | 0 / 220 (max delta 0) | **0 / 220 (max delta 0)** |
| Camera identical | yes | **yes** |
| Combinations with any difference | 0 / 16 | **0 / 16** |
| Page errors / console errors / failed requests | — | 0 / 0 / 0 |

FNV-1a checksums of both canvases are equal between host and standalone in all 16 combinations.

### The rig is proven capable of failing

A check that cannot fail is not evidence. Perturbing one hex in the standalone's `AURORA_BG_TOP`
(`#1b1a17` -> `#1b1a18`) moves the verdict to `DIVERGENT` with **161,195** differing background
pixels while the control stays at 0 — so the measurement is sensitive, and specific to the copy
that was changed.

### Three false passes, caught and fixed

Each of these reported success while measuring nothing. They are recorded because the failure mode
is not hypothetical.

1. **Blank canvas read as identical.** Pinning `performance.now()` to a constant removed the
   edge-flow animation phase difference, but also froze the reveal stagger at progress 0 — every node
   rendered fully transparent. All 16 palettes compared "identical" on an empty canvas. Fixed with a
   clock that advances once per frame and is shared by all panels, plus an assertion on inked-pixel
   fraction and distinct-colour count that makes a blank layer a hard failure (`BLANK-GRAPH`).
2. **The host's tokens were never loaded.** The generated stylesheet sat outside the dev server's
   root, so the request returned **`HTTP 200 text/html`** — the `index.html` SPA fallback. A 200 is
   invisible to both a console-error listener and a `requestfailed` listener, so nothing reported it.
   All 27 graph tokens resolved to `""` and the engine was silently rendering its own hardcoded
   fallbacks in every palette. The CSS is now injected directly rather than fetched, and token
   resolution is asserted (`TOKENS-UNRESOLVED`).
3. **The palette sweep was not discriminating.** All 16 combinations produced the identical render.
   The sweep now requires the host's own fingerprint to differ between combinations
   (`SWEEP-NON-DISCRIMINATING`), so it cannot silently degrade into one comparison repeated 16 times.
   It currently reports 16 distinct fingerprints for both layers.

A fourth issue affected only the artifact, not the measurement: the harness never loaded
`graph.css`, so `.osio-graph__fg` had no `z-index`, stacked below the background canvas in normal
flow, and was pushed outside the visible cell. Every screenshot showed an empty aurora field. The
comparison reads canvas backing stores through `getImageData`, which no stylesheet can influence, so
the numbers were never affected — but an artifact that misrepresents what it documents is worse than
none, so `graph.css` is now injected too.

### Environment note

The check runs in its own container (the official Playwright image). The VM still has no browser and
cannot install one — `/opt` is not writable and `sudo` needs a tty — so the container is the
environment, not a workaround. `vite` is installed **in the rig** and never added to `package.json`:
the package ships no bundler by design, because a bundler is the consumer's choice.

The host's token *values* are extracted verbatim from `src/app/styles/global.css` by
`verify/extract-tokens.mjs`. That is a faithful reproduction of what the engine sees, because
`core/theme/tokens.ts` reads colours with `getComputedStyle(...).getPropertyValue()` at runtime and
never consumes a Tailwind utility. The palette cascade
(`[data-palette="x"][data-theme="y"]` > `[data-theme]` > `:root`) is preserved by keeping the original
selectors, so the sweep exercises the real cascade rather than a flattened approximation.

### What this still does not prove

- **Interaction**: no click, hover, drag, zoom, selection, focus-dimming or keyboard path is
  exercised. Parity is established for a rendered frame.
- **Live layout convergence**: engines are frozen on `onReady` and compared at that instant, not
  after N ticks of settling. Positions are bit-identical at the freeze point, which is a real
  guarantee, but it is not a statement about long-run convergence.
- **The app's own composition**: `GraphEngineExplorer` is not mounted. The package's `GraphView` is,
  from both copies. App chrome around it is out of scope for a package check.
- **Non-graph code**: nothing outside `src/` was compared.

---

## 5. The one real limitation

`resolveSceneTheme()` (`src/core/theme/tokens.ts`) reads ~27 `--osio-*` custom properties off the
document root at runtime.

**The runbook's §2.4 gets this wrong, and the correction matters.** It says a non-osionos consumer
"gets an unstyled or wrong-colored graph". Not unstyled: *every* read goes through
`read(token, fallback)` → `value || fallback`, and `src/styles/graph.css` mirrors the coupling with
`var(--osio-graph-bg-1, #160c30)`. A standalone consumer renders the engine's hardcoded warm-dark
aurora theme.

The real cost is **silent divergence**: a host that defines the tokens gets its palette, a host that
doesn't gets the baked-in one, and both look intentional. No error, warning, or log distinguishes
them. Theme switching is unavailable standalone — `mode` is inferred from the luminance of
`--osio-graph-bg-1`, so with the fallback it is always `dark`.

This is stated in the new `README.md` under `## Limitations`, with the escape hatch (define the
tokens you care about; each falls back independently). Shipping neutral default token values *in the
package* would remove the coupling, but that is new scope, not extraction, so it was not done.

A second boundary is documented alongside it: `core/layout/layoutBridge.ts:201` builds its worker as
`new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" })`. That form is
portable, but it requires the consumer's bundler to understand `new URL(..., import.meta.url)` as a
worker entry.

---

## 6. osionos is unmodified

```
$ git -C /home/dlesieur/Documents/osionos status --porcelain
A  .claude
M  .gitmodules
?? .opencode/
?? AGENTS.md
?? opencode.json
?? prompt_opencode.md
```

This output is **byte-identical to the Phase 0 preflight baseline** (verified with `diff` at both the
Phase 1 gate and here). Nothing was staged, committed, or reverted in the host repo.

The staged `.claude` submodule and modified `.gitmodules` were **already present at Phase 0** and are
not mine; `AGENTS.md` is mine; the rest predate or postdate this task. Per the runbook's §3 these
were reported rather than stashed or discarded.

---

## 7. Recommended follow-ups — human decisions, deliberately not performed

### 7.1 osionos still carries a full duplicate of the engine's model layer

An audit of the host found that `src/features/second-brain/model/` **hand-mirrors** package
internals, and one copy has already rotted:

| App | Package | State |
|---|---|---|
| `model/graphModel.ts:24-90` (whole data contract) | `core/types.ts:15-89` | field-for-field identical, hidden by a **double assertion** at `src/widgets/graph-explorer/GraphEngineExplorer.tsx:43`: `model as unknown as EngineGraphModel` |
| `model/graphModel.ts:143-193` `indexModel` | `core/model/model.ts:24-64` | duplicate, live in 5 files |
| `model/graphModel.ts:93-122` id builders | `core/model/ids.ts:11-40` | byte-identical **including doc comments** |
| `model/weights.ts:23-43` `applyDegreeWeights` | `core/model/weights.ts:12-26` | byte-identical down to the `clamp(0.2 + 0.8 * …)` expression; live in 5 files |
| **`model/graphModel.ts:125-137` `nodesEqual`** | `core/model/model.ts:10-21` | **ALREADY DIVERGED** — the app copy compares `a.icon === b.icon`; the package's does not |
| `model/palette.ts` | `core/theme/{palette,categorical}.ts` | self-declared hand-mirror; **dead** (barrel re-export with zero importers) |
| `model/selectors.ts:27-52` `neighborhood` | `core/model/neighborhood.ts:9-28` | weaker duplicate; dead in app code, kept alive by one test |

`nodesEqual` is the proof this is worth acting on: a hand-mirror with no test against the original
drifts, and this one already has. The `as unknown as` cast at `GraphEngineExplorer.tsx:43` is what
keeps the drift from being a compile error.

**This was not touched.** It modifies 6+ live app files in a working application, which the runbook's
§0 forbids and Phase 4 does not authorise. The clean end state — delete the app-side mirrors, import
from `@osionos/graph-engine`, drop the double assertion — is a separate, reviewable change.

Also worth noting: `GraphEngineExplorer.tsx:44,110-114,124-168` re-composes what the package's own
`src/react/GraphExplorer.tsx` already does. It *calls* the package's `deriveLegend`/`neighborhood`
rather than reimplementing them, so this is composition duplication, not logic duplication — but the
package appears to already own that component.

### 7.2 Nothing validates the engine's token reads

`scripts/check-ui-token-contract.sh` derives its required set from `packages/osionos-ui/tokens.css`
only. Nothing in `npm run test:quality` checks the graph engine's runtime `getPropertyValue` reads
against `global.css`. Concretely: `--osio-graph-aurora-1..4` are read at `tokens.ts:113` and defined
nowhere — **this one is deliberate and documented** (`tokens.ts:107-111` explains the bands are
derived from `--osio-graph-select`/`--osio-graph-note` so cold palettes aren't washed in terracotta).
But the gate could not tell that apart from a real rot if someone renamed a token tomorrow. Note also
`core/render/background.ts:10` carries a comment contradicting the intentional behaviour
("the palette's own `--osio-graph-aurora-*` hues") — a stale internal doc.

### 7.3 Phase 4 — wire osionos to this as a submodule

Per §2.3 this is mechanically low-risk: the submodule would mount at the same path the folder occupies
today, so `vite.config.ts`, `tsconfig.json` and `eslint.config.js` should all resolve unchanged. If
they don't, that is a real finding, not something to patch by guessing a new path.

**Not started, and it needs from you:** a `devil` verdict, and an explicit remote URL. Nothing in the
runbook authorises `git remote add`, `git push`, or `gh repo create`. Per §2.3, prefer `tar`-ing a
copy of `packages/graph-engine` before the `rm -rf` so you can diff old-vs-new.

### 7.4 Two runbook facts that are wrong

- §2.1 says graph-engine is "60 source files, ~1600–2000 LOC". It is **71 files, 5,819 LOC**.
- §2.1's premise that only `src/` matters is right, but it misses that the host's own
  `tests/canvas/graph-engine.test.ts` is the package's unit test stranded in the wrong repo — which
  the runbook does correctly identify in §2.2.

---

## 8. What "done" means here

| Requirement | Status |
|---|---|
| Standalone repo at `/home/dlesieur/Documents/graph-engine` | done |
| Own `typecheck` / `lint` / `test` green, real runner (`node --test`) | done — 17/17, in a clean container |
| No import reaching outside itself | done — no `@/…`, no `../../` escapes out of the package; the one `new URL(…, import.meta.url)` worker is portable and documented |
| States its one real limitation instead of overclaiming | done — `README.md` § Limitations, and §5 above |
| osionos has zero modified files | done — status byte-identical to the Phase 0 baseline |
| Evidence, not adjectives, for every claim | **done** — §4 ran in a real browser and reports 0 differing pixels across 16 palettes |
| Visual parity (§5.1) | **PASSED** — `./verify/run-parity.sh`, exit 0. Host vs standalone: 0/744,000 pixels differ per palette, 16/16 palettes, geometry bit-identical, 0 page errors. Rig proven sensitive by a mutation test. |
| `reviewer` sign-off (§5.2) | done — see §9 |

---

## 9. Reviewer sign-off (§5.2)

A `reviewer` subagent reviewed the diff-from-copy (everything written in Phase 1, excluding the
untouched copied `src/`) read-only. Verdict: **REQUEST CHANGES**, one blocker. Disposition of every
finding:

| # | Finding | Disposition |
|---|---|---|
| **B1** | **The index was incomplete** — `tests/ts-extension-loader.mjs`, `package-lock.json` and `Dockerfile` were untracked, so the committed tree would have shipped a `test` script pointing at a missing loader and a `Dockerfile` whose `COPY package-lock.json` could not resolve. | **Fixed.** `git add -A`, then verified with `git ls-files --error-unmatch` per file. All 83 files tracked; nothing untracked. This is the class of defect a linter cannot see — the working tree was correct, only the *committed* tree was not. |
| **W1** | `README.md` advertised the boundary contract as "enforced by ESLint" while listing "no `@/…` host-app imports" — but the `@/*` ban was deliberately dropped at extraction, so half the advertised guarantee was convention only. | **Fixed.** The section is now "Boundary contract" and states plainly that the React half is machine-enforced (with the rule named) and the `@/…` half is a convention in this package, with the reason it was dropped. |
| **W2** | `@typescript-eslint/no-explicit-any` was dropped in the port. The host sets it to `warn`, which its `--max-warnings=0` escalates to a hard error — so the standalone package had silently legalized `any`. | **Fixed.** Restored as `"warn"` with a comment. Zero occurrences in `src/`, so the gate is unchanged. |
| **W3** | `README.md` still documented `npm install && npm run check` with no Node version, written before `engines` was added — so the documented path fails on this machine. | **Fixed.** "Standalone usage" now leads with the Node >= 22.6 requirement, explains that `engines` is advisory without `engine-strict`, and makes the Docker one-liner the default path. |
| **W4** | Claimed `motion` and `react-dom` were dead peers, "which is why neither appears in devDependencies". | **Rejected — the premise is false.** `react-dom@19.2.5` *is* in `devDependencies` and in the lockfile (verified). The underlying observation (`src/` imports neither) is true but is not a defect: declaring `react`/`react-dom` as peers is the standard convention for a package that exports React components, since a consumer needs `react-dom` to mount them, and the runbook's own §4.3 template lists both. No change made. |
| NIT | `PARITY.md` is osionos's cutover sign-off (branch `refactor_perf_and_mark`, `@/widgets/graph-explorer`, host gate results) shipped into a standalone repo. | **Fixed by labelling, not deleting** — §4.1 requires the file be copied. A provenance note now says it is host history, that the unit suite moved here, and that only the feature-parity table describes this package. |
| NIT | `.gitignore` is one line; `.dockerignore` lists a not-yet-existing `EXTRACTION_REPORT.md`. | Left. No live risk (`lint` is scoped to `eslint src`, nothing emits `dist/`/`build/`), and `EXTRACTION_REPORT.md` now exists. |
| NIT | The loader rethrows non-`ERR_MODULE_NOT_FOUND` where the host's fell through to `nextResolve`. | Confirmed as an improvement; kept. |

**The reviewer's one UNKNOWN, resolved:** it could not execute the suite (host Node v20.19.2) and
correctly refused to claim "the tests pass" as a result. That is the same `run-safely` discipline
applied properly. It is resolved here by the containerised gate:

```
$ docker build -t graph-engine-check . && docker run --rm graph-engine-check
> tsc -p tsconfig.json --noEmit
> eslint src --max-warnings=0
> node --test --experimental-strip-types --experimental-loader ./tests/ts-extension-loader.mjs tests/*.test.ts
# tests 17
# pass 17
# fail 0
GATE_EXIT=0
```

Independently confirmed by the reviewer: `src/` byte-identical (71 files), the host untouched (no
`packages/` or `tests/` file modified — host `tsconfig.json` is still the 3-line `extends`, host
`tests/canvas/graph-engine.test.ts` still carries the original `state.idToIndex.get("a")` line), zero
host-app coupling in `src/` or `tests/`, `strict` not loosened, no `!` or `eslint-disable` added, and
the React firewall load-bearing (a `--stdin` probe importing `react` into `src/core/` errors and
exits 1).

**Net:** one blocker found and fixed before commit. The extraction was not signed off unmodified.
