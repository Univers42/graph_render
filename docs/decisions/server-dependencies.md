# ADR — Dependencies of the server workspace

Status: **proposed** (plan approved by the user 2026-09-29); confirmed or amended by the
`devil` verdict owed before D1. Rung 6 of the minimalism ladder: each new dependency with
what it pulls in, who maintains it and how it is removed.

The motor's allow-list (`crates/`) stays closed. This list applies to `server/` only.

| Crate | Used for | Why a lower rung fails | Removal path |
|---|---|---|---|
| `tokio` | async runtime | std has no async I/O; every HTTP and PostgreSQL client needs one | none while the server is async |
| `axum` | HTTP routing, extractors, SSE | hand-written HTTP/1.1 parsing of untrusted input is a security liability | routes are thin; swap for `hyper` directly |
| `tower-http` | request limits, timeouts, tracing | the same limits rewritten by hand, per route | inline the two layers used |
| `tokio-postgres` | PostgreSQL wire protocol | no std client; `sqlx` pulls a macro layer and compile-time database access | the store is one crate; swap the driver there |
| `redis` | Redis protocol, pub/sub | RESP by hand is small, pub/sub reconnection is not | the cache is one module behind one trait-free struct |
| `serde`, `serde_json` | request and response bodies | already in the root workspace | — |
| `sha2` | cache key, token hash | already in the root workspace | — |

## Rules

- Versions are pinned exactly in `server/Cargo.toml`; `cargo deny` and `cargo audit` are gate
  rows (`deny`, `audit`). A known-vulnerable dependency fails the gate.
- Row `lock-parity`: every crate `graph-core` links resolves to the same version in both
  lockfiles.
- `proptest` for the server and `fast-check` for the TypeScript packages are **not** on this
  list. Property tests for the byte decoder and the ingest reader need one of them; that is
  stop-and-ask item 10.
