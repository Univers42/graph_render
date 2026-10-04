verdict: BLOCK

# Round 2 review — `graph-server` (docs/contract/service-api.md "Verdict")

Judge: independent, 2026-10-04. Branch `review-svc-r2`, base `origin/svc-image`
(`svc-server` b60d450b plus image `graph-motor:5f01f980602fc2dd`, 86,957,988 B).

The author's "As built" (contract line 256) covers conditions 3, 4, 9, 11 and 12 only. I checked
all 13 by symbol, not by the "As built" line numbers, which have moved (examples below).

## Evidence I accepted without re-running (author's, 2026-10-04, as pasted)

`scripts/orch/gate.sh` over the 26 rows of `scripts/orch/rows/svc-floor.rows` (4),
`service-supply.rows` (9) and `service.rows` (13, minus the two memory rows) on b60d450b: every row
PASS. Row names and order verified to match `scripts/orch/rows/*.rows` exactly
(`svc-fmt, svc-clippy, svc-test, root-fmt, svc-digest, negctl-svc-digest, svc-digest-wasm,
negctl-svc-digest-wasm, svc-features, negctl-svc-features, lock-parity,
negctl-lock-parity-version, negctl-lock-parity-feature, svc-preauth, negctl-preauth,
negctl-slow-headers, svc-exit2, negctl-exit2, svc-sighup, negctl-sighup, svc-healthcheck,
negctl-healthcheck, svc-shutdown, negctl-shutdown, svc-caps, negctl-caps`).
On svc-image: `svc-memory` PASS (8 GiB, `listening`), `negctl-memory` PASS (1 GiB, exit 2,
`GRAPH_WORKERS: unset, and memory.max holds no slot`). `service-image.rows` = **NOT RUN**
(`svc-image`, `negctl-svc-image`, `negctl-svc-image-leak`, `svc-sdk-live`, `negctl-svc-sdk-live`).

## Axis scores (5 = worst)

| Axis | Score | Why |
|---|---|---|
| Blast radius | 3 | a second workspace and a new public HTTP surface, but `git diff --name-only b60d450b..HEAD` touches no file under `crates/` and none under `server/` |
| Reversibility | 3 | routes, the key-file format and `crates/graph-sdk-js/src/remote.ts` become shipped surface; `server/` and `deploy/` are not fingerprinted, so a revert costs no motor re-gate |
| Cost on failure | 4 | one keyed run at cap under a memory cap nobody has tested is an OOM-abort of the whole process (exit 137) that takes every in-flight response with it; the published run shape cannot start at all (finding 1) |
| Confidence | 4 | conditions 1, 4 and 8 rest on rows whose negctl failed red; condition 3's central number rests on arithmetic plus one unit test, and its named row does not exist |

## The 13 conditions

Verdict key: **met** / **partly** / **not met**. A row with no negctl is "partly" whatever the code
does.

