# graph-hub DoD — final report

Written on the `hub-report` branch. develop's tip at the time of writing: `50aae08d`
(`git rev-parse --short origin/develop`); this branch's tip: `c202527f`
(`git rev-parse --short HEAD`). Every `file:line` below was read on that tree and every command
was run on it.

Landing commits: the first-parent subjects on `origin/develop` are all the literal string `updated`,
so a branch name is not greppable from the log (`docs/reports/service-dod.md:8-12`). Each slice is
therefore named by its landing merge, found with `git log --first-parent --merges origin/develop`
and tied to the slice by the paths it brought in (`git diff --stat <merge>^1 <merge>`); the second
parent is cited. Slice 2 landed as two merges and both are cited.

This report covers plan Tasks 1–4 only (`docs/superpowers/plans/2026-10-05-graph-hub-report.md`).
§5 and §6 are the full gate, which the orchestrator runs after the `hub-upload` and `hub-maxheader`
branches land; nothing in this report gates itself, and no gate row was run here.

## 1. Steps

| Step | Verdict | Landing commit | Rows (+ negative control) | Measured vs target |
|---|---|---|---|---|
| 1. hub-contract | met | `cb11ea17`, 2nd parent `14934630` (the `hub-contract` branch tip, `git for-each-ref --points-at 14934630`) | `hub-wire` `scripts/orch/rows/hub-contract.rows:10`, `hub-materialize` `:12`, `motor-lock` `:16`, `codegen --check` `:8` (`scripts/orch/gr cargo run -q -p graph-cli -- codegen --check`), `fmt` `:4`, `clippy` `:5` | §6 of this report |
| 2. hub-store | met | two landings: `d1dae673` 2nd parent `64e867a4` (= `origin/hub-store-changes`, exact ref match), and `ba17fe9e` 2nd parent `004d2c53` | `hub-virtual-root` `scripts/orch/rows/hub-store.rows:29` and `hub.rows:3`, `hub-floor` `hub-store.rows:35` and `hub.rows:5`, `svc-supply` `hub-store.rows:61` and `hub.rows:14`, `hub-seq` `:66`, `hub-epoch-trigger` `:41`, `hub-epoch-detector` `:45`, `hub-epoch-deadlock` `:70`, `hub-idem` `:68`, `hub-changes-snapshot` `:75`, `hub-pg-durability` `:82`, `cargo-deny-server` `:59` | §4 of this report |
| 3. hub-api | met | `c202527f`, 2nd parent `74ef178d` | `hub-authz` `scripts/orch/rows/hub.rows:31`, `hub-reload` `:33`, `hub-limits` `:35`, `hub-events` `:38`, `hub-roundtrip` `:43`, `hub-motor-map` `:45`, `hub-durability` `:47`, `hub-memory` `:49`, `hub-breaks-off` `:1`, `cargo-deny-server` `:19`, `lock-parity` `:21`, `svc-features` `:24` | `docs/measurements/hub-memory.md` |
| 4. hub-sdk | met | `bf61f076`, 2nd parent `7510e7d9` | `hub-sdk-types` `scripts/orch/rows/hub-sdk.rows:13`, `hub-sdk-lint` `:14`, `hub-sdk-unit` `:15`, `hub-sdk-example` `:20`, plus the `negctl-hub-sdk-*` rows `:16-19` and `:21`. The rows `hub-sync` and `hub-sdk` the plan names are **commented out** at `hub-sdk.rows:26-27`, so they did not run | `hub-sdk.rows` |
| 5. hub-report | partial — this report | this file | none of its own (a report gates nothing) | §5 and §6, filled by the full gate |

The landing-commit ties are from the paths each merge brought in:

- `cb11ea17` (`git diff --stat cb11ea17^1 cb11ea17`): `crates/graph-contract/src/hub.rs`,
  `src/hub/{batch,breaks,change,error,ids,manifest,model,prune,schema,strict}.rs`,
  `src/codegen{,/tests,/typescript}.rs`, `generated/hub.d.ts`. Slice 1.
