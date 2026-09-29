# ADR — The renderer ports from the oracle, it never imports it

Status: **accepted** (user, 2026-09-29).

## Context

`src/core/**` is the TypeScript oracle: the reference the Rust motor is checked against. The
first studio imports it at runtime from seven files (camera, theme tokens, sprite cache,
aurora background, types), so the oracle ships inside the product and a change to the
reference can change what users see.

## Decision

`packages/graph-render` and `packages/graph-studio` import nothing from `src/`. What is
needed is ported once and owned there:

| Need | Source | Lines | Rung |
|---|---|---:|---|
| camera maths (`zoomAt`, `panBy`, `visibleWorldRect`) | `src/core/camera/{transform,controls}.ts` | 81 | 5 — a small helper we own |
| theme tokens | the studio's own CSS custom properties | — | 2 — the platform |
| node sprites, aurora field | not ported; the renderer draws discs and a flat ground | — | 1 — deleted |

The renderer's only input is snapshot **bytes** (`docs/contract/binary-layout.md`), so the
local path and the server path decode, and hash, the same thing.

## Consequences

- Row `no-oracle-import`: `grep` for `src/core` under `packages/` and `app/src` finds
  nothing. Negative control: a fixture file with such an import turns it red.
- The ported camera keeps the oracle's tests, copied with it.
- The two copies can drift. That is accepted: the oracle is frozen reference code, the
  renderer is a product.