| n | condition (short) | met? | file:line evidence | row + negctl | gap |
|---|---|---|---|---|---|
| 1 | native seam, motor first | met | `crates/graph-wasm/src/lib.rs:156,163,171` (`contract`, `ingest`, `pub mod service` carry no `cfg`); `service.rs:39` `build`, `service.rs:60` `run` are `pub` and ungated; `ingest.rs:128` `read_records`, `ingest/ids.rs:43` `index`, `contract.rs:75` `derive` all `pub`, no `cfg` above any of them; `server/graph-server/src/motor.rs:10` imports them; `server/graph-server/Cargo.toml:15,16,18` give graph-contract/core/wasm no `features` key; `git grep probe\|threads -- server/` finds only comments | `svc-features` + `negctl-svc-features` PASS | the `hashgate --seeds 1000` / merge-floor clause of condition 1 is **not in the evidence set** — UNKNOWN, carried as new condition 5. Cosmetic: `lib.rs:141` `pub mod probe;` is ungated so `probe.rs` compiles natively; the race at `ingest/phases.rs:60-66` is prevented by `compile_error!` at `service.rs:21-24`, not by the gate |
| 2 | work caps, 413 after ingest before the run | partly | `server/graph-server/src/layout.rs:104` `motor::build` → `:110` size → `:112-115` `caps.admit(...).and_then(|| motor::run(...))`; refusal at `caps.rs:82-88` → `error.rs:51-52`; table `include_str!` at `caps.rs:15` | `svc-caps` + `negctl-caps` (break `lift-caps`, `caps.rs:81`) PASS | the registry↔tsv↔ceiling half is proved: 42 `layout.*` (`crates/graph-core/src/registry/layouts.rs:49`) + 8 `post.*` (`crates/graph-wasm/src/post/registry.rs:32`) rows, 1:1, none missing either way, every `cap_n` ≤ `scale_ceiling` (`tests/caps.rs` `every_service_id_has_exactly_one_row`, `no_cap_is_past_its_ledger_ceiling`). The **time** half is not: `docs/measurements/service-caps.md:13-19` states the ladder ran on the host CPU under load1 3.76–38.76 and "This is not the service image's reference CPU named in condition 2" |
| 3 | memory budget: admission before the body read, `WORKERS × per_slot` fits | not met | admission `layout.rs:64`, body read `layout.rs:65` (order holds); `config/slots.rs:12` `PER_SLOT_BYTES = 4_635_677_069`; `slots.rs:24-30` `default_workers`; refusal `config.rs:169-174` → `main.rs:69` exit 2; unit proof `config/tests.rs:72` `default_workers_is_the_smaller_of_cores_and_memory_slots` | **no `svc-limits` row exists.** `svc-memory` / `negctl-memory` PASS but prove only that `memory.max` reaches the server and that 8 GiB starts / 1 GiB refuses. Neither runs a slot at cap under a limit; no "double one cap" control exists | the load-bearing claim of condition 3 — the budget fits the container, no exit 137 — is **unrun**. UNKNOWN = FAIL. Compounded by finding 1: the documented 4 GiB shape exits 2 |
| 4 | request order, header/body/connection limits | met | `layout.rs:58` query-pair scan → `:59` `auth::check` → `:60` query → `:61` Accept → `:64` admit → `:65` `body::read`; variables `config.rs:158,161,162,180,182,183` applied `serve.rs:110-115` (timeout, `max_buf_size`) and `serve.rs:93-99` (pre-accept permit) | `svc-preauth` PASS; `negctl-preauth` (`layout.rs:53`), `negctl-slow-headers` (`serve.rs:111`) PASS | `GRAPH_MAX_CONNECTIONS` and `GRAPH_MAX_HEADER_BYTES` **enforcement** has no test (only range validation, `config/tests.rs:43`). The "As built" line 274 omits the pair scan at `:58`, which is deliberately before auth (`layout.rs:1-4`) |
| 5 | slots and faults: permit moves into the closure, panic → 500 | partly | `layout.rs:79` `OwnedSemaphorePermit` → `:86-89` moved into `spawn_blocking`, `_slot` bound inside; timeout path `layout.rs:94` drops only the handle; `JoinError`/panic → `layout.rs:92-93`; `panic = "unwind"` `server/Cargo.toml:12,15`; fault hook `app.rs:39-44` called at `layout.rs:100-102` | **no row.** `tests/limits.rs:86` `a_timed_out_run_keeps_its_slot_until_it_ends` and `:121` `a_panic_is_500_and_the_next_run_is_200` run only under the blanket `svc-test` (`svc-floor.rows:4`), which has no negctl. The break that would move them, `drop-permit` (`layout.rs:80`), has **no row** | the property is in the code and a break exists, but nothing proves the break turns it red, and a future edit that drops the permit at the timeout keeps every row green. Also `app.rs:39-44` is **not** behind `#[cfg(feature = "negctl")]` unlike every break |
| 6 | one post, named errors, Accept | partly | one post `query.rs:184-188`; Accept `query.rs:120-121,127,158-169`; no chain: `crates/graph-wasm/src/service.rs:63` layout geometry, `:65` post pass; 14 emitted names `error.rs:37,44,52,59,67,75,81,90,98` + `motor.rs:33-47`, all present in the contract table; SDK additive `crates/graph-sdk-js/src/remote/errors.ts:16` `SERVICE_ERROR_NAMES` | **no row** (contract names `svc-sdk`). Tests `errors.rs:21,84,107,125` run only under `svc-test` | contract's own `svc-sdk` row and its "rename one name" control do not exist. `docs/contract/service-api.md:29` still advertises `post=<id>[,<id>]` "applied in order", which `query.rs:184` refuses |
| 7 | digest manifest, both sources, all ids, JSON round-trip | met | `server/graph-server/tests/digest/manifest.json` 99 rows over 2 fixtures; tests `digest.rs:77` `both_readers_are_covered`, `:109` `every_id_whose_cap_admits_a_fixture_is_pinned`, `:141` `a_pinned_fixture_holds_a_component_in_the_lobpcg_window` (`COMPONENT_FLOOR=257`, `CEILING=700` at `digest/manifest.rs:33,35`), `:160` `the_json_face_round_trips_to_the_same_bytes`; `git grep -nE 'target-cpu\|target-feature\|RUSTFLAGS' -- server/` → no match | `svc-digest` + `negctl-svc-digest`, `svc-digest-wasm` + `negctl-svc-digest-wasm` PASS | the manifest's union is all 42 layouts and all 8 posts, so "drop a registry id" is already fatal by construction; `--break` only moves `every_row_hashes_to_its_committed_digest` (`digest/seam.rs:76`), the four manifest-coverage tests are not moved by it — a coverage regression would need a new id in the registry to show up |
| 8 | lock-parity with features | met | `scripts/orch/rows/service-supply.rows:24-26`; `scripts/orch/lock-parity.sh:98-101` compares `cargo tree -e normal,features` for `graph-wasm` in both lockfiles plus `graph-wasm feature` lines, `diff` at `:147` | `lock-parity` + `negctl-lock-parity-version`, `negctl-lock-parity-feature` PASS | none |
| 9 | auth: sensitive header, query/second-header 400, exit 2, SIGHUP, `off` refuses non-loopback | partly | `observe.rs:80` `mark_sensitive` before every handler (`lib.rs:40`, layered after all routes and both fallbacks); key in query `query.rs:63-67`; second header `auth.rs:12-16`; case-insensitive `auth.rs:34-38`; `WWW-Authenticate` `error.rs:114`; refusals `keys.rs:60,85-87,187-189,190-192,196,207-219`; SIGHUP parse-then-swap `app.rs:80-97` + `keys.rs:146-148`; `GRAPH_AUTH=off` on a resolved `IpAddr` `config.rs:139-144` | `svc-exit2` + `negctl-exit2`, `svc-sighup` + `negctl-sighup` PASS. **No `svc-auth` and no `svc-log` row exist** | `tests/auth.rs` (6 functions, incl. `a_missing_wrong_or_truncated_key_is_401_with_a_challenge`, `auth_off_serves_without_a_key_on_loopback`) and `tests/log.rs` (5, incl. `no_line_holds_the_key_or_its_hash`) run only under `svc-test`. The breaks that would move them exist and are unrowed: `any-key` (`auth.rs:24`), `log-header` (`observe.rs:81`). `mark_sensitive` (`observe.rs:119`) is asserted by no test. `negctl-exit2`'s break touches one of ten `start.rs` exit-2 cases (`keys.rs:60`) |
| 10 | embed: content-derived version, `..`/symlink/dotfile 404 | partly | version from `scripts/service.sh content_hash()` into `embed/VERSION`, read at `embed.rs:53,99-109` (16 hex); `nosniff` `embed.rs:29`, JS MIME `:177`; walk uses `symlink_metadata` `embed.rs:145` and skips symlinks and dotfiles `:148`, inserts only `is_file()` `:155-157`; request is pure table lookup with a version match `embed.rs:82-90` | **no `svc-embed` row in any `.rows` file.** `tests/embed.rs:41,61,87,94` run only under `svc-test`. The break that would move them, `follow-symlinks` (`embed.rs:142`), has no row | a change that switches the walk to `metadata` keeps every row green. The same-origin / module-Worker rule (`contract:229-231`) exists in docs only, never in `server/` |
| 11 | image: pinned digest, numeric non-root, HEALTHCHECK | partly | `deploy/service.Dockerfile:7` `debian:trixie-slim@sha256:a99cfc51…`; `:15` `USER 10001:10001`; `:21-22` `HEALTHCHECK graph-server healthcheck` → `main.rs:61-66` → `health.rs:15-17` true only on a 200 within 2 s | `svc-healthcheck` + `negctl-healthcheck` PASS prove the **command**; `svc-image` NOT RUN, so the image itself is UNKNOWN | `deploy/service.Dockerfile.dockerignore:1-4` is not named `.dockerignore` and `scripts/service.sh` never copies it into `target/service/stage`, so it excludes nothing; the no-leak property rests solely on the unrun `svc-no-leak` scan (`scripts/service-image.sh:63`). Publish-on-loopback is only a `-p 127.0.0.1` in a script (`scripts/service.sh`), unproved by a row |
| 12 | operability: X-Request-Id, one JSON log line, SIGTERM drain, preflight | partly | id `observe.rs:106-116` (1..=128 `is_ascii_graphic`, `MAX_REQUEST_ID=128` at `:20`), generated `:69-72`, echoed `:163`; log line `observe.rs:94-101` → `render` `:182-196` carries id, key name, route, status, ms, layout, post, n, m; SIGTERM/SIGINT `serve.rs:59-60`, listener dropped `:70`, drain `serve.rs:145-155` on `GRAPH_TIMEOUT_MS` (`config.rs:161`), `serve.rs:44` exit 0; preflight before auth, `/v1/` + configured origin only `observe.rs:85-88,134-138`; `meta.rs:30-36` drops `analyses` | `svc-shutdown` + `negctl-shutdown` PASS cover the drain only | the id and log halves have **no row**; `tests/routes.rs:20,96` and `tests/log.rs:13,39,69,88,117` run only under `svc-test`. The "As built" (contract:292-293) says as much and does not claim them. `breaks.rs` names no break; `drop-permit`, `log-header` unrowed |
| 13 | evidence scope: `server/`+`deploy/` unfingerprinted, motor owes the full gate | not met | `git diff --name-only b60d450b..HEAD` → 17 files, **zero under `crates/`, zero under `server/`**; `git diff --stat origin/svc-image...HEAD` → empty (HEAD is an ancestor of `origin/svc-image`) | `git diff origin/svc-image...HEAD` empty; no `develop-full.rows` run for this branch in the evidence set; `service-image.rows`' `svc-sdk-live` / `negctl-svc-sdk-live` NOT RUN, and condition 13 names the row `svc-sdk` | the condition's own rows — the develop full gate and `svc-sdk` — are unrun. `crates/graph-sdk-js/src/remote.ts` is byte-unchanged on this branch (`:73` single `config.fetch` call site, `:15` `FetchLike`, key only in `Authorization` at `:57`), but nothing here re-proves it |

