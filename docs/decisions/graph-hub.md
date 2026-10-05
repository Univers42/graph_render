# ADR — graph-hub: a stateful service that plugins feed

- Status: **accepted with conditions**, revision 5: PROCEED-WITH-CONDITIONS from the `devil` on
  2026-10-05. Hub code may start; each slice meets its conditions (spec §12–§16) before it lands.
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
are H1–H15 in the spec, §2.

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
| 9 | tags stay inside a plugin | `tag:<value>` is workspace-wide (`crates/graph-core/src/ingest/build/builder.rs:254-255`, `check_tag`) |
| 10 | the records it leans on are final | `server-dependencies.md` is "proposed"; `service-api.md:3` still reads "blocked" |

**Revision 2: BLOCK (2026-10-05).** Axes: blast radius 3, reversibility 3, cost on failure 4,
confidence 4; worst: cost and confidence. 18 conditions per slice (nine lift the block) and 13
defects. The two that carried the BLOCK: the memory bound contradicted the measured ingest peak
(18.25× the body, `server/graph-server/src/config/slots.rs:22`), and `sync` diffed against the
lossy `/graph`, so it churned forever. The others: resume holes (no epoch, `since > head`
undefined, a watch that could step back), negative controls that could not fire (`drop-record`,
`ack-before-commit`), vetoes across plugins, lock-parity and svc-features scratch copies that break
on new members, the motor linked into the hub, and features of the hub unifying into the shipped
graph-server at the `server/` virtual root (D13). Auth reuse was accepted on four conditions:
`default-features = false` with no forwarded `negctl` or `test-hooks`; the hub rebuilds the 400 for
a second header and the uniform 401; `pub fn bearer` with its first caller, slice confirmed by 4f;
D13 resolved. Spec §13 maps every condition and defect to where revision 3 meets it.

**Revision 3: BLOCK (2026-10-05).** Axes: blast radius 3, reversibility 3, cost on failure 4,
confidence 3; the worst is cost on failure. Three defects carry the block: N1, the multi-event
trigger DDL that PostgreSQL refuses, with `TRUNCATE`, the replica role and a restore bypassing the
epoch, and a sweeper that would move it; N2, `/changes` reading headers and operations in two
`READ COMMITTED` statements, so a prune between them loses operations silently; N6, a memory bound
that does not cover every route. Thirteen more: N3–N5, N7–N16. Revision 2 conditions met: 1, 2, 3,
6, 7, 11, 14, 16, 17, 18; partly met: 4, 5, 8, 9, 10, 12, 13, 15. The claim that H12 guarantees a
readable document was refuted by `motor.rs:18-19`: a 422 also comes from the caller's layout. The
PostgreSQL facts are measured in `docs/measurements/hub-pg-epoch-probe.md`; spec §14 maps every fix.

**Revision 4: BLOCK (2026-10-05).** Axes: blast radius 3, reversibility 3, cost on failure 3,
confidence 3; the worst is confidence. Two defects carry the block: R3, a restore detector that ran
only at start and whose keys no physical restore changes, with point-in-time recovery and promotion
unmeasured; R1, a trigger set that named no tables and a detector bump outside `hub.writer`. Nine
more: R2 (an empty `hub_meta`, a bump outside one transaction), R4 (SSE pages and `PUT /workspaces`
outside the memory bound), R5 (a negative control that could not fail), R6 (millisecond epochs),
R7 (one key holding both writer permits), R8 (a manual-SQL deadlock as a 500), R9 and R11 (two
wrong citations), R10 (`SET LOCAL` outside a transaction). The physical restores are measured in
`docs/measurements/hub-pg-epoch-probe.md` ("Physical restores"); spec §15 maps every fix and the one
deviation (`event: busy` instead of `event: resync`).