- `d1dae673` (119 files): 55 under `server/graph-store/`, `crates/graph-contract/src/hub/*.rs`,
  `docs/measurements/hub-pg-epoch-probe/`, `docs/contract/hub-schema.json`. Slice 2, first landing;
  the baseline for §4 is its first parent `5372c803`.
- `ba17fe9e` (99 files): `server/graph-hub/{Cargo.toml,src/app.rs,src/relay.rs,src/hooks.rs}`,
  `server/graph-store/src/{writer,retention,sweeper}.rs`, `scripts/orch/{hub-mem.sh,hub-run.sh}`,
  `scripts/orch/rows/hub-store.rows`, `docs/measurements/hub-memory.md`. Slice 2, second landing,
  folded together with unrelated `graph-core`/`graph-cli` lanes work.
- `bf61f076`: `crates/graph-sdk-js/src/{hub.ts,plugin.ts}`, `src/hub/*`, `src/plugin/*`,
  `package.json`. Slice 4.
- `c202527f`: `server/graph-hub/src/{auth/*,events/*,gate/subscribers.rs,etag.rs,breaks.rs}`,
  `scripts/orch/rows/hub.rows`, `docs/decisions/graph-hub.md`. Slice 3, and the tree this report
  is written on.

Neither `hub-contract` nor `hub-api` has a surviving branch ref; the merges above are tied by
their paths, and `14934630` additionally matches a live ref exactly.

## 2. The micro-service

| Item | Verdict | Landing commit | Detail | Where it is |
|---|---|---|---|---|
| The hub binary and its image | met | `ba17fe9e` 2nd parent `004d2c53` | the `graph-hub` crate plus `deploy/hub.Dockerfile` | `server/graph-hub/Cargo.toml:2`, `deploy/hub.Dockerfile:3-10` |
| The keys and grants files | met | `c202527f` 2nd parent `74ef178d` | `GRAPH_HUB_GRANTS_FILE` required, no default; only 0640 or stricter accepted | `server/graph-hub/src/config/env.rs:35` (the allow-list), `server/graph-hub/src/grants.rs:115-116` |
| `graph-store` SQL and the epoch triggers | met | `d1dae673` 2nd parent `64e867a4` | `hub_meta` table, one `ENABLE ALWAYS` trigger per event, the `hub.writer` bypass detector | `server/graph-store/sql/0002_epoch.sql:17`, `:172`, `:52` |
| The SDK `hub.ts` / `plugin.ts` and the `./hub`, `./plugin` exports | met | `bf61f076` 2nd parent `7510e7d9` | `createHub`, `createPlugin` and their re-export blocks | `crates/graph-sdk-js/src/hub.ts:19,27,39`, `src/plugin.ts:20,38,49`, `package.json:12-13` |
| The contract module | met | `cb11ea17` 2nd parent `14934630` | nine `pub mod`s and the generated `hub.d.ts` | `crates/graph-contract/src/hub.rs:29-38`, `:44-54`, `generated/hub.d.ts` (202 lines) |
| The example plugin | met | `bf61f076` 2nd parent `7510e7d9` | registers itself with the hub | `examples/plugins/rows-file/sync.mjs:16` |
| This report | partial | this commit | §1–§4 only; §5–§6 await the full gate | `docs/reports/hub-dod.md` |

### The numbers §6 asks to be measured

All five measured cells come from `docs/measurements/hub-memory.md` on `origin/develop`, which is
slice 3's file and is read here, never written.