### Notes on the "As built"

- Correct where it speaks: the order at `layout.rs:64`/`:65`, the refusal at `config.rs:169-174`
  → `main.rs:69`, the drain at `serve.rs:145-155`, the SIGHUP swap, the healthcheck chain.
- **Stale, and stale against itself.** Lines 269-271 say `svc-memory` and `negctl-memory` are NOT
  RUN because no image exists; the image now exists and both rows PASS. The paragraph understates
  the evidence it has and overstates the coverage it lacks.
- Its line numbers have drifted: `:126-135` for the SIGHUP swap (really `serve.rs:129-142` +
  `app.rs:80-97`), `:94` for the pre-accept permit (really `serve.rs:93-99`), `:144` for the drain
  (really `serve.rs:145-155`), and it omits the pre-auth query-pair scan at `layout.rs:58`.
- `breaks.rs` (15 lines) defines **no break names**. It is the `on()` helper only
  (`breaks.rs:6-9` with the feature, `:12-14` always false). The 11 break names are string literals
  at their call sites: `auth.rs:24`, `caps.rs:81`, `embed.rs:142`, `health.rs:17`, `keys.rs:60`,
  `layout.rs:53`, `layout.rs:80`, `observe.rs:81`, `serve.rs:111`, `serve.rs:136`, `serve.rs:147`.
  All 7 names set by a `negctl-` row resolve to a call site. Four resolve to nothing:
  `any-key`, `log-header`, `follow-symlinks`, `drop-permit`.