**Revision 5: PROCEED-WITH-CONDITIONS (2026-10-05).** Axes: blast radius 3, reversibility 3, cost
on failure 4, confidence 3; the worst is cost on failure. R1–R11 are met, and the `event: busy`
deviation is accepted on its merits. Seven new defects become conditions: S1, a small-gap restore
that passes the LSN compare (fixed by a per-workspace last-seen map checked on every new
connection); S2, an order of high-water reads that bumps a live database under write load (the
high-water snapshotted before the LSN read and lowered only after COMMIT, under a mutex); S3, a
"measured" claim for triggers the probe never set to `ALWAYS` (reworded; a catalog assertion); S4,
a durability premise checked only on the gate's image (`synchronous_commit` on per writer
transaction, `fsync` and `full_page_writes` checked per connection, the flush LSN); S5, stale status
lines; S6, two guard bypasses missing from the Caveat; S7, a deadlock case that could not fail.
Spec revision 5.1 folds the fixes into the body, and its §16 lists the conditions: hub-store 12–18,
hub-api 11, hub-sdk 6, hub-report 5.

## Agreements before slice 2

graph-render-4f owns `server/` and `server/graph-server`. Asked on 2026-10-05 for:

- one line added to the `members` list of `server/Cargo.toml`, and the shared
  `server/Cargo.lock`;
- `auth::bearer` made `pub`, so the hub carries no second Bearer parser.

Their reply, 2026-10-05, verbatim:

> 1. YES to adding server/graph-hub and server/graph-store to `members` and sharing
> server/Cargo.lock, on these conditions:
> (a) Edits to server/Cargo.toml are additive. The [profile.*] panic = "unwind" applies to your
> members too; leave it as is.
> (b) What ships stays the same. `cargo tree --manifest-path server/Cargo.toml -p graph-server -e
> normal,build --locked` must be byte-identical before and after (diff it in the slice report). In
> Cargo.lock, graph-server's existing entries must not change version.
> (c) My floor stays database-free. svc-clippy and svc-test run `--workspace`
> (svc-floor.rows:3-4), so your crates join them. They must pass there without a live Postgres.
> Put any DB test behind its own row and negctl, not in plain `cargo test`.
> (d) The image ships graph-server only. scripts/service.sh:73 builds `--bin graph-server`; keep
> it that way, and keep svc-image (service-image.rows) and its negctls green on the merged tree.
> (e) cargo-deny-server, lock-parity and svc-features stay green with their negctls, as you
> offered.
> 2. YES to `pub fn bearer` (server/graph-server/src/auth.rs:34). Make the one-word change in your
> slice-2 branch, in the same commit as its first caller: no pub without a caller. Keep its doc
> comment.
> Reuse bearer and keys::KeySet::{load,parse,name_of} only. Never reuse auth::check: it carries the
> `any-key` break knob (auth.rs:24) and the compute App.
> This is an auth surface, so the devil's verdict on revision 2 must cover the reuse before it
> lands.

So the `pub` is the hub's edit, made in slice 3 (hub-api, the first caller), not 4f's.
Conditions (a)–(e) and the reuse limit are acceptance criteria of slices 2 and 3.

After the revision 2 verdict, asked about D13, D7, D8 and the slice of `bearer`. Their second
reply, 2026-10-05, verbatim:

> yes to D13, you make the D7 fix, D8 option (i), and bearer goes in slice 3.
> 1. D13: YES, add `default-members = ["graph-server"]` in hub-store. That's a real catch; my (b)
> was blind to virtual-root unification. Make the virtual-root diff the binding check:
> `cargo tree --manifest-path server/Cargo.toml -e features --locked`, with no -p, byte-identical
> before and after. Put it in the slice report next to (b).
> 2. D7: make it in hub-store yourself. Give lock-parity.sh `scratch_setup` and svc-features.sh the
> same shape: loop over the workspace members, copy each one's Cargo.toml and link each one's src
> with the computed relative `..`. Change nothing else. Run all of service-supply.rows on the merged
> tree. The three negctls (`--break`, `--break-version`, `--break-feature`) must still exit 1 for
> their original reason; grep their message, don't trust the exit code alone. Send me the diff
> before it lands.
> 3. D8: (i), with default-features = false.
> Why not (ii): keys.rs:74 calls breaks::on("accept-group-writable"). Without `negctl` it compiles
> to a const false (breaks.rs:11-14), so nothing reaches a hub release build. (ii) would add a
> forwarded negctl feature to a new crate and re-prove service.rows' group-writable control, which
> means more edits on a gated auth surface to save link size.
> Conditions: hub-breaks-off shows neither negctl nor test-hooks on graph-server's edge in the
> release build. The spec states the graph-core and graph-wasm linkage, with a Caveat.
> Reopen trigger: extract graph-keys when a third consumer appears, or when a hub size or
> attack-surface budget refuses graph-core.
> 4. Confirmed: `pub fn bearer` lands in the slice whose commit adds its first caller, i.e. slice 3
> (hub-api). My "slice 2" was wrong.

