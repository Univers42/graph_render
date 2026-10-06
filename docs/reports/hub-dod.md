# graph-hub DoD — final report

Written on the `hub-report` branch in two passes. §1–§4 were first written with develop at
`50aae08d` on slice 3's landing tree `c202527f`; this pass merged develop at `fbd696f7`
(`git rev-parse --short origin/develop`), after the `hub-maxheader`, `hub-upload` and `hub-live`
landings, and §5 ran on that merged tree. Every cited path was checked with
`git diff --numstat 50aae08d fbd696f7 -- <path>`, and the ones that changed are handled as follows:

- Spec lines are cited at their `c202527f` numbers. The spec gained 8 lines before `:588` since: 4
  at `:568-587` in `hub-maxheader` (`611c9c01`) and 4 at `:325-574` in `hub-upload` (`c07227f4`,
  landed by `5c4f10e4`). So every spec line this report cites past `:587` is 8 higher on `fbd696f7`
  (checked by hashing `:650`, `:672`, `:678`, `:680`, `:688`, `:690`, `:790`, `:804`, `:872`, `:938`
  and `:993` on `c202527f` against the line 8 below on `fbd696f7`: all equal).
- `docs/measurements/hub-memory.md` gained its change-header and upload sections; its citations
  below are at their `fbd696f7` lines.
- `scripts/orch/rows/hub.rows` gained rows at `:51-52` only, and `hub-sdk.rows` uncommented its live
  rows at `:26-27`; no other line cited from either moved.
- `server/graph-hub/src/config/env.rs` changed one line, the `GRAPH_HUB_FETCH_ROWS` default at
  `:192` (32 → 4096).
- `crates/graph-sdk-js/src/{hub/,plugin/sync.ts}`, `server/graph-store/src/materialize.rs`,
  `server/graph-hub/src/relay/body.rs` and `scripts/orch/hub-mem.sh` changed and are cited by path
  only, except in the §3 deviations, whose line numbers are at `fbd696f7`.
- `server/graph-hub/src/{main.rs,config/check.rs}` changed in `fbd696f7` (migrations at start,
  deviation (a)); cited at `fbd696f7` lines.

Landing commits: the first-parent subjects on `origin/develop` are all the literal string `updated`,
so a branch name is not greppable from the log (`docs/reports/service-dod.md:8-12`). Each slice is
therefore named by its landing merge, found with `git log --first-parent --merges origin/develop`
and tied to the slice by the paths it brought in (`git diff --stat <merge>^1 <merge>`); the second
parent is cited. Slice 2 landed as two merges and both are cited.

This report covers plan Tasks 1–6 (`docs/superpowers/plans/2026-10-05-graph-hub-report.md`).
The report itself gates nothing; §5 is the full gate, run once on the merged tree after the
`hub-upload`, `hub-maxheader` and `hub-live` branches landed, one rows file at a time under
`scripts/orch/timed`.

Landings after slice 3, tied by their paths the same way (`git log --first-parent c202527f..fbd696f7`;
the ones without a second parent are fast-forwarded commits):

| Branch | Develop commits | What they brought in |
|---|---|---|
| `hub-maxheader` | `03f09c02`, `3c332ce2`, `611c9c01`, `50aae08d` | `graph_store::changes::heads`, `server/graph-hub/tests/memory/heads.rs`, the `hub-max-header` row, `hub-memory.md` "The change header" |
| `hub-upload` | `b549d92e`, 2nd parent `5c4f10e4` | `server/graph-hub/src/relay/body.rs` (pages cut at `CHANGES_BYTES`), `GRAPH_HUB_FETCH_ROWS` 4096, `scripts/orch/hub-mem-upload{,-run}.sh`, the `hub-upload-timeout` row, `hub-memory.md` "The /layout upload" |
| SDK subscribe fixes | `185178bc`, `439d33c5`, `fd84a7b2` | `crates/graph-sdk-js/src/hub/*` (deviation (b) in §3) |
| `hub-live` | `572b46be`, `ac8f53c2`, `fbd696f7` | `scripts/orch/hub-live.sh`, `crates/graph-sdk-js/test/live/*`, the live `hub-sync` and `hub-sdk` rows, `graph_store::migrate` at start (deviation (a) in §3) |

## 1. Steps

| Step | Verdict | Landing commit | Rows (+ negative control) | Measured vs target |
|---|---|---|---|---|
| 1. hub-contract | met | `cb11ea17`, 2nd parent `14934630` (the `hub-contract` branch tip, `git for-each-ref --points-at 14934630`) | `hub-wire` `scripts/orch/rows/hub-contract.rows:10`, `hub-materialize` `:12`, `motor-lock` `:16`, `codegen --check` `:8` (`scripts/orch/gr cargo run -q -p graph-cli -- codegen --check`), `fmt` `:4`, `clippy` `:5` | not measured — no `docs/measurements/*.md` file covers slice 1 |
| 2. hub-store | met | two landings: `d1dae673` 2nd parent `64e867a4` (= `origin/hub-store-changes`, exact ref match), and `ba17fe9e` 2nd parent `004d2c53` | `hub-virtual-root` `scripts/orch/rows/hub-store.rows:29` and `hub.rows:3`, `hub-floor` `hub-store.rows:35` and `hub.rows:5`, `svc-supply` `hub-store.rows:61` and `hub.rows:14`, `hub-seq` `:66`, `hub-epoch-trigger` `:41`, `hub-epoch-detector` `:45`, `hub-epoch-deadlock` `:70`, `hub-idem` `:68`, `hub-changes-snapshot` `:75`, `hub-pg-durability` `:82`, `cargo-deny-server` `:59` | §4 of this report — the virtual root is unchanged and `server/Cargo.lock` is byte-identical to the baseline |
| 3. hub-api | met | `c202527f`, 2nd parent `74ef178d` | `hub-authz` `scripts/orch/rows/hub.rows:31`, `hub-reload` `:33`, `hub-limits` `:35`, `hub-events` `:38`, `hub-roundtrip` `:43`, `hub-motor-map` `:45`, `hub-durability` `:47`, `hub-memory` `:49`, `hub-breaks-off` `:1`, `cargo-deny-server` `:19`, `lock-parity` `:21`, `svc-features` `:24` | `docs/measurements/hub-memory.md`: peak RSS 395 MiB of the 1 GiB cap, §2 of this report |
| 4. hub-sdk | met, with two ruled deviations | `bf61f076`, 2nd parent `7510e7d9`; the live rows in `hub-live` (`ac8f53c2`, `fbd696f7`) | `hub-sdk-types` `scripts/orch/rows/hub-sdk.rows:13`, `hub-sdk-lint` `:14`, `hub-sdk-unit` `:15`, `hub-sdk-example` `:20`, plus the `negctl-hub-sdk-*` rows `:16-19` and `:21`. The live rows `hub-sync` `:26` (nested `hub-sync.rows:7`, control `:8`) and `hub-sdk` `:27` (nested `hub-sdk-live.rows:8`, control `:9`) were commented out at slice 4's landing and run since `ac8f53c2`; deviations (a) and (b) in §3 are what they found | `hub-sdk.rows`; the live rows' verdicts are in §5 |
| 5. hub-report | met | this file | none of its own (a report gates nothing); §5 runs every row of `scripts/orch/rows/{hub-contract,hub-store,hub,hub-sdk,service-supply,svc-floor}.rows` | §2 and §4 measured here; §5 is the full gate, 149 rows and 149 PASS (68 of them controls, all red as required); §6 is what is not done |

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
| This report | met | this commit | §1–§6 | `docs/reports/hub-dod.md`; §5's evidence is `$GM_SCRATCH/evidence/hub-report/<logdir>/summary.txt` and one `<row>.log` per row |