## Ruling on the per-slot decision: option (b)

`docs/deploy/service.md:47-50` publishes `docker run … --memory 4g` and
`docs/decisions/memory-guard.md:26-29` fixes "a 4 GiB default cap" for every container
(`scripts/orch/drun:21`, `DRUN_MEM:-4g`), so a 4 GiB deployment **is** a stated shape, and
option (a) makes that shape exit 2 at start. Option (b), with the term taken from the doc's own
table: a 16 MiB contract body measured 307,241,473 B of heap
(`docs/measurements/service-caps.md:196`), so per-slot = 67,108,864 + 307,241,473 +
3,343,908,864 = 3,718,259,201 (3.46 GiB) and 4 GiB holds one slot with 576 MB spare — not the
~3.55 GB the job brief estimated, because the contract heap is 18.3× body
(`service-caps.md:193`), not the studio ratio. Cost: one `GRAPH_*` variable, one 413 path, one row
with a negctl. If the operator side may move instead, raising `DRUN_MEM` and the published
`docker run` to 8 GiB is strictly cheaper and needs no code — but then `docs/deploy/service.md:50`
is the contract, and a 4 GiB host still cannot run the service.

## Task 4 — conditions "As built" does not cover

| n | covered by "As built"? | does the code + rows meet it anyway? |
|---|---|---|
| 1 | no | yes for the seam and the features (`svc-features` + negctl PASS); no for the `hashgate --seeds 1000` clause (not in the evidence set) |
| 2 | no | yes for the registry↔tsv↔ceiling mapping and for 413-after-ingest (`svc-caps` + negctl PASS); no for the "within `GRAPH_TIMEOUT_MS` on the image's reference CPU" clause — measured off-CPU under load, declared at `service-caps.md:13-19` |
| 5 | no | the code meets it (`layout.rs:79-89`, `:92-94`, `panic = "unwind"`); **no row**, so partly |
| 6 | no | the code meets it (`query.rs:184`, `query.rs:120-127`, `service.rs:63-65`, all 14 names in the table, `remote/errors.ts:16`); **no row**, so partly |
| 7 | no | yes, and it is the best-evidenced condition in the set: four digest rows, all PASS |
| 8 | no | yes: `lock-parity` + two negctls, PASS |
| 10 | no | the code meets it structurally (`embed.rs:145-157` walk, `:82-90` lookup); **no row**, and the break that would prove it is unrowed, so partly |
| 13 | no | **not met**: `git diff origin/svc-image...HEAD` is empty, so the motor seam is inherited unproven here, and neither named row (`develop-full`, `svc-sdk`) was run |
| 12, id/log half | no (the author says so) | the code meets it (`observe.rs:106-116`, `:182-196` with all nine fields); tests exist under `svc-test` only, so partly |