Revision 3 takes each answer as written: spec H2 and row `hub-virtual-root` (D13), slice 2 step 2
and row `svc-supply` (D7), H1, H8 and row `hub-breaks-off` (D8), §10 slice 3 (`bearer`).

## Consequences

- `crates/` gains a `hub` module in graph-contract. Every later change to the hub wire voids
  the motor's gate evidence, so the wire freezes once slice 1 lands.
- `server/` gains two members and a database driver. `cargo-deny-server`, `lock-parity` and
  `svc-features` re-run in slices 2 and 3.
- `server-and-write-path.md` deferred "streams". This record lifts that only for an SSE
  change feed. Remote access and TLS stay stop-and-ask: the hub binds loopback by default.

## Round 4 (slice 3, Task 8): the `/layout` relay

1. **A motor 429 becomes the hub's own 503 with `Retry-After: 1`**, which is
   `HubApiError::busy_wait()`. §5.2's table says "503 + `Retry-After`" for the motor's
   queue-full row and the plan's test name says the same, while `HubApiError::Busy`'s status is
   429 for a non-zero `retry_after` and 503 only for `busy_wait()`. So the 429 row uses
   `busy_wait()`: the caller waits in the **hub's** queue, which is what a 503 with a
   `Retry-After` already says.
   Caveat: the hub cannot distinguish its own queue-full from a motor admission wait, and
   does not try.
2. **`relay::post` takes the already-open `Document` and an optional `Probe`** rather than
   opening the snapshot itself. The handler admits the `LAYOUTS` permit and opens the snapshot
   before the exchange (§5.2's order: permit, then work), and `layout_never_holds_a_whole_
   document` needs to hand the relay its own counters. `RelayReq.cursor` is read off that same
   `Document`, so it is the snapshot's own position and not a second read.
3. **The `LAYOUTS` permit travels with the response body** (`relay::body::held`). §6's default
   of 1 is what bounds a waiting `/layout` to one snapshot, one xmin horizon and one pool
   connection; a permit freed when the response head was written would let a second upload start
   while the first answer was still streaming. Caveat: a caller that keeps a connection open and
   never reads holds the permit until `GRAPH_HUB_STREAM_DEADLINE_MS` cuts the stream.
4. **`GRAPH_HUB_STREAM_DEADLINE_MS` cuts the answer as well as the request**, and a cut ends the
   stream rather than failing it: the caller sees a short body and re-reads at its cursor, which
   is §6's rule for every streamed route.
5. **The relay builds a `hyper-util` client per call.** Caveat: a fresh TCP connection per
   `/layout`, which §6's table treats as free because `GRAPH_HUB_LAYOUTS` defaults to 1.
6. **`negctl-drop-record` runs two commands**: the byte equality must fail and
   `graph_is_not_affected_by_drop_record` must pass. One command alone would only prove that some
   relay exists, not that `drop-record` is in the relay alone (D4, §5.3).
7. **`GM_HUB_BREAK` is forwarded into the container with `-e GM_HUB_BREAK`** on both control
   rows, like every other control in `scripts/orch/rows/hub.rows`. Without it the variable is set
   on the host only, the break never reaches the test, and the control passes for the wrong
   reason.