### The numbers §6 asks to be measured

Every measured cell comes from `docs/measurements/hub-memory.md` on `fbd696f7`, which slices 3,
`hub-maxheader` and `hub-upload` wrote and this report only reads.

| Number | §6 asks | Measured | Ledger row | Cell |
|---|---|---|---|---|
| peak RSS at every cap at once | < 1 GiB under `drun` | **395 MiB** peak `VmHWM` (404 580 KiB), cgroup peak 413 130 752 B, 38 % of the cap; §5's run measured **431 MiB** (441 368 KiB), cgroup peak 450 682 880 B, 42 % of the cap | `hub-memory.md:170` (`measure` row; the `control` row at `:171` is OOM-killed, exit 137); §5's run `gate-dod-hub-rerun/hub-memory.log:129`, its control `negctl-hub-memory.log:30` (`oom_killed=true`, exit 137) | met — the file's 395 MiB and the gate's 431 MiB are two runs of one test; both are under the cap with 58 % or more left |
| `F_w` | graph-server's measured 18.25 replaced by the hub's own | **52** ceiling, 50.92 worst of three runs | `hub-memory.md:16` | met — graph-server's value is `server/graph-server/src/config/slots.rs:15` ("1,224,659,341 B, 18.25x the body") and the constant at `:22`; the hub's 52 replaces it and is **2.85× larger**, which is why the budget grew |
| `max_header` | one change header row as read, planning estimate 256 B | **410 B** ceiling; 234.1, 326.2 and 280.1 B per header over three runs (the last is gate row `hub-max-header`) | `hub-memory.md:18`, runs at `:122-124` | met — the ceiling replaces the 256 B estimate (`:126`), and §6's total uses 410 B (`:138`, `:149`) |
| last-seen map entry size | planning estimate 256 B | **272 B** ceiling, 263.8 measured at 65 536 entries | `hub-memory.md:17`, `:106` | met; re-checked against the 272 B ceiling by `the_last_seen_map_entry_is_the_planned_size` (`server/graph-hub/tests/memory.rs:78`, the assert at `:84-87`) inside `hub-floor` (`hub.rows:5`, `hub-store.rows:35`) and `svc-test` (`hub-contract.rows:21`, `svc-floor.rows:4`), all PASS in §5; the gate does not print the number (no `--nocapture`), so §5 adds no new measurement of it |
| `/layout` upload of a `GRAPH_HUB_MAX_DOC_BYTES` workspace | slowest of five under 8 000 ms, two seconds under graph-server's 10 s `GRAPH_BODY_TIMEOUT_MS` (`hub-memory.md:217-222`) | **2 773 ms** slowest, median 2 605, over a 67 108 842 B document of 745 633 records; no 408 at the motor | `hub-memory.md:25-30`, results at `:318-325` | met — before the fix the same upload failed three runs (slowest 8 133, 8 482 and 9 868 ms, `:359`); the cause and the fix are `:290-316` |
| the planning total | about 425 MiB plus `base` and the `IO_BUF` terms | **655 MiB** recomputed, plus `base` and three `IO_BUF`; 369 MiB of the 1 GiB left over | `hub-memory.md:144-154` | met, with the caveat the file states at `:155-156`: this is arithmetic over a measured `F_w`, not a whole-process measurement |

