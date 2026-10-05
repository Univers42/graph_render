# ADR — graph-hub: a stateful service that plugins feed

- Status: **proposed**, revision 2 re-submitted to the `devil` on 2026-10-05. No hub code
  before a verdict of PROCEED or PROCEED-WITH-CONDITIONS is recorded here.
- Date: 2026-10-05
- Design: `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md`
- Continues: `docs/decisions/server-and-write-path.md`, phase D1 (the store and the write path).

## Context

The user asked on 2026-10-05 for a complete SDK and API: a micro-service plugin system
that tracks any kind of data and shows it as a graph. Service v1 (`server/graph-server`) is
stateless compute: a document in, a snapshot out. Nothing keeps data between requests,
numbers changes or streams them.

The spec was approved under the user's standing full-autonomy instruction, not by an
interactive review. That is a deviation, recorded here.

## Decision

A second service, **graph-hub**, in front of PostgreSQL. Plugins are out-of-process
clients that push records into their own namespace of a workspace. The hub numbers each
change, streams changes over SSE, serves the workspace as an ingest document, and asks
graph-server for layouts over HTTP. The stored model is the ingest contract. The decisions
are H1–H13 in the spec, §2.

## Verdicts

**Revision 1: BLOCK (2026-10-05).** It had already landed on develop (7a3416ac) before the
verdict; that order is a deviation. The 15 conditions and where revision 2 meets each are
in spec §12. Ten statements were found false against the tree:

| # | Revision 1 said | The tree |
|---|---|---|
| 1 | a dangling link is legal | `check_link_cells` (`crates/graph-contract/src/ingest/validate/cells.rs:28`) refuses it |
| 2 | deleting a linked record is harmless | `/layout` would answer 422 |
| 3 | `/layout` calls graph-server plainly | graph-server's `source` defaults to `studio`; the hub must send `contract` |
| 4 | the hub never sends a body graph-server refuses | per-id work caps (413), 422 and 429 remain |
| 5 | the `motor-alone` row stays green | no rows file defines it (`docs/reviews/review-studio.md:201-202`) |
| 6 | the hub reuses graph-server's auth | `auth::check` needs graph-server's `App`; `bearer` is private (`auth.rs:34`) |
| 7 | keys and grants reload atomically through `KeyStore` | `KeyStore` swaps keys alone |
| 8 | `at` is the commit time | it is read before commit; `seq` is the order |
| 9 | tags stay inside a plugin | `tag:<value>` is workspace-wide (`crates/graph-core/src/ingest/build.rs`) |
| 10 | the records it leans on are final | `server-dependencies.md` is "proposed"; `service-api.md:3` still reads "blocked" |

**Revision 2: pending.** Its verdict is recorded here when it comes.

## Agreements owed before slice 2

graph-render-4f owns `server/` and `server/graph-server`. Before slice 2 starts, this
record holds their written agreement to:

- one line added to the `members` list of `server/Cargo.toml`, and the shared
  `server/Cargo.lock`;
- `auth::bearer` made `pub` (their edit), so the hub carries no second Bearer parser.

## Consequences

- `crates/` gains a `hub` module in graph-contract. Every later change to the hub wire voids
  the motor's gate evidence, so the wire freezes once slice 1 lands.
- `server/` gains two members and a database driver. `cargo-deny-server`, `lock-parity` and
  `svc-features` re-run in slices 2 and 3.
- `server-and-write-path.md` deferred "streams". This record lifts that only for an SSE
  change feed. Remote access and TLS stay stop-and-ask: the hub binds loopback by default.
