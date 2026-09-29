# ADR — Service and browser images are built from Debian packages

Status: **accepted** (user, 2026-09-29). Applies `prompt.md` rule 0.3 to images that rule
did not foresee: a database, a cache and a browser.

## Decision

Every new image is `FROM debian:trixie-slim` plus packages from Debian's own archive. No
`postgres:*`, `pgvector/pgvector`, `redis:*`, `valkey/*`, `mcr.microsoft.com/playwright` or
`browserless/*` base.

| Image | Packages | Version checked 2026-09-29 (`apt-cache policy`, trixie) |
|---|---|---|
| `deploy/chromium.Dockerfile` | `chromium`, `python3`, `fonts-dejavu-core` | 154.0.8037.57, 3.13.5 |
| `deploy/postgres.Dockerfile` (D1) | `postgresql-17`, `postgresql-17-pgvector` | pgvector 0.8.0-1 |
| `deploy/redis.Dockerfile` (D2) | `redis-server` | 8.0.2 |

New images live in `deploy/`, not `docker/`: `docker` is fingerprinted
(`crates/graph-cli/src/fingerprint.rs:21-35`) and a new file there voids gate evidence.

## What this does not do

- It does not pin package versions or the base digest; `docker/*.Dockerfile` does not
  either. A rebuild on another day can pick up a newer Chromium, and perf numbers then
  move for a reason that is not the studio's. `studio-perf` records the browser version in
  every report so the change is visible.
- `node:22-slim` is still used by `scripts/studio.sh` and `scripts/orch/node-slim.sh`. That
  predates this ADR and is not changed by it.
- Redis or Valkey is a stop-and-ask item (D2).