The planning total is worth restating plainly: §6 planned "about 425 MiB plus `base` and the `IO_BUF`
terms", and the measured constants give **655 MiB**. The writers term (`hub-memory.md:146`) is
`2 × 4 MiB × 52 = 416 MiB` on its own, two thirds of the total. The upload fix lowered the reads arm
from 348 MiB to 204 MiB by cutting a document page at `CHANGES_BYTES` (`:140-142`); before it the
total was 799 MiB. No default shrinks on this arithmetic (`:154`).

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
The five source files changed since (`git diff --numstat 50aae08d fbd696f7`: `materialize.rs` +4/−1,
`sync.ts` +41/−6, `subscribe.ts` +51/−10, `config/check.rs` +19, `main.rs` +7/−2) were re-read on
`fbd696f7`. `materialize.rs` gained one field, `page_bytes: store.config().changes_bytes`, and a doc
comment; the single `REPEATABLE READ` snapshot of §12.5 is untouched.

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
| §12.15 no `motor-alone` claim; deferrals lifted in writing; 4f agreement recorded | spec `:804`; `docs/decisions/graph-hub.md:80-82` | yes — §6 of this report lists every deferral of spec §9 (`:680-691`) with its verdict, and claims nothing for `motor-alone`, which is in no rows file |
| §13.1–18 conditions, D1–D13 | spec `:806-841`; `docs/decisions/graph-hub.md:69-78` | yes where a row exists. The `[L]` conditions map to `hub-materialize` (`keep-cells`), `hub-seq`, `hub-idem`, `hub-epoch*`, `svc-supply`, `hub-virtual-root`, `hub-memory`, `hub-authz`, `hub-events`, `hub-breaks-off`, `hub-sync`. Conditions 17 ("every row and negative control on the landing commit") and 18 ("record the verdict") are **this report's** conditions: §5 runs every row and control on `fbd696f7` and §6 records the verdict |
| §14(a).4, .5, .8, .9, .10, .12 (the six partly-met revision 2 conditions) | spec `:855-861` | yes — `:856` fixes `default-members`, `:854` the manifest lock order, `:859` `GRAPH_HUB_LAYOUTS`, `:860` the `M` total, `:861` the §5.2 status map. The plan's "nine" counts the rows differently; the spec text at `:851` says **eight**, and its table has six [L] rows plus the two prose cases, so the count is not load-bearing here |
| §16(b) hub-sdk condition 6 | spec `:993`; `crates/graph-sdk-js/src/plugin/sync.ts` | met — the adapter's `deleted` maps to a delete when stored and is dropped otherwise (the `N14` fix, commit `6c8a2884`). The row the spec names, `hub-sync` (`hub-sdk.rows:26`, nested `hub-sync.rows:7`, its test "a row deleted from the file is deleted from the hub, and one never written is not sent"), runs since `ac8f53c2`; `hub-sdk-unit` (`hub-sdk.rows:15`) covers the unit half. The live row found that the records route answers qualified collections a batch body must not carry (deviation (b)) |

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
| N4 408 and 503 retried | 408 → 502 `MotorBodyTimeout`; 503 relayed without `Retry-After`; the SDK never retries `/layout`; the upload measured in slice 3 (§5.2, §5.3, §7, §11) | `7b2feb89` | `hub-motor-map` `hub.rows:45`; the upload, `hub-upload-timeout` `hub.rows:51` and §2 (slowest 2 773 ms of five against the 8 000 ms budget, `hub-memory.md:25-30`) | met |
| N5 `/layout` holds a snapshot while graph-server queues | `GRAPH_HUB_LAYOUTS` = 1 and a Caveat (§5.3, §6) | `7b2feb89` | `hub-motor-map` `hub.rows:45` | met |
| N6 M not total | every route under `READS` or `LAYOUTS`; connection and header limits; the anti-join portal; the hash compare; manifests streamed (§5.1, §5.3, §6) | `cc787956` | `hub-limits` `hub.rows:35` | met |
| N7 `hub-memory` negative control cannot fail | sized from the measured `F_w`, `--memory 1g`, a barrier (§8) | `4a05a967` (`scripts/orch/hub-mem.sh`) | `hub-memory` `hub.rows:49`, control `negctl-hub-memory` | met — the control row is OOM-killed at exit 137 (`hub-memory.md:155`) |
| N8 `doc_bytes` misses the head, the tail and manifests | head and tail lengths; the manifest PUT and the workspace create keep it (§4, §6) | `763f01c6` | `hub-seq` `hub-store.rows:66` | met |
| N9 manifest PUT lock order | workspace row first; create by `ON CONFLICT DO NOTHING` (§4) | `763f01c6` | `hub-seq` `hub-store.rows:66` | met |
| N10 virtual-root checked once | slices 2, 3 and 5; `client-legacy` grepped (§8, §10) | `e2851d3c` (`server/Cargo.toml`, +3 — the `default-members` line) | `hub-virtual-root` `hub-store.rows:29` and `hub.rows:3` | met — §4 of this report runs the slice-5 half; the `client-legacy` grep is recorded there |
| N11 `sequence-seq` cannot fail | removes the lock, delays odd commits (§8) | `79cdd47c` (`server/graph-store/tests/writer/sequence.rs`) | `hub-seq` `hub-store.rows:66` | met |
| N12 byte order assumed | `initdb --encoding=UTF8 --locale=C` and a start check (§6, §10) | `89b1e0cb` (`deploy/postgres.Dockerfile`, +30) | `hub-start-check`, inside `hub-pg-durability` `hub-store.rows:82` | met |
| N13 no body timeout | `GRAPH_HUB_BODY_TIMEOUT_MS`, 408 (§6) | `c3e22ad8` (`server/graph-hub/src/body.rs`, +74) | `hub-limits` `hub.rows:35` | met |
| N14 adapters' `deleted` unmapped | a delete when stored, else dropped (§7, `hub-sync`) | `6c8a2884` (`crates/graph-sdk-js/src/plugin/sync.ts`), completed by `185178bc` (`bareCollection`, deviation (b)) | `hub-sync` `hub-sdk.rows:26` (nested `hub-sync.rows:7`) | met — the live row runs since `ac8f53c2`; its verdict on `fbd696f7` is in §5 |
| N15 unlisted graph-server statuses | the default row, 502 `MotorError` (§5.2) | `7b2feb89` | `hub-motor-map` `hub.rows:45` | met |
| N16 records paging not a snapshot | `plugin_seq` per page, `If-Match`, start over on 412 (§5.2, §7, `hub-sync`) | `64e867a4` (`server/graph-store/src/records.rs`), the SDK half completed by `185178bc` (`readStored` ends on a `null` next, deviation (b)) | `hub-sync` `hub-sdk.rows:26`, the test "two writers of one plugin converge on the one that synced last", which asserts a 412 was seen | met — as N14 |
| H14 byte-equality precondition | hub-contract condition 1 (§10 slice 1) | `eb74797b` (`crates/graph-contract/src/hub/tests/materialize/order.rs`) | `hub-materialize` `hub-contract.rows:12` | met |

The first pass of this report left `N14` and `N16` **not claimed met**: their proving row `hub-sync`
was commented out at slice 4's landing. The `hub-live` branch uncommented it (`hub-sdk.rows:26-27`,
`ac8f53c2`), and running it end to end is what found the two SDK defects that completed both fixes
(deviation (b) below). All sixteen are now met, on the rows named.

### The §15(b) deviation, accepted in revision 5

The verdict's fix sent `event: resync` on a mid-stream pool timeout. Revision 5 sends `event: busy`
instead. The spec's own words, at `:938-939`: "Revision 5 sends `event: busy` instead: the cursor is
still valid, and a resync makes every subscriber read the whole `/graph` while the pool is already
short". The ADR accepts it on its merits (`docs/decisions/graph-hub.md:81-82`, and names it as the
one deviation from the verdict's fix at `:78-79`). The rows that pin it are `hub-events`
(`scripts/orch/rows/hub.rows:38`, the `busy` cases, §16(b) hub-api condition 11 at `:996`) and
`hub-sdk` — whose live row is `hub-sdk-live.rows:8` (nested under `hub-sdk.rows:27`), the tests "a busy
stream reconnects from its cursor and delivers" and "subscribers sent busy together do not come back
together"; `hub-sdk-unit` (`hub-sdk.rows:15`) holds the unit half.

This is a **deviation from the verdict's fix, accepted in revision 5**, not a defect found here.

### Deviations the live rows found after slice 4

Both were found by the live rows the `hub-live` branch enabled, both were fixed and landed on
develop before §5 ran, and both were masked by tests that do the missing step themselves. Line
numbers are at `fbd696f7`.

**(a) The hub binary never migrated its database.** The first live run answered the workspace PUT
with 500 `db`: the hub opened a fresh PostgreSQL and served without its schema. The Rust tests and
`scripts/orch/hub-mem.sh` call `graph_store::migrate::apply` on their own, so no row before the live
ones started the binary on an empty database. Fixed in `fbd696f7`: `main.rs:74` calls
`config::migrate_database` (`server/graph-hub/src/config/check.rs:173`) after the start checks and
before the bind; two hubs starting together serialize on `graph_store::migrate::MIGRATE_LOCK`
(`check.rs:172`). The test is `server/graph-hub/tests/start/migrate.rs`. The spec names migrations only
as graph-store's (`grep -in migrat` on it finds `:93` and slice 2's `:714`), never at hub start, so
this is an omission of the plan, not a change of the contract.

