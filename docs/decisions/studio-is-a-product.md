# ADR — The studio is a product, and the motor still stands alone

Status: **accepted** (user, 2026-09-29, plan `vectorized-sleeping-fairy`).
Supersedes `prompts/ONBOARDING.md` §8 ("It is a conformance viewer and debug console, not a
product").

## Context

The first studio (`app/`, develop `0d4f9a7`) was built as the throwaway viewer §8 describes.
The user then asked for the opposite: an Obsidian-style interface with a floating console,
embeddable as a plugin in a web app, a webview desktop app and a native host, and usable on
its own. Measured defects of the first studio: `docs/measurements/studio-perf-baseline.md`.

## Decision

| Layer | Path | Depends on | Must not depend on |
|---|---|---|---|
| motor | `crates/*` | as today | `packages/`, `server/`, `app/`, `deploy/` |
| render | `packages/graph-render` | nothing at runtime | React, the SDK runtime, `src/`, `fetch` |
| studio | `packages/graph-studio` | graph-render, the SDK (worker only), React | `src/`, SQL, Redis |
| host 1 | `app/` | graph-studio | anything else |

What §8 protected is kept as a gate row rather than as a status: the studio still consumes
**only the published SDK**, and row `motor-alone` runs the root merge floor (fmt, clippy
`-D warnings`, `cargo test --workspace --no-fail-fast`) in a worktree with `app packages
server deploy` removed. If that row is red, the motor has grown a dependency on its viewer.

## Consequences

- New TypeScript lives under `packages/`, each package with its own manifest and lockfile.
  The root `package.json`, `package-lock.json` and `tsconfig.json` are fingerprinted
  (`crates/graph-cli/src/fingerprint.rs:21-35`), so touching them voids gate evidence.
- The studio gets its own gates (`scripts/studio.sh check`, `scripts/studio-perf.sh`) and its
  own phase reports. It never takes the host gate lock.
- Not decided here: which webview host and which native host come first (stop-and-ask, H2/H3).