| Number | §6 asks | Measured | Ledger row | Cell |
|---|---|---|---|---|
| peak RSS at every cap at once | < 1 GiB under `drun` | **395 MiB** peak `VmHWM` (404 580 KiB), cgroup peak 413 130 752 B, 38 % of the cap | `hub-memory.md:154` (`measure` row; the `control` row at `:155` is OOM-killed, exit 137) | met |
| `F_w` | graph-server's measured 18.25 replaced by the hub's own | **52** ceiling, 50.92 worst of three runs | `hub-memory.md:16` | met — graph-server's value is `server/graph-server/src/config/slots.rs:15` ("1,224,659,341 B, 18.25x the body") and the constant at `:22`; the hub's 52 replaces it and is **2.85× larger**, which is why the budget grew |
| `max_header` | one change header row as read, planning estimate 256 B | see below | `hub-memory.md:18` | pending: branch hub-maxheader |
| last-seen map entry size | planning estimate 256 B | **272 B** ceiling, 263.8 measured at 65 536 entries | `hub-memory.md:17` | met |
| `/layout` upload of a `GRAPH_HUB_MAX_DOC_BYTES` workspace | under graph-server's 10 s body timeout (`body.rs:20`) | see below | — | pending: branch hub-upload |
| the planning total | about 425 MiB plus `base` and the `IO_BUF` terms | **799 MiB** recomputed, plus `base` and three `IO_BUF`; 225 MiB of the 1 GiB left over | `hub-memory.md:128-138` | met, with the caveat the file states at `:139-140`: this is arithmetic over a measured `F_w`, not a whole-process measurement |

`max_header` and the `/layout` upload are being re-measured on branches that have not landed, so
those two cells are pending and this report does not restate a number for them. Note that
`hub-memory.md:18` does carry a `max_header_ceiling_bytes` ledger value of 410 measured 326.2, and
`:113` records that it replaces the 256 B planning estimate; it is left pending here because the
brief for this slice defers both cells to the branch landing, not because it is absent.

The planning total is worth restating plainly: §6 planned "about 425 MiB plus `base` and the `IO_BUF`
terms", and the measured constants give **799 MiB**. The plan's own writers term (`hub-memory.md:130`)
is `2 × 4 MiB × 52 = 416 MiB`, already over half the container. No default shrinks on this
arithmetic (`hub-memory.md:138`).

### The rest of §2

The `GRAPH_HUB_*` defaults the running hub resolves, from `server/graph-hub/src/config/env.rs`:
`MAX_BODY` 4 MiB `:179`, `MAX_BATCH` 10 000 `:180`, `MAX_RECORD_BYTES` 1 MiB `:181`,
`MAX_PLUGIN_BYTES` 16 MiB `:182`, `MAX_DOC_BYTES` 64 MiB `:183`, `RETAIN` 100 000 `:184`,
`RETAIN_BYTES` 512 MiB `:185`, `CHANGES_BYTES` 8 MiB `:186`, `BODY_TIMEOUT_MS` 10 000 `:187`,
`LAST_SEEN` 65 536 `:193`, `MAX_HEADER_BYTES` 16 384 `:204`, `MAX_SUBSCRIBERS` 64 and
`MAX_SUBSCRIBERS_PER_KEY` 8 `:221-222`, `WRITERS_PER_KEY` 1 `:214`, `DB_POOL` 8 `:231`,
`GRAPH_HUB_GRANTS_FILE` required with no default, so it appears only in the allow-list at `:35`
rather than as an assignable default. The runbook at `docs/deploy/hub.md`
documents the ones an operator sets (`:55-58` keys, grants, motor key and URLs; `:104-113` the start
checks; `:119` `MAX_HEADER_BYTES`; `:155` `LAST_SEEN`).

The runbook's epoch bump is `docs/deploy/hub.md:140`, verbatim
`UPDATE workspaces SET epoch = hub_next_epoch();`, run through `hub-pg.sh` at `:144`; `:146`
explains it as `UPDATE epoch_clock SET last = greatest(last + 1, …)`.

The six §5.3 restore-detector cases, from `docs/measurements/hub-pg-epoch-probe.md` (112 lines),
all in the "Physical restores" table at `:81` with the raw output in
`docs/measurements/hub-pg-epoch-probe/{out.txt,out2.txt,out3.txt,run3.sh}` (cited at `:22-24`,
`:62-64`, `:75`):