**(b) Five SDK defects behind the unit fakes.** `hub-sdk-unit` runs the SDK against fakes that
answered the shapes the SDK expected, not the shapes the hub sends:

| Defect | What the live row saw | Fix | Commit |
|---|---|---|---|
| `readStored` paged forever | the records route ends on `next: null`; the loop stopped only on `undefined` | `sync.ts:136` stops on either | `185178bc` |
| a batch named qualified collections | the records route answers `<plugin>.<collection>`; a batch body must name it bare (`a_qualified_collection_in_a_body_is_422`), so every delete was a 422 | `bareCollection`, `sync.ts:144` | `185178bc`, `439d33c5` |
| an empty first cursor | `/changes` refused `since=` with no cursor (`since_of`) | `formatCursor({epoch, seq})`, `subscribe.ts:208` | `fd84a7b2` |
| `/changes` read as an array | the route answers `{epoch, head_seq, next, bytes, changes}` | the page parsed as that object, `subscribe.ts:212`; `hub-api.md` updated | `fd84a7b2` |
| a failed read killed Node | a 503 while the read permits were taken rejected a detached loop | `passOf` catches it and reports it through a new `onError` option, `subscribe.ts:101-110` | `fd84a7b2` |

A subscriber with no `since` is now placed at the first notice's id minus one (`placeAt`,
`subscribe.ts:235`). None of the five changes the wire contract; each makes the SDK read what the
hub already sent.

### Found while writing this report

Neither is fixed in this slice; both are §6 rows. Line numbers are at `fbd696f7`.

**(c) A stray `POST /v1/workspaces`.** `server/graph-hub/src/lib.rs:58` routes
`get(routes::workspaces::list).post(not_ready)`, and `not_ready` (`:111`) answers 501
`NotImplemented` (`server/graph-hub/src/error.rs:137`), the only use of that variant. Spec §5.2's
route table (`:252-266`, unchanged since `c202527f`) has `GET /v1/workspaces` and
`PUT /v1/workspaces/{ws}`, and no `POST` on either path. The doc comment at `lib.rs:105-110` calls it
"a route of §5.2's table whose handler its own task has not written yet", which is false: no task
writes it. A `POST` there should be the JSON 404 `NotFound` every other wrong method gets
(`lib.rs:83`, `.method_not_allowed_fallback(not_found)`; `docs/contract/hub-api.md:57`), which the
router gives on its own once the method is removed.
No SDK call or row sends that request, so nothing observable to a plugin changes.

**(d) `hub-run.sh`'s reset cannot remove root-owned step files.** `hub-store.rows` runs its cargo
tests as root inside `gr` with `GM_HUB_STEP_DIR=../../target/hub-steps` (`scripts/orch/hub-pg.sh:279`),
which leaves root-owned files in `target/hub-steps`. `hub-run.sh`'s `reset` (`:172-175`,
`rm -rf "$state" "$steps"`) and its start (`:202`, `rm -f "$steps"/*.req`) run as the host user, so in
a worktree that ran `hub-store.rows` first, `hub.rows:47-50` fail before any hub starts. That is
exactly what §5's first run of `hub.rows` hit. `hub-store.rows:81-82` and `:85` already remove their
own step file through `scripts/orch/gr rm -f`, with the reason at `:81`; `hub-run.sh` does not.

## 4. Open review items

"Open" here means: no row in the review is marked closed or fixed, and reading the cited code on
`fbd696f7` shows the defect still present. Counts: **high 0, medium 0, low 0**, plus **0 process
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
because the (b) diff in §3 has 16 of 16 met, and the two defect classes the live rows found
(deviations (a) and (b) in §3) were fixed before §5 ran.

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
| slice 5 (this report, `c202527f`; `fbd696f7` has the same `server/Cargo.lock` and manifests) | 0 | 1 |

Slice 3 is **not** `not claimed met`: the count the row asserts is 1 in the `-p graph-hub` tree and
0 in the virtual root, and both hold on the landing commit, which is exactly what
`scripts/orch/rows/hub.rows:3` (`hub-virtual-root`) checks. Slice 5 re-ran the same grep on the same
commit and got the same two numbers; the merged tree `fbd696f7` changes none of its inputs
(`git diff --stat c202527f fbd696f7 -- server/Cargo.lock server/Cargo.toml server/graph-hub/Cargo.toml
server/graph-store/Cargo.toml` prints nothing, exit 0), and §5's `hub-virtual-root` row runs it there.

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

Run on 2026-10-06 (+02:00) in the `hub-report` worktree, on `c2c2f034` (develop `fbd696f7` merged
into the branch; `git diff --stat fbd696f7 c2c2f034` lists only this file) plus this report's first
draft, committed as `926df582`. The driver ran each rows file once, in this order, each under the
host gate lock:

```sh
for f in hub-contract hub-store hub hub-sdk service-supply svc-floor; do
  timeout 14400 scripts/orch/timed scripts/orch/gate.sh target/gate-dod-$f scripts/orch/rows/$f.rows
done
```

- **Totals:** 149 rows, 149 PASS, 0 not run. 68 of the 149 are controls, and all 68 went red as
  required. Four rows were red on the first run and green on a re-run of the same lines on the same
  tree (below); every other row was green on its first run.
- **Controls:** a control row expects exit 0 and wraps its own check (`! <check>`, or
  `test $? -eq 137`), so its PASS means the check it breaks went red.
- **Nested gates:** `hub-sdk.rows:26-27` run their own `gate.sh`, untimed because a nested `timed`
  deadlocks, into `target/gate-hub-sync` (`hub-sync.rows`) and `target/gate-hub-sdk`
  (`hub-sdk-live.rows`). Their rows are listed under those files.
- **Deviation from plan Task 5 Step 2:** one log directory per rows file, with one `<row>.log` per
  row in it, not one directory per row.
- **Evidence:** `$GM_SCRATCH/evidence/hub-report/<logdir>/`, copied from `target/<logdir>/` with
  every `scheme://…` value replaced by `<url>`; line numbers are unchanged.
- **After the gate:** the push job merged develop `d8dc23c6` into the branch (`6be43446`, 12:55:15).
  `git diff --stat 926df582 6be43446` lists eight files: `deploy/nav/interactrows.py`,
  `packages/graph-render/src/spacing.ts` and its test, `packages/graph-studio/src/styles/studio.css.ts`,
  `prompts/jobs/render-sparse-cells.md`, `scripts/orch/queue.txt`,
  `scripts/orch/rows/render-sparse-cells.rows` and `scripts/studio-interact.sh`. No rows file of
  this gate reads any of them, so the gate was not re-run.