## Task 5 — house limits on `server/`

- Files over 300 lines: **none**. Largest source file `server/graph-server/src/config.rs` at 276;
  `server/Cargo.lock` 619 is generated.
- Functions over 40 lines: **none**. Longest are `tests/caps.rs:162` (34),
  `tests/limits.rs:86` (32), `tests/common/mod.rs:89` (31), `src/embed.rs:137` (30),
  `src/keys.rs:76` (27), `src/observe.rs:76` (27), `src/serve.rs:47` (26). Ten `let` closures,
  longest 12 lines.
- Functions over 4 parameters: **none**. Widest is `caps.rs:75` `admit(&self, layout, post, size)`.
- `#[allow(`: **none** in `server/` (`grep -rn` → no match), so no unreasoned allow.
- `Caveat:` on constants: **4 of 13**. Present at `embed.rs:20`, `serve.rs:28`, `slots.rs:9`,
  `slots.rs:15`. **Missing** on `serve.rs:26` `RUNTIME_GRACE`, `health.rs:10` `BUDGET`,
  `health.rs:13` `MAX_STATUS_LINE`, `observe.rs:20` `MAX_REQUEST_ID`, `error.rs:12` `MAX_MESSAGE`,
  `keys.rs:18` `KEY_BYTES`, `keys.rs:20` `MAX_FILE_BYTES`, `keys.rs:22` `MAX_KEYS`,
  `keys.rs:24` `MAX_NAME`, and on every default literal at `config.rs:158-184` (64 MiB body, 2×
  workers, 30 000 ms, 10 000 ms, 1024 workers, 5 000 ms header, 16 384 / 8 192 header bytes,
  256 / 65 536 connections).

## Task 6 — security pass (`risk.md`: auth and untrusted input)