| Case | Row | What it measured | Where |
|---|---|---|---|
| primary after writes | §5.3 | the detector fires on a restored primary | `hub-pg-epoch-probe.md:83` |
| standby promoted at once | §5.3 | fires before any write lands | `:84` |
| standby promoted after `CHECKPOINT` | §5.3 | fires on the replay | `:85` |
| volume snapshot started on its own | §5.3 | fires on a volume-level restore | `:86` |
| base backup without `recovery.signal` | §5.3 | fires with no signal file | `:87` |
| PITR to an earlier LSN promoted | §5.3 | fires on a point-in-time promotion | `:88` |

Conclusions are at `:90-98`; `:62-65` records that the detector's inputs stay readable under role
`hub`, and `:100` is the file's own "what this does not show".

## 3. Resolved questions

Verdicts, revision 1 → 2 (`docs/decisions/graph-hub.md:69` records the revision 4 BLOCK, `:80-82`
the revision 5 PROCEED-WITH-CONDITIONS, `:78-79` the `event: busy` deviation). Spec line and code
line both cited; "agree" means the code read on `50aae08d` satisfies the condition's own wording.

| Question | What the lines say | Do spec and code agree? |
|---|---|---|
| §12.1 one dangling-reference policy, tested | spec `:790`; `crates/graph-contract/src/hub/{prune,breaks}.rs` | yes — `prune.rs` is the single policy, `hub-materialize` (`hub-contract.rows:12`) tests it |
| §12.2 tag value with `:` → 422 at write | spec `:791`; `crates/graph-contract/src/hub/schema.rs` | yes |
| §12.3 `hub` module in default features, no new dependency, cost stated | spec `:792`; `crates/graph-contract/Cargo.toml`, `crates/graph-contract/src/hub.rs:29-38` | yes — `motor-lock` (`hub-contract.rows:16`) and `codegen --check` (`:8`) are the rows |
| §12.4 row-locked seq, idempotency row in the same transaction | spec `:793`; `server/graph-store/src/writer.rs` | yes |
| §12.5 one `REPEATABLE READ` snapshot, byte-order materialization | spec `:794`; `server/graph-store/src/materialize.rs` | yes |
| §12.6 canonical text, not `jsonb`; `\u0000` → 422 | spec `:795`; `server/graph-store/sql/0001_schema.sql` | yes |
| §12.7 `fsync` and `synchronous_commit` on; crash test | spec `:796`; `server/graph-store/sql/0002_epoch.sql` | yes — `hub-pg-durability` (`hub-store.rows:82`) |
| §12.8 memory in bytes, concurrent materialization cap, measured peak | spec `:797`; `server/graph-hub/src/config/env.rs:211-213` | yes — `hub-memory` (`hub.rows:49`), measured in `docs/measurements/hub-memory.md` |
| §12.9 byte-bounded SSE, defined resume protocol | spec `:798`; `server/graph-hub/src/events/page.rs` | yes — `hub-events` (`hub.rows:38`) |
| §12.10 hub-only keys file, one swapped pair, 0640 grants | spec `:799`; `server/graph-hub/src/grants.rs:115-116`, `config/env.rs:35` | yes — `hub-reload` (`hub.rows:33`) |
| §12.11 authz matrix, no veto across plugins, no existence leak, no auth-off | spec `:800`; `server/graph-hub/src/auth/{grant,need}.rs` | yes — `hub-authz` (`hub.rows:31`) |
| §12.12 `source=contract`, mapped motor answers | spec `:801`; `server/graph-hub/src/relay/map.rs` | yes — `hub-motor-map` (`hub.rows:45`) |
| §12.13 deny, lock-parity, svc-features, `-p graph-hub` image, breaks off | spec `:802`; `scripts/orch/rows/hub.rows:1` | yes — `hub-breaks-off` (`hub.rows:1`), `lock-parity` (`:21`), `svc-features` (`:24`) |
| §12.14 u64 seq in JavaScript; no key in a browser | spec `:803`; `crates/graph-sdk-js/src/hub/cursor.ts` | yes — `hub-sdk-unit` (`hub-sdk.rows:15`) |
| §12.15 no `motor-alone` claim; deferrals lifted in writing; 4f agreement recorded | spec `:804`; `docs/decisions/graph-hub.md:80-82` | not checked in this slice — §6 of the spec (`:680-691`) is the deferral list and this report does not write §6 |
| §13.1–18 conditions, D1–D13 | spec `:806-841`; `docs/decisions/graph-hub.md:69-78` | yes where a row exists. The `[L]` conditions map to `hub-materialize` (`keep-cells`), `hub-seq`, `hub-idem`, `hub-epoch*`, `svc-supply`, `hub-virtual-root`, `hub-memory`, `hub-authz`, `hub-events`, `hub-breaks-off`, `hub-sync`. Conditions 17 ("every row and negative control on the landing commit") and 18 ("record the verdict") are **this report's** conditions and are §5/§6, not yet met |
| §14(a).4, .5, .8, .9, .10, .12 (the six partly-met revision 2 conditions) | spec `:855-861` | yes — `:856` fixes `default-members`, `:854` the manifest lock order, `:859` `GRAPH_HUB_LAYOUTS`, `:860` the `M` total, `:861` the §5.2 status map. The plan's "nine" counts the rows differently; the spec text at `:851` says **eight**, and its table has six [L] rows plus the two prose cases, so the count is not load-bearing here |
| §16(b) hub-sdk condition 6 | spec `:993`; `crates/graph-sdk-js/src/plugin/sync.ts` | met — the adapter's `deleted` maps to a delete when stored and is dropped otherwise (the `N14` fix, commit `6c8a2884`). Note the row the spec names, `hub-sync`, is **commented out** at `hub-sdk.rows:26`, so the end-to-end case did not run; `hub-sdk-unit` (`hub-sdk.rows:15`) covers the unit half |