| Rows file | Rows | PASS | Window | Time |
|---|---|---|---|---|
| `hub-contract.rows` | 19 | 19 | 11:33:37–12:01:07 | 1 650 s |
| `hub-store.rows` | 46 | 46 | 12:01:07–12:11:39 | 632 s |
| `hub.rows` | 52 | 48; 52 after the re-run | 12:11:39–12:35:21 | 1 422 s |
| `hub-sdk.rows`, nested `hub-sync.rows`, `hub-sdk-live.rows` | 13 + 2 + 2 | 17 | 12:35:21–12:51:32 | 971 s |
| `service-supply.rows` | 9 | 9 | 12:51:32–12:52:13 | 41 s |
| `svc-floor.rows` | 6 | 6 | 12:52:13–12:54:24 | 131 s |
| re-run of `hub.rows:47-50` | 4 | 4 | 12:54:26–12:55:12 | 46 s |

The six files took 4 847 s; the re-run took 46 s more.

### The rows

| Row | Expect | Exit | Verdict | Evidence |
|---|---|---|---|---|
| `fmt` (`hub-contract.rows:4`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/fmt.log` 1s |
| `clippy` (`hub-contract.rows:5`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/clippy.log` 10s |
| `clippy-hub` (`hub-contract.rows:6`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/clippy-hub.log` 4s |
| `test` (`hub-contract.rows:7`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/test.log` 594s |
| `codegen` (`hub-contract.rows:8`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/codegen.log` 0s |
| `wasm32-core` (`hub-contract.rows:9`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/wasm32-core.log` 5s |
| `hub-wire` (`hub-contract.rows:10`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/hub-wire.log` 5s |
| `hub-materialize` (`hub-contract.rows:12`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/hub-materialize.log` 0s |
| `motor-lock` (`hub-contract.rows:16`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/motor-lock.log` 0s |
| `svc-fmt` (`hub-contract.rows:19`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/svc-fmt.log` 1s |
| `svc-clippy` (`hub-contract.rows:20`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/svc-clippy.log` 7s |
| `svc-test` (`hub-contract.rows:21`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/svc-test.log` 135s |
| `root-fmt` (`hub-contract.rows:22`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/root-fmt.log` 1s |
| `hooks-gated` (`hub-contract.rows:23`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-contract/hooks-gated.log` 15s |
| `hub-virtual-root` (`hub-store.rows:29`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-virtual-root.log` 1s |
| `scratch-members` (`hub-store.rows:33`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/scratch-members.log` 0s |
| `hub-floor` (`hub-store.rows:35`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-floor.log` 126s |
| `clippy-store` (`hub-store.rows:37`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/clippy-store.log` 3s |
| `hooks-gated-store` (`hub-store.rows:39`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hooks-gated-store.log` 3s |
| `hub-epoch-trigger` (`hub-store.rows:41`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-epoch-trigger.log` 5s |
| `hub-epoch-detector` (`hub-store.rows:45`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-epoch-detector.log` 8s |
| `hub-promotion` (`hub-store.rows:53`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-promotion.log` 5s |
| `hub-pitr` (`hub-store.rows:55`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-pitr.log` 8s |
| `hub-snapshot` (`hub-store.rows:56`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-snapshot.log` 11s |
| `hub-crash-copy` (`hub-store.rows:57`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-crash-copy.log` 12s |
| `drun-check` (`hub-store.rows:58`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/drun-check.log` 0s |
| `cargo-deny-server` (`hub-store.rows:59`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/cargo-deny-server.log` 3s |
| `svc-supply` (`hub-store.rows:61`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/svc-supply.log` 18s |
| `svc-supply-messages` (`hub-store.rows:65`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/svc-supply-messages.log` 4s |
| `hub-seq` (`hub-store.rows:66`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-seq.log` 8s |
| `hub-idem` (`hub-store.rows:68`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-idem.log` 7s |
| `hub-epoch-deadlock` (`hub-store.rows:70`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-epoch-deadlock.log` 16s |
| `hub-materialize` (`hub-store.rows:72`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-materialize.log` 80s |
| `hub-changes-snapshot` (`hub-store.rows:75`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-changes-snapshot.log` 6s |
| `hub-retention` (`hub-store.rows:77`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-retention.log` 4s |
| `hub-pg-durability` (`hub-store.rows:82`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-store/hub-pg-durability.log` 25s |
| `hub-breaks-off` (`hub.rows:1`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-breaks-off.log` 0s |
| `hub-virtual-root` (`hub.rows:3`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-virtual-root.log` 2s |
| `hub-floor` (`hub.rows:5`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-floor.log` 125s |
| `clippy-hub` (`hub.rows:7`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/clippy-hub.log` 2s |
| `hooks-gated-hub` (`hub.rows:9`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hooks-gated-hub.log` 16s |
| `hub-image` (`hub.rows:11`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-image.log` 15s |
| `drun-check` (`hub.rows:13`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/drun-check.log` 0s |
| `svc-supply` (`hub.rows:14`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/svc-supply.log` 18s |
| `svc-supply-messages` (`hub.rows:18`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/svc-supply-messages.log` 3s |
| `cargo-deny-server` (`hub.rows:19`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/cargo-deny-server.log` 8s |
| `lock-parity` (`hub.rows:21`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/lock-parity.log` 2s |
| `svc-features` (`hub.rows:24`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/svc-features.log` 1s |
| `svc-image` (`hub.rows:26`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/svc-image.log` 74s |
| `hub-start-check` (`hub.rows:29`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-start-check.log` 8s |
| `hub-authz` (`hub.rows:31`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-authz.log` 5s |
| `hub-reload` (`hub.rows:33`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-reload.log` 5s |
| `hub-limits` (`hub.rows:35`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-limits.log` 16s |
| `hub-routes` (`hub.rows:37`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-routes.log` 8s |
| `hub-events` (`hub.rows:38`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-events.log` 16s |
| `hub-events-seq` (`hub.rows:40`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-events-seq.log` 4s |
| `hub-max-header` (`hub.rows:41`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-max-header.log` 238s |
| `hub-roundtrip` (`hub.rows:43`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-roundtrip.log` 6s |
| `hub-motor-map` (`hub.rows:45`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-motor-map.log` 8s |
| `hub-durability` (`hub.rows:47`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-rerun/hub-durability.log` 11s |
| `hub-memory` (`hub.rows:49`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-rerun/hub-memory.log` 24s |
| `hub-upload-timeout` (`hub.rows:51`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub/hub-upload-timeout.log` 262s |
| `wasm-release` (`hub-sdk.rows:12`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/wasm-release.log` 0s |
| `hub-sdk-types` (`hub-sdk.rows:13`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/hub-sdk-types.log` 1s |
| `hub-sdk-lint` (`hub-sdk.rows:14`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/hub-sdk-lint.log` 3s |
| `hub-sdk-unit` (`hub-sdk.rows:15`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/hub-sdk-unit.log` 7s |
| `hub-sdk-example` (`hub-sdk.rows:20`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/hub-sdk-example.log` 1s |
| `sdk-remote-regression` (`hub-sdk.rows:22`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/sdk-remote-regression.log` 0s |
| `hub-sync` (`hub-sdk.rows:26`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/hub-sync.log` 13s |
| `hub-sdk` (`hub-sdk.rows:27`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-hub-sdk/hub-sdk.log` 19s |
| `hub-sync` (`hub-sync.rows:7`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-hub-sync/hub-sync.log` 6s |
| `hub-sdk-live` (`hub-sdk-live.rows:8`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-hub-sdk/hub-sdk-live.log` 9s |
| `svc-digest` (`service-supply.rows:12`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-service-supply/svc-digest.log` 9s |
| `svc-digest-wasm` (`service-supply.rows:14`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-service-supply/svc-digest-wasm.log` 11s |
| `svc-features` (`service-supply.rows:16`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-service-supply/svc-features.log` 0s |
| `lock-parity` (`service-supply.rows:18`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-service-supply/lock-parity.log` 2s |
| `svc-fmt` (`svc-floor.rows:2`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-svc-floor/svc-fmt.log` 2s |
| `svc-clippy` (`svc-floor.rows:3`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-svc-floor/svc-clippy.log` 0s |
| `svc-test` (`svc-floor.rows:4`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-svc-floor/svc-test.log` 125s |
| `root-fmt` (`svc-floor.rows:5`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-svc-floor/root-fmt.log` 1s |
| `hooks-gated` (`svc-floor.rows:15`) | 0 | 0 | PASS | `$GM_SCRATCH/evidence/hub-report/gate-dod-svc-floor/hooks-gated.log` 3s |

### The controls

| Control | Break | Row it must turn red | Exit | Verdict |
|---|---|---|---|---|
| `negctl-lax-reader` (`hub-contract.rows:11`) | `GM_HUB_BREAK=lax-reader` | `hub-wire` | 0 | PASS |
| `negctl-keep-dangling` (`hub-contract.rows:13`) | `GM_HUB_BREAK=keep-dangling` | `hub-materialize` | 0 | PASS |
| `negctl-keep-cells` (`hub-contract.rows:14`) | `GM_HUB_BREAK=keep-cells` | `hub-materialize` | 0 | PASS |
| `negctl-motor-lock` (`hub-contract.rows:17`) | one appended line in a copy of `Cargo.lock` | `motor-lock` | 0 | PASS |
| `negctl-hooks-gated` (`hub-contract.rows:24`) | `--features test-hooks` forced on the release build | `hooks-gated` | 0 | PASS |
| `negctl-hub-virtual-root` (`hub-store.rows:32`) | the virtual root rebuilt from the base `f1a23521` | `hub-virtual-root` | 0 | PASS |
| `negctl-scratch-members` (`hub-store.rows:34`) | graph-store's manifest removed from the two-member scratch copy | `scratch-members` | 0 | PASS |
| `negctl-hub-floor` (`hub-store.rows:36`) | an unformatted file in a scratch copy | `hub-floor` | 0 | PASS |
| `negctl-clippy-store` (`hub-store.rows:38`) | a clippy-failing line in a scratch copy | `clippy-store` | 0 | PASS |
| `negctl-hooks-gated-store` (`hub-store.rows:40`) | `--features test-hooks` forced on the store lib | `hooks-gated-store` | 0 | PASS |
| `negctl-no-trigger` (`hub-store.rows:42`) | `GM_HUB_BREAK=no-trigger` | `hub-epoch-trigger` | 0 | PASS |
| `negctl-trigger-enable-origin` (`hub-store.rows:43`) | `GM_HUB_BREAK=trigger-enable-origin` | `hub-epoch-trigger` | 0 | PASS |
| `negctl-one-trigger-origin` (`hub-store.rows:44`) | `GM_HUB_BREAK=one-trigger-origin` | `hub-epoch-trigger` | 0 | PASS |
| `negctl-detector-at-start` (`hub-store.rows:46`) | `GM_HUB_BREAK=detector-at-start` | `hub-epoch-detector` | 0 | PASS |
| `negctl-lsn-only` (`hub-store.rows:47`) | `GM_HUB_BREAK=lsn-only` | `hub-epoch-detector` | 0 | PASS |
| `negctl-hw-after-lsn` (`hub-store.rows:48`) | `GM_HUB_BREAK=hw-after-lsn` | `hub-epoch-detector` | 0 | PASS |
| `negctl-checkpoint-timeline` (`hub-store.rows:54`) | `GM_HUB_BREAK=checkpoint-timeline` | `hub-promotion` | 0 | PASS |
| `negctl-deny-license-server` (`hub-store.rows:60`) | `MIT` removed from a copy of `deny.toml` | `cargo-deny-server` | 0 | PASS |
| `negctl-svc-supply-features` (`hub-store.rows:62`) | `--break` | `svc-supply` | 0 | PASS |
| `negctl-svc-supply-lock-version` (`hub-store.rows:63`) | `--break-version` | `svc-supply` | 0 | PASS |
| `negctl-svc-supply-lock-feature` (`hub-store.rows:64`) | `--break-feature` | `svc-supply` | 0 | PASS |
| `negctl-sequence-seq` (`hub-store.rows:67`) | `GM_HUB_BREAK=sequence-seq` | `hub-seq` | 0 | PASS |
| `negctl-no-idem` (`hub-store.rows:69`) | `GM_HUB_BREAK=no-idem` | `hub-idem` | 0 | PASS |
| `negctl-no-deadlock-retry` (`hub-store.rows:71`) | `GM_HUB_BREAK=no-deadlock-retry` | `hub-epoch-deadlock` | 0 | PASS |
| `negctl-keep-dangling` (`hub-store.rows:73`) | `GM_HUB_BREAK=keep-dangling` | `hub-materialize` | 0 | PASS |
| `negctl-keep-cells` (`hub-store.rows:74`) | `GM_HUB_BREAK=keep-cells` | `hub-materialize` | 0 | PASS |
| `negctl-changes-read-committed` (`hub-store.rows:76`) | `GM_HUB_BREAK=changes-read-committed` | `hub-changes-snapshot` | 0 | PASS |
| `negctl-prune-in-own-transaction` (`hub-store.rows:78`) | `GM_HUB_BREAK=prune-own-transaction` | `hub-retention` | 0 | PASS |
| `negctl-sync-commit-unset` (`hub-store.rows:85`) | `synchronous_commit = off`, `wal_writer_delay = 10s` | `hub-pg-durability` | 0 | PASS |
| `negctl-hub-breaks-off` (`hub.rows:2`) | the `negctl` feature forced on in a scratch copy | `hub-breaks-off` | 0 | PASS |
| `negctl-hub-virtual-root` (`hub.rows:4`) | the virtual root rebuilt from the base `f1a23521` | `hub-virtual-root` | 0 | PASS |
| `negctl-hub-floor` (`hub.rows:6`) | an unformatted file in a scratch copy | `hub-floor` | 0 | PASS |
| `negctl-clippy-hub` (`hub.rows:8`) | a clippy-failing line in a scratch copy | `clippy-hub` | 0 | PASS |
| `negctl-hooks-gated-hub` (`hub.rows:10`) | `--features test-hooks` forced on the hub release build | `hooks-gated-hub` | 0 | PASS |
| `negctl-hub-image` (`hub.rows:12`) | `HUB_IMAGE_BREAK=bin` | `hub-image` | 0 | PASS |
| `negctl-svc-supply-features` (`hub.rows:15`) | `--break` | `svc-supply` | 0 | PASS |
| `negctl-svc-supply-lock-version` (`hub.rows:16`) | `--break-version` | `svc-supply` | 0 | PASS |
| `negctl-svc-supply-lock-feature` (`hub.rows:17`) | `--break-feature` | `svc-supply` | 0 | PASS |
| `negctl-deny-license-server` (`hub.rows:20`) | `MIT` removed from a copy of `deny.toml` | `cargo-deny-server` | 0 | PASS |
| `negctl-lock-parity-version` (`hub.rows:22`) | `--break-version` | `lock-parity` | 0 | PASS |
| `negctl-lock-parity-feature` (`hub.rows:23`) | `--break-feature` | `lock-parity` | 0 | PASS |
| `negctl-svc-features` (`hub.rows:25`) | `--break` | `svc-features` | 0 | PASS |
| `negctl-svc-image` (`hub.rows:27`) | `SERVICE_IMAGE_BREAK=headers` | `svc-image` | 0 | PASS |
| `negctl-svc-image-leak` (`hub.rows:28`) | `SERVICE_IMAGE_BREAK=leak` | `svc-image` | 0 | PASS |
| `negctl-no-start-check` (`hub.rows:30`) | `GM_HUB_BREAK=no-start-check` | `hub-start-check` | 0 | PASS |
| `negctl-skip-grant` (`hub.rows:32`) | `GM_HUB_BREAK=skip-grant` | `hub-authz` | 0 | PASS |
| `negctl-reload-keys-only` (`hub.rows:34`) | `GM_HUB_BREAK=reload-keys-only` | `hub-reload` | 0 | PASS |
| `negctl-no-cap` (`hub.rows:36`) | `GM_HUB_BREAK=no-cap` | `hub-limits` | 0 | PASS |
| `negctl-skip-event` (`hub.rows:39`) | `GM_HUB_BREAK=skip-event` | `hub-events` | 0 | PASS |
| `negctl-sse-full-page` (`hub.rows:42`) | `GM_HUB_BREAK=sse-full-page` | `hub-max-header` | 0 | PASS |
| `negctl-drop-record` (`hub.rows:44`) | `GM_HUB_BREAK=drop-record` | `hub-roundtrip` | 0 | PASS |
| `negctl-layoutfailed-as-502` (`hub.rows:46`) | `GM_HUB_BREAK=layoutfailed-as-502` | `hub-motor-map` | 0 | PASS |
| `negctl-ack-before-commit` (`hub.rows:48`) | `GM_HUB_BREAK=ack-before-commit` | `hub-durability` | 0 | PASS |
| `negctl-hub-memory` (`hub.rows:50`) | the request cap lifted under `drun --memory 1g` (expects exit 137) | `hub-memory` | 0 | PASS |
| `negctl-throttle-upload` (`hub.rows:52`) | `GM_HUB_BREAK=throttle-upload` | `hub-upload-timeout` | 0 | PASS |
| `negctl-hub-sdk-cursor` (`hub-sdk.rows:16`) | `GM_HUB_SDK_BREAK=parsefloat` | `hub-sdk-unit` | 0 | PASS |
| `negctl-hub-sdk-gap` (`hub-sdk.rows:17`) | `GM_HUB_SDK_BREAK=no-gap-check` | `hub-sdk-unit` | 0 | PASS |
| `negctl-hub-sdk-key` (`hub-sdk.rows:18`) | `GM_HUB_SDK_BREAK=new-key-per-retry` | `hub-sdk-unit` | 0 | PASS |
| `negctl-hub-sdk-ifmatch` (`hub-sdk.rows:19`) | `GM_HUB_SDK_BREAK=head-seq-if-match` | `hub-sdk-unit` | 0 | PASS |
| `negctl-hub-sdk-example` (`hub-sdk.rows:21`) | `GM_HUB_SDK_BREAK=1` | `hub-sdk-example` | 0 | PASS |
| `negctl-hub-sync-via-graph` (`hub-sync.rows:8`) | `GM_HUB_SDK_BREAK=sync-via-graph` | `hub-sync` | 0 | PASS |
| `negctl-hub-sdk-live` (`hub-sdk-live.rows:9`) | `GM_HUB_SDK_BREAK=1` | `hub-sdk-live` | 0 | PASS |
| `negctl-svc-digest` (`service-supply.rows:13`) | `--break` | `svc-digest` | 0 | PASS |
| `negctl-svc-digest-wasm` (`service-supply.rows:15`) | `--break` | `svc-digest-wasm` | 0 | PASS |
| `negctl-svc-features` (`service-supply.rows:17`) | `--break` | `svc-features` | 0 | PASS |
| `negctl-lock-parity-version` (`service-supply.rows:19`) | `--break-version` | `lock-parity` | 0 | PASS |
| `negctl-lock-parity-feature` (`service-supply.rows:20`) | `--break-feature` | `lock-parity` | 0 | PASS |
| `negctl-hooks-gated` (`svc-floor.rows:16`) | `--features test-hooks` forced on the release build | `hooks-gated` | 0 | PASS |

### Controls that stayed green

None. All 68 exited 0, which for a control means the check it breaks went red.

### Rows over 60 s

| Row | Time | Line it ends on (a cargo row: the sum of its `test result:` lines) |
|---|---|---|
| `test` (`hub-contract.rows:7`) | 594s | 23 `test result:` lines, 2715 passed, 0 failed |
| `svc-test` (`hub-contract.rows:21`) | 135s | 46 `test result:` lines, 97 passed, 0 failed |
| `hub-floor` (`hub-store.rows:35`) | 126s | 46 `test result:` lines, 97 passed, 0 failed |
| `hub-materialize` (`hub-store.rows:72`) | 80s | 1 `test result:` line, 18 passed, 0 failed |
| `negctl-keep-dangling` (`hub-store.rows:73`) | 79s | 1 `test result:` line, 14 passed, 4 failed |
| `negctl-keep-cells` (`hub-store.rows:74`) | 77s | 1 `test result:` line, 17 passed, 1 failed |
| `hub-floor` (`hub.rows:5`) | 125s | 46 `test result:` lines, 97 passed, 0 failed |
| `svc-image` (`hub.rows:26`) | 74s | `no row was left unrun` |
| `hub-max-header` (`hub.rows:41`) | 238s | 1 `test result:` line, 1 passed, 0 failed |
| `negctl-sse-full-page` (`hub.rows:42`) | 208s | 1 `test result:` line, 0 passed, 1 failed |
| `hub-upload-timeout` (`hub.rows:51`) | 262s | 1 `test result:` line, 1 passed, 0 failed |
| `negctl-throttle-upload` (`hub.rows:52`) | 255s | 1 `test result:` line, 0 passed, 1 failed |
| `svc-test` (`svc-floor.rows:4`) | 125s | 46 `test result:` lines, 97 passed, 0 failed |

### Red on the first run

| Red row | Exit | Cause (its log) | Repair |
|---|---|---|---|
| `hub-durability` (`hub.rows:47`) | 1 | `hub-run.sh reset` could not remove the root-owned step files: 20 `rm: cannot remove 'target/hub-steps/…': Permission denied` lines (`gate-dod-hub/hub-durability.log:2-21`) | re-run: PASS, 11 s (`gate-dod-hub-rerun/hub-durability.log`) |
| `negctl-ack-before-commit` (`hub.rows:48`) | 1 | the same, 10 lines (`gate-dod-hub/negctl-ack-before-commit.log:2-11`), so the hub never started and the control could not go red | re-run: PASS, 5 s |
| `hub-memory` (`hub.rows:49`) | 2 | the same, 10 lines (`gate-dod-hub/hub-memory.log:5-14`), then `hub-mem: the database or the hub did not start` (`:15`) | re-run: PASS, 24 s, 441 368 KiB peak (`gate-dod-hub-rerun/hub-memory.log:129`) |
| `negctl-hub-memory` (`hub.rows:50`) | 1 | the same, 10 lines, then the same `did not start` line; the exit was not the 137 the control asserts | re-run: PASS, 6 s, `oom_killed=true` exit 137 (`gate-dod-hub-rerun/negctl-hub-memory.log:30`) |

The cause is deviation (d) in §3: `hub-store.rows` had left root-owned files in
`target/hub-steps`. The repair was one operator step,
`scripts/orch/gr chown -R 1000:1000 target/hub-steps target/hub-run`, then
`scripts/orch/timed scripts/orch/gate.sh target/gate-dod-hub-rerun target/hub-rerun.rows`, where
`target/hub-rerun.rows` is `hub.rows:47-50` byte for byte. No source changed between the two runs.
The chown is a workaround, not a fix; the fix is a §6 row.

## 6. What is not done

Spec §9 opens "Each of these waits for a caller" (`:678`); each deferral is cited at its `c202527f`
line (8 higher on `fbd696f7`, see the header). `won't fix (YAGNI)` means no caller exists today, not
that the item is refused.

| Item | Verdict | Why |
|---|---|---|
| The Redis cache and the snapshot cache | won't fix (YAGNI) | spec `:680`; D2 makes it a stop-and-ask item |
| Two or more hub instances, `LISTEN/NOTIFY` | won't fix (YAGNI) | spec `:681`; one hub per database |
| Pull or webhook plugins, in-process plugins, plugin distribution | won't fix (YAGNI) | spec `:682`; plugins push through the SDK's `sync` |
| Edge properties | won't fix (YAGNI) | spec `:683-684`; a relation with attributes is a record in its own collection |
| A query language, traversal or search route | won't fix (YAGNI) | spec `:685` |
| Time travel, `/graph?at=` | won't fix (YAGNI) | spec `:685`; `/graph` reads one snapshot at the head (§12.5) |
| Deleting a workspace or a plugin, changing or removing a manifest's collections or fields over HTTP | won't fix (YAGNI) | spec `:686-687`; operators do these in SQL and the epoch (H15) tells clients |
| TLS and publishing beyond the host | not attempted | spec `:688`; a stop-and-ask item |
| Per-key write quotas | won't fix (YAGNI) | spec `:688`; `hub-limits` (`hub.rows:35`) caps request bytes and permits, not a key's volume |
| A Python SDK | won't fix (YAGNI) | spec `:688`; the TypeScript SDK is the one SDK (§1 row 4) |
| `pgvector` | won't fix (YAGNI) | spec `:689`; the images ADR lists it and nothing calls it |
| Live studio integration | not attempted | spec `:690-691`; it needs `packages/graph-studio/src/host/*`, which graph-render-4f owns, so it is a later slice agreed with them |
| `motor-alone` | not run | spec `:672-674`; no rows file defines it (`git grep -n motor-alone -- scripts/orch/rows` prints nothing, exit 1), and this report claims `motor-lock` (`hub-contract.rows:16`) instead |
| Slice 1's measurement cell (§1 row 1) | not measured | slice 1 is the contract crate and computes no number the spec asks to measure |
| `POST /v1/workspaces` answers 501, not the JSON 404 (deviation (c) in §3) | not attempted | `server/graph-hub/src/lib.rs:58`, `:111`; a follow-up branch removes `.post(not_ready)`, `not_ready` and the `NotImplemented` variant, gated by `hub.rows` |
| `hub-run.sh` reset and root-owned step files (deviation (d) in §3) | not attempted | `scripts/orch/hub-run.sh:174`, `:202`; a follow-up branch removes the step directory through `scripts/orch/gr` as `hub-store.rows:81` does, gated by `hub-store.rows` then `hub.rows` in one worktree with no chown between them |

### Verdict

graph-hub is **done** against spec §8 and §10. All five slices are met (§1). The six numbers §6 of
the spec asks for are measured, and each is within its target (§2). All sixteen §14(b) defects
(`N1`–`N16`) are met, each on a named row (§3). No review item is open (§4). The full gate ran every
row and every control of the six hub rows files on the merged tree, and all 149 rows PASS, with all
68 controls red as required (§5). Two deviations of the plan were found and fixed before the gate,
(a) and (b) in §3. Two more were found while writing this report, (c) and (d); neither changes the
wire contract, and both are listed above as follow-up branches. The rest of the table is spec §9's
deferrals, each waiting for a caller or for an agreement with its owner, and `motor-alone`, which no
rows file defines.