| question | answer | evidence |
|---|---|---|
| Is any key, hash or `Authorization` value logged? | **no**, in any shipped build | `observe.rs:80` `mark_sensitive` runs in middleware before the handler (`:87`); the only raw-value path is `:81` → `:197-199`, gated on `breaks::on("log-header")`, which is `const fn false` without `negctl` (`breaks.rs:12-14`), and `negctl` is not a default feature (`graph-server/Cargo.toml:40`) nor passed by the release build (`scripts/service.sh:72`). No `{:?}` of a `HeaderMap`/`Request` anywhere in `src/`. The key **name** is logged (`observe.rs:189`) and is charset- and length-validated at load (`keys.rs:190-205`, `[A-Za-z0-9._-]`, 1..=64), so no log injection. `config.rs:123-131` prints `set`/`unset` per variable and no value. The only stdout key is `keygen` (`main.rs:53`), a CLI by design. Residual: the request path is logged verbatim (`observe.rs:83`), JSON-escaped |
| Does the embed resolve a path outside `GRAPH_EMBED_DIR` through symlinks? | **no** | startup walk uses `symlink_metadata` (`embed.rs:145`) and skips symlinks and dotfiles at any depth (`:148`), inserting only `is_file()` entries (`:155-157`); a symlinked root is refused (`:55-58`); no `canonicalize` anywhere. The request path never touches the filesystem: it is a table lookup with a version match (`embed.rs:82-90`), so a symlink created **after** startup cannot be served. Residual startup TOCTOU: type is checked at `:145-147`, then the file is read **by path** at `:162` with no `O_NOFOLLOW`, which needs write access to the embed dir — root-owned and read-only in the image (`deploy/service.Dockerfile:10,15`) |
| Can any request reach the motor before auth? | **no** | `lib.rs:33-42` routes: `/healthz` (`lib.rs:45`, returns `"ok"`), `/v1/meta` (auth `meta.rs:28`, motor ids `meta.rs:34-35` after), `POST /v1/layout` (auth `layout.rs:59`, `query::layout` `:60` and `motor::build` `:104` after), `/embed/{*path}` (public by design, no motor), method/unknown fallback (`lib.rs:49`), OPTIONS preflight answered in middleware (`observe.rs:85-88`). The only pre-auth work is `query::pairs` (`layout.rs:58`), a `String` allocation bounded by `GRAPH_MAX_HEADER_BYTES` (`serve.rs:115`) |
| Does `GRAPH_AUTH=off` refuse a non-loopback bind? | **yes** | `config.rs:139-144` checks `bind.is_loopback()` on the **parsed** `IpAddr` (`config.rs:102`), not the literal string: `0.0.0.0`, `::`, `[::]` are refused, `127.0.0.0/8` and `::1` are accepted, `localhost` cannot be passed (parse fails → refuse, `config.rs:246-250`). The image sets `GRAPH_BIND=0.0.0.0` (`deploy/service.Dockerfile:12`), so `off` is impossible there. Test `start.rs:102` `auth_off_on_a_public_bind_is_exit_2` covers `0.0.0.0` only |
| extras | verified clean | hash compare is constant-time over every entry, no early exit (`keys.rs:107-116`); no unbounded pre-auth allocation (`body.rs:13-22` after `layout.rs:59`); CORS is an exact byte compare on `/v1/` only, `*` rejected at config (`observe.rs:136-137`, `config.rs:197-204`); HTTP/1 only, no hand framing (`serve.rs:109`); SIGHUP is parse-then-atomic-swap with the old set kept on a bad file and only a line number logged (`app.rs:80-97`); no metrics or debug endpoint exists |

## Why BLOCK, and what would change it

Round 1 ruled BLOCK because two load-bearing claims were false. One of them (condition 1, the native
seam) is now true and row-proved. The other — "the ingest limits bound how slow a run can be", the
memory budget — has been **replaced, not closed**: condition 3's named row (`svc-limits`: every slot
busy at cap under `docker --memory` at that limit, with no exit 137, negctl double one cap) does not
exist in any rows file, and the substitute rows prove only that `memory.max` reaches the server and
that 8 GiB starts while 1 GiB refuses. Per the Round-1 rule, an unproven safety claim rules BLOCK.
On top of that, the published 4 GiB run shape cannot start the service at all.

### New conditions, each checkable by a row with a negctl

1. **`svc-limits` exists and proves the budget.** Runs `GRAPH_WORKERS=1` at the largest cap in
   `docs/measurements/service-caps.tsv` under `docker --memory` at `PER_SLOT_BYTES + BASE_BYTES`,
   asserting exit 0 and no exit 137. Negctl: double one cap in a scratch copy of the tsv, expect the
   row red.
