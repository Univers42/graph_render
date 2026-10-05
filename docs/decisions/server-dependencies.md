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

## Amendment: graph-store (slice 2, 2026-10-05)

The 2026-09-29 table above is unchanged. `graph-store` adds one direct dependency —
`tokio-postgres` — and thirty-eight crates the driver drags in behind it. Rung 6 again: what each
group is for, why the lower rung fails, and how it goes away.

Versions are the ones `server/Cargo.lock` resolves, measured 2026-10-05. Nothing here is a direct
edge of `graph-store`: the store's own `Cargo.toml` names `graph-contract`, `sha2`, `tokio`,
`tokio-postgres` and `futures-util` only.

| Crate group | Pinned | Used for | Why a lower rung fails | Removal path |
|---|---|---|---|---|
| `tokio-postgres` | `0.7.18` | the store's only database driver, over plain TCP with `NoTls` | `sqlx` pulls a macro layer and compile-time database access, and the store must build and link with no database present (condition (c)) | one crate owns it; swap the driver inside `graph-store` alone |
| `postgres-protocol`, `postgres-types`, `fallible-iterator`, `byteorder` | `0.6.12`, `0.2.14`, `0.2.0`, `1.5.0` | the driver's wire layer: message framing, the `FromSql`/`ToSql` traits, its OID-keyed type map | a hand-written PostgreSQL frontend protocol implementation is a security liability, and §5.1's `COPY` and portal paging are protocol features, not SQL ones | they go with the driver |
| `md-5`, `hmac`, `sha2 0.11`, `digest 0.11`, `crypto-common`, `block-buffer`, `hybrid-array`, `cpufeatures` | `0.11.0`, `0.13.0`, `0.11.0`, `0.11.3`, `0.2.2`, `0.12.1`, `0.4.15`, `0.3.1` | SCRAM-SHA-256 and MD5 authentication inside `postgres-protocol`. **This is why a second `sha2` major appears**: the store pins `=0.10.9` (the version graph-server already pins) and the driver's authentication path is on `0.11` | disabling SCRAM server-side does not remove the code, only the negotiation; and a `sha2` in `graph-server`'s closure cannot be moved without breaking the `cargo tree` byte-diff of condition (b) | when the driver drops SCRAM, or when PostgreSQL 18 stops accepting MD5, the whole group leaves with it |
| `base64`, `stringprep`, `unicode-normalization`, `unicode-bidi`, `unicode-properties` | `0.22.1`, `0.1.5`, `0.1.25`, `0.3.18`, `0.1.4` | SASLprep for SCRAM, per RFC 4013 | SASLprep is normalization of a password string; implementing it is exactly the kind of subtle, security-relevant code that must not be a local reimplementation | with SCRAM |
| `rand`, `rand_core`, `getrandom`, `chacha20`, `cmov` | `0.10.3`, `0.10.1`, `0.4.3`, `0.10.2`, `0.5.4` | the driver's SCRAM nonce | the store itself draws no randomness (D-rules: no randomness except the seeded generators), so this is not a store dependency and cannot be traded for a deterministic source | with SCRAM |
| `whoami` | `2.1.3` | the driver's default `application_name` | a dependency whose only job is to read the OS user; the store overrides `application_name` itself when it matters | a driver release that drops the default |
| `tokio-util`, `futures-sink` | `0.7.19`, `0.3.34` | the driver's codec adapter between its stream and `tokio` | hand-adapting the codec is the driver's own extension point, not ours | with the driver |
| `phf`, `phf_shared`, `siphasher` | `0.13.1`, `0.13.1`, `1.0.4` | the driver's query parsing, i.e. recognising statement types without a server round trip | this is a compile-time table over SQL's fixed statement set; nothing in `crates/` provides it | with the driver |
| `parking_lot`, `lock_api`, `scopeguard` | `0.12.5`, `0.4.14`, `1.2.0` | the driver's connection state | the store already depends on `tokio`'s synchronisation; adding a second lock crate for the driver's internals is not a choice available to us | with the driver |
| `log` | `0.4.34` | the driver's tracing; `tracing` is not in the server's closure | `tracing` is not on the list above, so `log` is what a crate that traces can use without a new direct edge | with the driver |
| `async-trait` | `0.1.92` | the driver's `GenericClient` trait | it is the driver's own public trait; the store consumes it through `Client`, so it never names it, but the edge exists in the graph | with the driver |
| `const-oid`, `ctutils`, `tinyvec` | `0.10.2`, `0.4.2`, `1.13.3` | the driver's OID table | a second OID table in `crates/` would be a second source of truth for PostgreSQL's type numbers | with the driver |

Two sentences a reviewer needs, both measured on this tree on 2026-10-05:

- `deny.toml` keeps `multiple-versions = "warn"`, and this slice adds **six** new duplicate reports
  under it: `sha2` (0.10.9 and 0.11.0), `digest` (0.10.7 and 0.11.3), `block-buffer` (0.10.4 and
  0.12.1), `crypto-common` (0.1.7 and 0.2.2), `cpufeatures` (0.2.17 and 0.3.1) and `wasi`
  (0.11.1 and 0.14.7, named by `postgres-protocol` and by `whoami`). The seventh report,
  `hashbrown`, predates the store. `sha2`'s duplicate is deliberate and is the only one this slice
  chose: the store pins the major graph-server already pins, so the virtual-root diff of
  condition (b) stays empty.
- `GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config deny.toml
  check advisories bans licenses sources` **exits 0** on this dependency set, printing
  `advisories ok, bans ok, licenses ok, sources ok`. A `cargo deny` failure on any crate listed
  above is a stop for this slice, not a warning to waive.
