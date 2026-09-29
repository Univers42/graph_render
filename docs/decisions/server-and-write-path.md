# ADR — A server and a write path exist, outside the motor

Status: **accepted in principle** (user, 2026-09-29). A `devil` verdict is owed before phase
D1 writes code; its conditions become D1/D2 acceptance criteria.
Supersedes the `graph-server` and "mutation/write path" entries of `prompt.md` §10 "Out of
scope — requires a human decision".

## Context

The user asked for a data model agents read and write (notes, ordinary rows, and an agent's
memory), with PostgreSQL as the system of record and Redis as a cache. Both out-of-scope
items were waiting on exactly that decision.

## Decision

- The **motor keeps no mutation path**. It builds from one canonical ingest document and
  returns a finished snapshot, as today. A write changes stored inputs; the motor rebuilds.
- `server/` is a **separate Cargo workspace** with its own `Cargo.lock`: `graph-store`
  (PostgreSQL) and `graph-server` (axum). It depends on `graph-contract` and `graph-core` by
  path. Nothing under `crates/` depends on it.
- The Rust writer in `graph-contract` is the only producer of canonical ingest bytes. A client
  never computes a cache key and never writes canonical bytes.
- A write is one transaction: bump `workspace.head_seq`, upsert the record with `rev + 1`,
  append one `change` row. `If-Match` mismatch is 409. A replayed idempotency key returns the
  original result.

## Consequences

- A second lockfile can resolve `libm` or `indexmap` differently from the root one, and the
  server would then disagree with the wasm build on bytes. Row `lock-parity` compares the
  two lockfiles on every crate `graph-core` links; the end-to-end row compares digests.
- Not in the first version: streams, locks, agent working memory in Redis, remote access
  and TLS. Each waits for a caller, and the last two are stop-and-ask items.