2. **The 4 GiB shape is resolved in code or in docs, not both ways.** Either `DRUN_MEM` and
   `docs/deploy/service.md:50` move to 8 GiB, or option (b) lands (a `GRAPH_MAX_CONTRACT_BODY` at
   16 MiB, a 413 path on it, `PER_SLOT_BYTES` = 3,718,259,201, and `config/tests.rs` gaining
   `4 << 30 -> 1`). Negctl: set the contract body limit back to 64 MiB and expect
   `default_workers(4 << 30) == 1` to go red.
3. **The caps are re-measured on the image's reference CPU**, or the time-bound caps are lowered by
   the one-rung margin `docs/measurements/service-caps.md:16-19` declares. New row `svc-caps-time`:
   every time-bound id at its cap answers 200 within `GRAPH_TIMEOUT_MS`. Negctl: halve one cap's rung.
4. **One `negctl-` row per unrowed break**: `negctl-any-key` (`auth.rs:24`), `negctl-svc-log`
   (`observe.rs:81`, the row condition 9 names `svc-log`), `negctl-svc-embed` (`embed.rs:142`, the
   row condition 10 names `svc-embed`), `negctl-svc-limits` (`layout.rs:80`). Each names the test
   function it turns red.
5. **Condition 1's process clause is run**: `develop-full` and `hashgate --seeds 1000` on the
   commit that carries the `graph-wasm` seam, on this branch. UNKNOWN until then.
6. **`svc-image` runs** (`service-image.rows`, all five rows) and
   `deploy/service.Dockerfile.dockerignore` is either copied into the staged context as
   `.dockerignore` or deleted, with `docs/deploy/service.md` saying which control owns the exclusion.
7. **The contract's stale rows are reconciled**: either `svc-auth`, `svc-embed`, `svc-log`,
   `svc-limits`, `svc-sdk` exist, or `docs/contract/service-api.md:128-136` is amended to the rows
   that do. Also fix `:28` (`analyses`), `:29` (`post=<id>[,<id>]`), `:103-105` (11 variables listed,
   14 in `config.rs:16-31`), `:269-271` (the two memory rows now PASS).
8. **`Caveat:` on the nine constants and the `config.rs:158-184` defaults** that lack one, and on
   `BASE_BYTES = 0` — `docs/measurements/service-caps.md:150-151` says `base` is "the server's idle
   RSS, which `svc-limits` measures", and `svc-limits` does not exist, so `base` is a stated 0.
9. **`config/tests.rs:72` gains `4 << 30 -> 0`** (or `-> 1` under option (b)), so the headline
   4 GiB row is asserted directly rather than through `PER_SLOT_BYTES - 1` (`tests.rs:80`).

## Findings

