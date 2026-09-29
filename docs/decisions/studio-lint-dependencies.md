# ADR — Lint dependencies of the studio packages

Status: **accepted** with the plan approved by the user on 2026-09-29 (gate rows `render-lint`
and `studio-lint`, plan §6). Rung 6 of the minimalism ladder: each new dependency with what
it pulls in, who maintains it and how it is removed.

Development dependencies of `app/` only. Nothing here is bundled, and nothing here reaches
`crates/`, the root `package.json` or the evidence fingerprint.

| Package | Version | Used for | Why a lower rung fails | Removal path |
|---|---|---|---|---|
| `eslint` | 9.39.5 | the runner | `tsc` checks types, not the house limits (40 lines, 4 parameters, 300 lines, depth 3) nor the layering | delete `app/eslint.config.js` and the `lint` command |
| `typescript-eslint` | 8.71.0 | type-aware rules (`strictTypeChecked`), the TypeScript parser | a `grep` cannot see an unawaited promise or a cast | same |
| `eslint-plugin-jsx-a11y` | 6.10.2 | accessibility of the chrome (`strict`) | no tool on a lower rung reads JSX | drop the one config entry |
| `eslint-plugin-react-hooks` | 7.1.1 | the rules of hooks | the failure is silent at run time | drop the one config entry |

`app/node_modules` went from 27 to 229 packages. `npm audit` on 2026-09-29: 0 vulnerabilities.
Maintainers: the ESLint team (OpenJS Foundation), the typescript-eslint project, the
jsx-eslint organisation, and the React team.

## What the config enforces

- The house limits and bans: `max-lines` 300, `max-lines-per-function` 40 (blank lines and
  comments not counted), `max-params` 4, `max-depth` 3, no `as` other than `as const`, no
  default export, no enum.
- The layering of the plan (§2) as `no-restricted-imports`:

| Files | May not import |
|---|---|
| `packages/graph-render/**` | the oracle (`src/core`, `src/react`), the motor's SDK, React, the studio |
| `packages/graph-studio/**` | the oracle; the SDK, except from `src/motor/worker.ts`, `src/motor/local.ts` and the `*.motor.test.ts` files |
| `app/src/**` | anything but `packages/graph-studio/src/element.ts` and `src/motor/local.ts` |

## Two rules are configured, not suppressed

- `no-floating-promises` allows `test`, `describe` and `it` from `node:test`: they return a
  promise for the runner, which awaits it.
- `no-confusing-void-expression` allows the arrow shorthand (`() => invalidate(state)`).

## Negative control

A file under `packages/graph-render/src/` importing `react`, `src/core` and the SDK turns the
row red with three `no-restricted-imports` errors (run on 2026-09-29, exit 1).

## What it does not do

Ponytail: the import bans match the text of the import. A path that reaches the oracle through
a re-export in an allowed file is not seen; nothing re-exports it today. The `no-oracle-import`
row (a `grep` for `src/core`) stays as the second check.