### The revision 3 → 4 condition (b) diff

Every §14(b) defect `N1`–`N16`, with the spec's own fix text at `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md:872-887`
(`H14` at `:888`). `Commit` is the first commit touching the named path that is not `updated`-noise,
found with `git log --oneline 7fbcfa6d..c202527f -- <path>` — the range spans all four slice landings,
not just the post-slice-2 window, so slice 1 and 2 fixes are attributed correctly.

| Defect | Fixed by (the spec's own text) | Commit | Row that proves it | Verdict |
|---|---|---|---|---|
| N1 trigger DDL refused; `TRUNCATE`, replica and restore bypass | one trigger per event plus `TRUNCATE`, `ENABLE ALWAYS`; `epoch_clock` floor; `hub.writer` on the batch, manifest PUT, workspace create and sweeper; `hub_meta` restore detector; runbook and Caveat; reverse check in `hub-epoch` (§5.3, §8) | `b24579c1` (`server/graph-store/sql/0002_epoch.sql`, +29 lines) and `b82f2260`; the `hub.writer` write paths at `763f01c6` and `85f26354` | `hub-epoch-trigger` `hub-store.rows:41`, `hub-epoch-detector` `:45`, `hub-epoch-deadlock` `:70` | met |
| N2 `/changes` headers without operations | one `REPEATABLE READ, READ ONLY` transaction; stored `ops` count checked (`§5.1, §5.3, §8`) | `64e867a4` (`server/graph-store/src/changes/page.rs`, +243) | `hub-changes-snapshot` `hub-store.rows:75` | met |
| N3 422 from the caller's layout read as a hub defect | 422 split on `error` (§5.2) | `7b2feb89` | `hub-motor-map` `hub.rows:45` | met |
| N4 408 and 503 retried | 408 → 502 `MotorBodyTimeout`; 503 relayed without `Retry-After`; the SDK never retries `/layout`; the upload measured in slice 3 (§5.2, §5.3, §7, §11) | `7b2feb89` | `hub-motor-map` `hub.rows:45`; the measured upload is the `pending: branch hub-upload` cell in §2 | met for the status map; the measurement is pending |
| N5 `/layout` holds a snapshot while graph-server queues | `GRAPH_HUB_LAYOUTS` = 1 and a Caveat (§5.3, §6) | `7b2feb89` | `hub-motor-map` `hub.rows:45` | met |
| N6 M not total | every route under `READS` or `LAYOUTS`; connection and header limits; the anti-join portal; the hash compare; manifests streamed (§5.1, §5.3, §6) | `cc787956` | `hub-limits` `hub.rows:35` | met |
| N7 `hub-memory` negative control cannot fail | sized from the measured `F_w`, `--memory 1g`, a barrier (§8) | `4a05a967` (`scripts/orch/hub-mem.sh`) | `hub-memory` `hub.rows:49`, control `negctl-hub-memory` | met — the control row is OOM-killed at exit 137 (`hub-memory.md:155`) |
| N8 `doc_bytes` misses the head, the tail and manifests | head and tail lengths; the manifest PUT and the workspace create keep it (§4, §6) | `763f01c6` | `hub-seq` `hub-store.rows:66` | met |
| N9 manifest PUT lock order | workspace row first; create by `ON CONFLICT DO NOTHING` (§4) | `763f01c6` | `hub-seq` `hub-store.rows:66` | met |
| N10 virtual-root checked once | slices 2, 3 and 5; `client-legacy` grepped (§8, §10) | `e2851d3c` (`server/Cargo.toml`, +3 — the `default-members` line) | `hub-virtual-root` `hub-store.rows:29` and `hub.rows:3` | met — §4 of this report runs the slice-5 half; the `client-legacy` grep is recorded there |
| N11 `sequence-seq` cannot fail | removes the lock, delays odd commits (§8) | `79cdd47c` (`server/graph-store/tests/writer/sequence.rs`) | `hub-seq` `hub-store.rows:66` | met |
| N12 byte order assumed | `initdb --encoding=UTF8 --locale=C` and a start check (§6, §10) | `89b1e0cb` (`deploy/postgres.Dockerfile`, +30) | `hub-start-check`, inside `hub-pg-durability` `hub-store.rows:82` | met |
| N13 no body timeout | `GRAPH_HUB_BODY_TIMEOUT_MS`, 408 (§6) | `c3e22ad8` (`server/graph-hub/src/body.rs`, +74) | `hub-limits` `hub.rows:35` | met |
| N14 adapters' `deleted` unmapped | a delete when stored, else dropped (§7, `hub-sync`) | `6c8a2884` (`crates/graph-sdk-js/src/plugin/sync.ts`) | **not claimed met** — the row the spec names, `hub-sync`, is commented out at `hub-sdk.rows:26`; `hub-sdk-unit` `hub-sdk.rows:15` covers the unit half only |
| N15 unlisted graph-server statuses | the default row, 502 `MotorError` (§5.2) | `7b2feb89` | `hub-motor-map` `hub.rows:45` | met |
| N16 records paging not a snapshot | `plugin_seq` per page, `If-Match`, start over on 412 (§5.2, §7, `hub-sync`) | `64e867a4` (`server/graph-store/src/records.rs`) | **not claimed met** — same reason as N14; no live row proves the restart on 412 |
| H14 byte-equality precondition | hub-contract condition 1 (§10 slice 1) | `eb74797b` (`crates/graph-contract/src/hub/tests/materialize/order.rs`) | `hub-materialize` `hub-contract.rows:12` | met |

Two of the sixteen (`N14`, `N16`) are **not claimed met**, and for the same reason: the spec's own
proving row `hub-sync` is commented out in `scripts/orch/rows/hub-sdk.rows:26-27`, so the end-to-end
paging and delete-mapping cases did not run on the landing commit. The code fixes are present and
named above; what is missing is the row that would prove them. Re-enabling `hub-sync` is the fix,
and it belongs to whoever owns `hub-sdk.rows`.

### The §15(b) deviation, accepted in revision 5

The verdict's fix sent `event: resync` on a mid-stream pool timeout. Revision 5 sends `event: busy`
instead. The spec's own words, at `:938-939`: "Revision 5 sends `event: busy` instead: the cursor is
still valid, and a resync makes every subscriber read the whole `/graph` while the pool is already
short". The ADR accepts it on its merits (`docs/decisions/graph-hub.md:81-82`, and names it as the
one deviation from the verdict's fix at `:78-79`). The rows that pin it are `hub-events`
(`scripts/orch/rows/hub.rows:38`, the `busy` cases, §16(b) hub-api condition 11 at `:996`) and
`hub-sdk` — whose live row is `hub-sdk-unit` `hub-sdk.rows:15`, since `hub-sync` is commented out.

This is a **deviation from the verdict's fix, accepted in revision 5**, not a defect found here.

## 4. Open review items

"Open" here means: no row in the review is marked closed or fixed, and reading the cited code on
`50aae08d` shows the defect still present. Counts: **high 0, medium 0, low 0**, plus **0 process
conditions** that no code change can close.

| Review | Verdict | Id | Severity | Finding |
|---|---|---|---|---|
| — | — | — | — | none |

The reviews run on the hub: **none**. There is no `docs/reviews/graph-hub.md` and no successor.
`docs/reviews/` holds 15 files (`review-bundle-unify`, `review-core-base`, `review-core-post`,
`review-gates`, `review-harness-sdk`, `review-host-api`, `review-host-columns`,
`review-layout-force`, `review-layout-rest`, `review-layout-tree`, `review-studio`,
`review-svc-r2`, `review-svc-r3`, `rv-sg-spiral3d`, `rv-sg-sugiyama`) and only two of them match
`hub` case-insensitively, both false positives: `docs/reviews/review-core-base.md:157` ("Tag hubs get
`version: 0.0`", F-102, about tag nodes, not this hub) and `docs/reviews/rv-sg-spiral3d.md:27`
("ForceAtlas2BarnesHut", S3-8). The hub's review is the `devil` verdicts in
`docs/decisions/graph-hub.md`, and the condition tables in spec §12–§16, which §1 and §3 above
report against; those are the findings this slice is accountable for. Nothing is listed as open
because the (b) diff in §3 has 14 of 16 met and the two exceptions are `not claimed met` rows, not
open code defects, and each is recorded there.

### The virtual-root diffs

The baseline is `5372c803`, the first parent of slice 2's first landing `d1dae673`
(`git rev-parse d1dae673^1`). It was materialized read-only with `git archive 5372c803` into
`target/hub-dod/base-src/`; `git worktree add` is denied in this worktree, and `scripts/orch/gr`
mounts the current git top-level at `/w`, so the baseline has to live inside the tree for the same
`gr` command to read it.

**Slice 2's virtual-root diff** — §10 slice 2 step 1, spec `:707-708`. The command, both trees, all
through `scripts/orch/gr`:

```sh
scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked > target/hub-dod/vr-base.txt   # baseline 5372c803
scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked > target/hub-dod/vr-final.txt  # 50aae08d
diff target/hub-dod/vr-base.txt target/hub-dod/vr-final.txt | wc -l
```

| | Value |
|---|---|
| exit (both trees) | 0 |
| baseline lines | 364 (`sha256 5328cfa545da4c0ab9a649b79af98eff079aba20ba159e4b92d700e97694ba98`) |
| final lines | 364 (`sha256 adce519c6c944954560e7580f8a6e16dac7c2f12da12220deb22499d9da8acd2`) |
| raw `diff` | 26 lines, 1118 bytes, exit 1 |
| every raw diff line is a path prefix | `/w/target/hub-dod/base-src` vs `/w` — an artifact of where the baseline was unpacked, not a dependency change |
| **diff after normalizing the prefix** | **empty — 0 lines, 0 bytes, exit 0** |

So the virtual root is **unchanged** across slice 2, and the raw 26-line difference is entirely my
scratch layout. That is the pass (`plan Decisions recorded here` 3: a diff with no output is
`empty`).

**The negative control** (§8 `hub-virtual-root`, spec `:650`): the same tree with `default-members`
removed, which lets `graph-store` and `graph-hub` unify into the virtual root.

```sh
cp -r target/hub-dod/base-src target/hub-dod/negctl-src && sed -i '10d' target/hub-dod/negctl-src/server/Cargo.toml
scripts/orch/gr cargo tree --manifest-path target/hub-dod/negctl-src/server/Cargo.toml -e features --locked > target/hub-dod/vr-negctl.txt
```

| | Value |
|---|---|
| exit | 0 |
| control lines | **661**, against 364 for the real root |
| diff vs the baseline | 1016 lines — the control fires |
| graph-server's own subtree changes | **yes**: 357 lines alone → 657 inside the unified tree, and its feature multiset gains `hyper-util` `client`, `hyper-util` `client-legacy`, `hyper` `client`, `graph-store` `default`, `graph-contract` `negctl`, `tokio-postgres` `runtime`, `sha2` `alloc`/`default`/`oid`, `md-5`, `hmac`, `chacha20` `rng`, `rand` and friends |

**The injected feature name, which spec `:650` and `:991` (§16(b) hub-store condition 18) make
mandatory: `hyper-util`'s `client-legacy`.** It is requested by
`server/graph-hub/Cargo.toml:23` (`features = ["tokio", "client-legacy", "http1"]`), it is absent
from the baseline graph-server subtree (count 0 in `vr-base.txt`), and it appears exactly once in
the control tree (`vr-negctl.txt:408`) and once inside the unified graph-server subtree. The
`tokio` features are *not* the injected set: the graph-store edge's `rt`, `sync` and `time`
(`server/graph-store/Cargo.toml:15`) are already enabled by graph-server's edge
(`server/graph-server/Cargo.toml:29`), which the manifest itself says at `:13-14`, and the diff of
`tokio` feature *names* between the two graph-server subtrees is empty. I record this because
`:650` names `tokio` as the carrier and the measured carrier is `client-legacy`; the control's
purpose — graph-server's subtree must differ from the baseline — is served either way.

**Slices 3 and 5, the `client-legacy` grep** (§8 and §10 slice 3, spec `:650`, `:731`):

```sh
scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked | grep -c 'hyper-util .*client-legacy'
```

| Tree | `client-legacy` in the virtual root (no `-p`) | With `-p graph-hub` |
|---|---|---|
| baseline `5372c803` | 0 | — |
| slice 3 (`c202527f`, this landing commit) | 0 | 1 |
| slice 5 (this report, `c202527f`) | 0 | 1 |

Slice 3 is **not** `not claimed met`: the count the row asserts is 1 in the `-p graph-hub` tree and
0 in the virtual root, and both hold on the landing commit, which is exactly what
`scripts/orch/rows/hub.rows:3` (`hub-virtual-root`) checks. Slice 5 re-runs the same grep on the same
commit and gets the same two numbers.

**graph-server's lock entries** (spec `:650` conditions 2, 3, 5; "no version change in graph-server's
lock entries"):

```sh
scripts/orch/gr cargo tree -p graph-server --manifest-path target/hub-dod/base-src/server/Cargo.toml -e normal,build --locked
scripts/orch/gr cargo tree -p graph-server --manifest-path server/Cargo.toml -e normal,build --locked
git diff --stat 5372c803 c202527f -- server/Cargo.lock
```

| | Value |
|---|---|
| both trees | exit 0, 131 lines each |
| normalized diff | **empty, 0 lines, exit 0** |
| `git diff --stat 5372c803 c202527f -- server/Cargo.lock` | **no output, exit 0** |
| `git diff` byte count on that lock | 0 |

`server/Cargo.lock` is byte-identical between the pre-slice-2 baseline and this landing commit, so
no version anywhere in it changed — graph-server's lock entries trivially keep their versions. Note
the row `hub-virtual-root` (`scripts/orch/rows/hub.rows:3`) pins its own baseline as `f1a23521`, the
pre-slice-1 tree, not `5372c803`; I used `5372c803` because the job asks for the commit before slice
2's first landing, and both are recorded here so the two baselines can be compared.

**Not run here:** the `hub-virtual-root` row itself, its `negctl-hub-virtual-root` control, and every
other §8 row. Those are §5, which the full gate runs on the landing commit.

## 5. The full gate

Filled by the full gate on the landing commit (plan Tasks 5–6).

## 6. What is not done

Filled by the full gate on the landing commit (plan Tasks 5–6).