| severity | file:line | defect | failure scenario | fix |
|---|---|---|---|---|
| high | `docs/deploy/service.md:47-50`, `scripts/orch/drun:21`, `docs/decisions/memory-guard.md:26-29` | the documented 4 GiB run shape is below one `PER_SLOT_BYTES` (4.32 GiB) | an operator copies the published `docker run --memory 4g`; the container exits 2 with `GRAPH_WORKERS: unset, and memory.max holds no slot` and never serves. Raise the cap instead and the same host OOM-kills at 137 instead | new condition 2 |
| high | `scripts/orch/rows/service.rows` (no `svc-limits` row) | condition 3's named row does not exist; `svc-memory` proves the cgroup read and the refusal, not the fit | a single run at the largest cap (n 1048576, m 4194304) under `--memory 8g` pushes the process past `memory.max`; the kernel OOM-kills it, every in-flight response dies, and no row is red | new condition 1 |
| medium | `docs/measurements/service-caps.md:13-19` | the time-bound caps were measured on the host CPU under load1 up to 38.76, and the doc says it is not the image's reference CPU | on the image's CPU a cap rung takes longer than `GRAPH_TIMEOUT_MS` (30 s default); the client gets 503 after the work is done and the slot is held for the rest of the run | new condition 3 |
| medium | `server/graph-server/src/auth.rs:24`, `observe.rs:81`, `embed.rs:142`, `layout.rs:80` | four breaks exist with no `negctl-` row naming them | any of `auth::check` accepting a well-formed key, the trace layer printing `Authorization`, the embed walk following a symlink, or the permit being dropped at the timeout lands with every row in the gate still green | new condition 4 |
| medium | `scripts/orch/rows/*.rows` vs `docs/contract/service-api.md:128-136` | the contract names `svc-auth`, `svc-embed`, `svc-log`, `svc-limits`, `svc-sdk`; none exists in any rows file | conditions 5, 6, 9 and 10 rest on `svc-test` (`svc-floor.rows:4`), which has no negative control at all — a green `svc-test` proves only that the tests pass today | new condition 7 |
| medium | `deploy/service.Dockerfile.dockerignore:1`, `scripts/service.sh build` | the file is not named `.dockerignore` and is never copied into `target/service/stage`, so it excludes nothing | the "no key file, `.env`, `.git` or scratch path in the image" property of condition 11 rests solely on the unrun `svc-no-leak` scan | new condition 6 |
| medium | condition 1's `hashgate --seeds 1000` clause; `git diff --stat origin/svc-image...HEAD` is empty | the motor seam is inherited into this branch with no develop gate or hashgate in the evidence set | a `pub`-widening in `crates/graph-wasm` lands on `develop` unmeasured; the digest rows only compare the two builds of the same tree | new condition 5 |
| low | `server/graph-server/src/serve.rs:26`, `health.rs:10`, `health.rs:13`, `observe.rs:20`, `error.rs:12`, `keys.rs:18,20,22,24`; `config.rs:158-184` | nine heuristic constants and every limit default carry no `Caveat:` line (4 of 13 constants have one) | `MAX_FILE_BYTES`, `MAX_KEYS` and `RUNTIME_GRACE` are the exact numbers a later edit will move without asking what they get wrong | add one line each |
| low | `server/graph-server/src/keys.rs:60-63` | only mode `& 0o022` is refused, so a world-readable `0644` key file is accepted, while the message says "0640 or stricter is accepted" | an operator widens the key file to 0644 to make a bind-mount work and no start refusal fires; the file holds hashes, so the exposure is hashes, not keys | mask `0o004`, or correct the message |
| low | `server/graph-server/src/config/tests.rs:72-87` | no `4 << 30 -> 0` case; the headline 4 GiB row is asserted only through `PER_SLOT_BYTES - 1` (`:80`) | a future edit to `PER_SLOT_BYTES` or `BASE_BYTES` moves the real 4 GiB answer with no assertion on it | new condition 9 |
| low | `docs/contract/service-api.md:28`, `:29`, `:103-105`, `:269-271` | the route table still lists `analyses` (dropped at `meta.rs:30-36`) and `post=<id>[,<id>]` (refused at `query.rs:184`); the config list has 11 of 14 variables; `:269-271` says the two memory rows are NOT RUN when both PASS | a caller builds against `post=a,b` and gets 400; an operator sizes from the 4 GiB line | new condition 7 |
| low | `server/graph-server/src/observe.rs:119`; `serve.rs:115`, `config/tests.rs:43` | `mark_sensitive` is asserted by no test; `GRAPH_MAX_CONNECTIONS` and `GRAPH_MAX_HEADER_BYTES` enforcement has no test | a layer reordered under the middleware, or the connection cap dropped, keeps every row green | add cases to `tests/log.rs` and `tests/preauth.rs` |
| low | `server/graph-server/src/app.rs:39-44` | the test fault hook `Hooks::before_run` is not behind `#[cfg(feature = "negctl")]`, unlike every break | it is a public field compiled into the shipped binary; today unreachable only because `from_settings` sets `Hooks::default()` | gate it with the feature |
| low | `server/graph-server/src/embed.rs:145-162` | startup TOCTOU: the type is checked with `symlink_metadata`, then the file is read by path with no `O_NOFOLLOW` | an entry swapped between the check and the read is followed; needs write access to the embed dir, which the image forbids | read through the already-open dirfd, or accept and document |

## Not verified here (UNKNOWN, and treated as FAIL)

- `service-image.rows` — all five rows NOT RUN by instruction. Condition 11's image half and
  condition 13's `svc-sdk` row are UNKNOWN.
- Condition 3's budget fit, and condition 1's `hashgate --seeds 1000` / develop full gate. No row
  in the evidence set touches either.
- The four `negctl-` rows that do not exist cannot be shown red; the four breaks they would name are
  read from the source only.
- Whether a slot at cap stays inside `PER_SLOT_BYTES` is a measurement nobody has run.