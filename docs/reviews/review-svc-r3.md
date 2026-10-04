verdict: PROCEED-WITH-CONDITIONS

# Round 3 review — `graph-server` (docs/contract/service-api.md "Verdict")

Judge: independent, 2026-10-04. Branch `review-svc-r3` at `825e6a08`, base `origin/develop`
(`develop` merged in at `473a0a6e`). I did not run cargo, docker or `gate.sh`; every PASS below is
the orchestrator's, from `target/evidence/`, and every code claim is by symbol with `file:line`.

## What changed the verdict

Round 2 ruled BLOCK on two unrun safety claims. Both are now run and both pass, with working
negative controls:

- **The memory budget fits.** `svc-limits` + `negctl-svc-limits` PASS (`service-limits.summary.txt:1-2`).
  `scripts/service-limits.sh` reads `PER_SLOT_BYTES` and `BASE_BYTES` out of
  `server/graph-server/src/config/slots.rs:12,22` with `grep` (`:77-81`), caps the container at their
  sum rounded up to a whole MiB (`:175-178`), and asserts four checks: two 200s, `OOMKilled false`,
  and `memory.peak <= M` (`:212-226`). The negctl runs the identical script at `1g` and passes only
  when the row exits 1 having written `FAIL oom` (`scripts/orch/rows/service-limits.rows:4`). The
  measurement is in `docs/measurements/service-caps.md:268-296`: peak 1,620,025,344 B against
  M 4,647,288,832 B — 34.9% — under load1 42.99 → 44.67.
- **The time-bound caps were re-measured on the image.** `svc-caps-time` + `negctl-svc-caps-time` PASS
  (`service-limits.summary.txt:3-4`). `target/evidence/service-caps-time.report.tsv`: 55 rows, 0 FAIL,
  slowest `layout.force.yifan_hu` at 19,514 ms against the 30,000 ms mark, load1 33.84 → 8.00, with the
  negctl at `SERVICE_CAPS_TIME_TIMEOUT_MS=1` required to turn **every** row 503
  (`scripts/orch/rows/service-limits.rows:5`).

The 4 GiB shape is resolved in docs, not in code — option (a), the cheaper of the two I offered:
`scripts/service.sh:22,108` pass `DRUN_MEM=${DRUN_MEM:-8g}`, `docs/deploy/service.md:50` names 8 GiB the
minimum and `:57` publishes `--memory 8g`, and `docs/decisions/memory-guard.md:29-32` records the service
as the one exception to the 4 GiB default.

The reason this is not PROCEED is one specific, reachable gap, and it is in the same property Round 2
blocked on: `PER_SLOT_BYTES` is computed from a 64 MiB body, and `GRAPH_MAX_BODY` is settable to 1 GiB
in range. New condition 1, below, closes it. Nothing else in the set is a false claim about the
shipped default.

## Axis scores (5 = worst)

| Axis | Score | Why |
|---|---|---|
| Blast radius | 3 | a second workspace and a new public HTTP surface with a shipped key-file format; `server/` and `deploy/` are unfingerprinted, so a revert costs no motor re-gate |
| Reversibility | 3 | the routes, the key file and `./remote` become shipped surface. The `crates/` delta is 528 lines and reaches no shipped code (`crates/graph-wasm/src/memory_measure.rs:28` is `#![cfg(test)]`) |
| Cost on failure | 4 | one in-range setting (`GRAPH_MAX_BODY=1g`) plus one keyed request makes the kernel OOM-kill the process, so every in-flight response dies and the container crash-loops. Recovery is a restart: no data loss, no breach |
| Confidence | 3 | the default path is measured twice with working controls; the one gap is narrow, understood, and named. It is 3, not 5, because the gate that would have caught it does not exist |

## Evidence I accepted, and the one thing I checked myself

Five summaries in `target/evidence/`, and every row count matches its rows file exactly — 4/4,
9/9, 23/23, 4/4, 5/5 — so the `gate.sh` last-row defect (fixed on develop at `d73b3869`) did not fire
here. Worth recording: `scripts/orch/rows/service.rows` still has **no trailing newline** (last byte
`0x22`) and its summary carries all 23 rows, so the fix holds on a file that would have lost one.

`svc-floor` and `service-supply` ran on this exact tree, `825e6a08`. `service`, `service-limits` and
`service-image` ran on `86362211`. The job brief asked me to verify that this is sound rather than
assume it. It is: `git diff --stat 86362211..HEAD` is one file, 10 insertions,
`server/graph-server/tests/digest/manifest.json`, five layout ids × two sources, and no existing hash
moved. That file is read in exactly two places — `server/graph-server/tests/digest/manifest.rs:24`
(`include_str!`) and `scripts/orch/svc-digest-wasm.sh:56` — and neither `scripts/service-limits.sh`,
`scripts/service-caps-time.sh` nor `scripts/service-image.sh` mentions `digest` at all
(`grep -n digest` → no match in any of the three). So the three older summaries are valid evidence for
this tree. Had they been wrong, `service`, `service-limits` and `service-image` would be UNKNOWN and the
verdict would be BLOCK.

## The 13 conditions

Verdict key **met** / **partly** / **not met**. A row with no negctl, or NOT RUN, caps at *partly*.

| n | condition (short) | met? | evidence on this tree | row + negctl | gap |
|---|---|---|---|---|---|
| 1 | native seam, motor first | **partly** | `server/graph-server/Cargo.toml` gives graph-contract/core/wasm no `features` key; `server/graph-server/src/motor.rs` imports `read_records`, `index`, `derive`; `git grep 'probe\|threads' -- server/` finds comments only | `svc-features` + `negctl-svc-features` PASS (`service-supply.summary.txt:5-6`) | the **process** clause — "its own branch, through the merge floor and `hashgate --seeds 1000`" — has no row and did not run. Deferred to develop by the merge policy; see Round-2 condition 5, where I judge that order sound |
| 2 | work caps; at most `scale_ceiling` and at most what fits the timeout | **met** | refusal after ingest before the run at `src/layout.rs` (`motor::build` → size → `caps.admit(..).and_then(\|\| motor::run(..))`), `src/caps.rs:75` `admit` (a missing row refuses), table `include_str!` at `src/caps.rs:15`; 55 tsv rows = 42 `layout.*` + 8 `post.*` + the 5 new 3-D/2z ids | `svc-caps` + `negctl-caps` PASS (`service.summary.txt:12-13`); `svc-caps-time` + `negctl-svc-caps-time` PASS | none material. The doc still declares the ladder ran on the host CPU (`service-caps.md:13-14,399-401`) while the image runs the same binary on the same 20 cores, so the CPU identity is disclosed, not closed; worst row 65% of the mark under load1 33.84 |
| 3 | memory budget: admission before the body read, `WORKERS × per_slot` fits | **partly** | order holds (`src/layout.rs` `admit` then `body::read`); `src/config/slots.rs:12` `PER_SLOT_BYTES = 4_635_677_069`, `:22` `BASE_BYTES = 11_534_336`, `:28-34` `default_workers` = `min(cores, (memory.max − BASE_BYTES) / PER_SLOT_BYTES)`; refusal `src/config.rs:186-196` → `src/main.rs` `refuse` | `svc-limits` + `negctl-svc-limits` PASS; `svc-memory` + `negctl-memory` PASS (`service.summary.txt:22-23`) | **`PER_SLOT_BYTES` is built on a 64 MiB body; `GRAPH_MAX_BODY` accepts up to 1 GiB in range and nothing cross-checks them.** `src/config.rs:178` `env.number("GRAPH_MAX_BODY", 64 << 20, 1..=graph_wasm_max_ingest())` with `graph_wasm_max_ingest() = 1 << 30` (`:224-226`), and `deploy/service.Dockerfile:12` sets no `GRAPH_MAX_BODY`, so the default holds until an operator moves it. See finding 1 and new condition 1 |
| 4 | request order and connection limits | **met** | `src/layout.rs` `serve`: pre-auth query-pair scan → `auth::check` → `query::layout`/`face` → `gate::admit` → `body::read`; header timeout and `max_buf_size` at `src/serve.rs:105-118` `Acceptor::spawn`; pre-accept permit at `src/serve.rs:93-104` `Acceptor::next` | `svc-preauth` + `negctl-preauth` + `negctl-slow-headers` PASS (`service.summary.txt:1-3`) | enforcement of `GRAPH_MAX_CONNECTIONS` and `GRAPH_MAX_HEADER_BYTES` has no test (`grep` over `server/graph-server/tests/` → no match); only range validation, `src/config/tests.rs:43`. Carried from Round 2, finding 11 |
| 5 | slots and faults: permit into the closure, panic → 500, process lives | **met** | `src/layout.rs` moves the `OwnedSemaphorePermit` into `spawn_blocking` and binds it inside; the timeout path drops only the handle; `JoinError`/panic → 500; `panic = "unwind"` in `server/Cargo.toml`; the fault hook is called before the run | `svc-slots` + `negctl-svc-slots` PASS (`service.summary.txt:20-21`) | the row names the test function its break must redden, `a_timed_out_run_keeps_its_slot_until_it_ends` (`scripts/orch/rows/service.rows:77`). Round 2's "no row" gap is closed. Residual: the hook `Hooks::before_run` is still not behind `#[cfg(feature = "negctl")]` — finding 4 |
| 6 | one post, named errors, Accept | **met** | `src/query.rs` refuses a second `post`; `crates/graph-sdk-js/scripts/live-check.mjs:46` sends `post=a,b` and requires the typed 400; `:41` the typed 401; `crates/graph-sdk-js/src/remote/errors.ts` `SERVICE_ERROR_NAMES` grows additively | `svc-sdk-live` + `negctl-svc-sdk-live` PASS 5/5 in `service-image.summary.txt:4-5` | none. The contract's own `svc-sdk` row is gone; the row that answers it exists |
| 7 | digest manifest, both sources, every admitting id, JSON round-trip | **met** | `tests/digest/manifest.json` now carries the five new ids for both fixtures, 10 rows, no existing hash moved — the exact defect that made `svc-digest` and `svc-test` red on `86362211`. `tests/digest/coverage.rs:5` documents that dropping a row fails a test rather than shrinking coverage; `tests/digest/wasm-arm.mjs` reads the same file | `svc-digest` + `negctl-svc-digest`, `svc-digest-wasm` + `negctl-svc-digest-wasm` PASS **on this tree** (`service-supply.summary.txt:1-4`); `svc-test` PASS (`svc-floor.summary.txt:3`) | the manifest's union is every registry id, so "drop a registry id" is fatal by construction. `--break` only moves the per-row hash test; a coverage regression needs a new registry id to surface. Same residual as Round 2, and it is the right residual |
| 8 | lock-parity with features | **met** | `scripts/orch/lock-parity.sh` compares `cargo tree -e normal,features` for `graph-wasm` in both lockfiles plus feature lines | `lock-parity` + `negctl-lock-parity-version` + `negctl-lock-parity-feature` PASS (`service-supply.summary.txt:7-9`) | none |
| 9 | auth: sensitive header, query/second-header 400, exit 2, SIGHUP, `off` refuses non-loopback | **met** | `src/observe.rs:83` `mark_sensitive` in middleware before any handler; asserted by `src/observe.rs:231` `the_authorization_reaching_the_handler_is_marked_sensitive`, which probes a layer inside `observe` and reads the flag as a handler would — the Round 2 "asserted by no test" gap is closed. `src/keys.rs` `KeySet::load` refuses any mode bit for others, read included (`docs/deploy/service.md:66-70` now matches the code) | `svc-auth` + `negctl-svc-auth` (`any-key`, names `a_missing_wrong_or_truncated_key_is_401_with_a_challenge`), `svc-log` + `negctl-svc-log` (`log-header`, names `no_line_holds_the_key_or_its_hash`), `svc-exit2`, `svc-sighup` — all PASS (`service.summary.txt:14-19`) | none. Every break that was unrowed in Round 2 now has a row that names the function it turns red |
| 10 | embed: content-derived version, `..`/symlink/dotfile 404 | **met** | version from `scripts/service.sh content_hash()` into `embed/VERSION`; the walk uses `symlink_metadata` and skips symlinks and dotfiles at any depth, inserting only `is_file()`; the request path is a pure table lookup, so a symlink created after start cannot be served; `src/embed.rs:113-114` carries an explicit `Caveat (TOCTOU)` on the residual read-by-path | `svc-embed` + `negctl-svc-embed` PASS (`service.summary.txt:18-19`), negctl names `every_path_outside_the_files_is_404` | none. The Round 2 TOCTOU finding is now documented rather than open |
| 11 | image: pinned digest, numeric non-root, HEALTHCHECK, no leak | **met** | `deploy/service.Dockerfile` pins `debian:trixie-slim` by digest, `USER 10001:10001`, `HEALTHCHECK graph-server healthcheck` → `src/main.rs` `healthcheck` → `src/health.rs` `healthcheck`, true only on a 200 inside `BUDGET`; `deploy/service.Dockerfile.dockerignore` is **gone** — `ls deploy/` shows no such file — and the staged context is the exclusion control | `svc-image` + `negctl-svc-image` + `negctl-svc-image-leak` + `svc-healthcheck` + `negctl-healthcheck` all PASS (`service-image.summary.txt:1-3`, `service.summary.txt:8-9`) | the summaries do not record which image tag ran. See finding 2 |
| 12 | operability: `X-Request-Id`, one JSON log line, SIGTERM drain, preflight | **met** | `src/observe.rs` validates the id (1..=128 `is_ascii_graphic`), generates one, echoes it, and `Line::render` carries id, key name, route, status, ms, layout, post, n, m; `src/serve.rs` drops the listener on SIGTERM/SIGINT so new connections are refused at once, then drains up to `GRAPH_TIMEOUT_MS`; `src/meta.rs` drops `analyses` | `svc-shutdown` + `negctl-shutdown` PASS (`service.summary.txt:10-11`) cover the drain; the id and log halves have `svc-log` + `negctl-svc-log` | the preflight branch has no row of its own, as in Round 2. Carried as finding 5 (low) |
| 13 | evidence scope: `server/`+`deploy/` unfingerprinted, motor owes the develop gate | **partly** | `git diff --name-only origin/develop...HEAD -- crates/` is 9 files, 528 insertions: `crates/graph-wasm/src/memory_measure.rs` (**`#![cfg(test)]`**, `:28`) plus `ingest_peak.rs`, and `crates/graph-cli/src/bench/cap_probe*` plus its command wiring. No `pub` widening, no kernel, no ingest-path change. `crates/graph-sdk-js/` does not appear at all, so `./remote` is inherited byte-unchanged from develop | `svc-sdk-live` PASS (the row condition 13 names). `develop-full.rows` and `hashgate --seeds 1000`: **no summary in `target/evidence/`, so NOT RUN** | the develop full gate is owed after landing, by policy. Condition 13's own process half is UNKNOWN; capped at *partly* by the rule that an unrowed row cannot exceed it |

## The 9 Round-2 conditions

| n | condition (short) | met? | evidence | row + negctl | gap |
|---|---|---|---|---|---|
| R2-1 | `svc-limits` exists and proves the budget fits, no exit 137 | **met** | `scripts/orch/rows/service-limits.rows:3-4`; `scripts/service-limits.sh:175-178` derives M from `slots.rs` and `:212-226` writes four checks; `service-caps.md:268-296` records them | both PASS (`service-limits.summary.txt:1-2`) | none |
| R2-2 | the 4 GiB shape resolved in code or docs, not both ways | **met** | `scripts/service.sh:22,108` `DRUN_MEM=${DRUN_MEM:-8g}`; `docs/deploy/service.md:50` "8 GiB is the minimum", `:57` `--memory 8g`; `docs/decisions/memory-guard.md:29-32`; `src/config/tests.rs:81-86` asserts `default_workers(8, 4<<30) == 0` **and** `default_workers(8, 8<<30) == 1` | `svc-memory` + `negctl-memory` PASS; the derivation is pinned by `src/config/tests.rs:73` `default_workers_is_the_smaller_of_cores_and_memory_slots` | none. The code now says out loud that 4 GiB refuses to start |
| R2-3 | caps re-measured on the image, or lowered by the declared rung margin | **met** | `service-caps.md:253-371`; `target/evidence/service-caps-time.report.tsv` 55 rows, 0 FAIL, worst 19,514 ms / 30,000 ms. No cap was lowered | `svc-caps-time` + `negctl-svc-caps-time` PASS (`service-limits.summary.txt:3-4`) | none material; the CPU-identity disclosure at `service-caps.md:399-401` is honest and load-biased the safe way |
| R2-4 | one `negctl-` row per unrowed break | **met** | all four exist and each greps for the test function its break must redden: `negctl-svc-auth` → `a_missing_wrong_or_truncated_key_is_401_with_a_challenge`, `negctl-svc-log` → `no_line_holds_the_key_or_its_hash`, `negctl-svc-embed` → `every_path_outside_the_files_is_404`, `negctl-svc-slots` → `a_timed_out_run_keeps_its_slot_until_it_ends` (`scripts/orch/rows/service.rows:53,58,68,77`) | all four PASS (`service.summary.txt:15,17,19,21`) | none. The 11 break names all resolve to a call site; the four Round 2 called dead are armed |
| R2-5 | condition 1's process clause: develop full gate + `hashgate --seeds 1000` | **partly** | `crates/` delta = 528 lines, 9 files, none reaching shipped code (`#![cfg(test)]` at `memory_measure.rs:28`; a new `graph-cli bench cap-probe` subcommand). Nothing under `crates/` changed between the commits the summaries were run on | no summary for `develop-full.rows`; **NOT RUN** | the run is owed on develop after landing (`prompts/RESUME.md`, CLAUDE.md "Parallel branches"). **I judge this order sound**: a `#![cfg(test)]` measuring module and a CLI subcommand cannot move the native/wasm byte parity that `svc-digest` and `svc-digest-wasm` already pin on this tree, and `svc-floor.rows` runs no `crates/` row at all, so a branch-level gate here would prove nothing the develop gate does not. It stays a *partly*, carried as new condition 5 |
| R2-6 | `service-image.rows` runs, and the build context's exclusion control is named | **met** | 5/5 PASS (`service-image.summary.txt`). `deploy/service.Dockerfile.dockerignore` no longer exists (`ls deploy/`); `scripts/service.sh` stages `bin/graph-server`, `embed/<version>/`, `embed/VERSION` into `target/service/stage` and makes that the whole context | `svc-image` + `negctl-svc-image` + `negctl-svc-image-leak` + `svc-sdk-live` + `negctl-svc-sdk-live` | the tag is not recorded in the summary. Finding 2 |
| R2-7 | the contract's stale rows, routes and variables reconciled | **partly** | `analyses` gone from the route table (`service-api.md:29`), `post` is one id (`:30`), all 14 `GRAPH_*` variables listed with default and range (`:108-123`) matching `src/config.rs:18-33` `NAMES`, and the Gates table (`:149-171`) names only rows that exist — I resolved every name in it against the five rows files | all rows PASS | the Configuration table's `GRAPH_WORKERS` default still reads `min(cores, memory.max / PER_SLOT_BYTES)` (`:115`) and the "As built" §3 repeats `floor(memory.max / PER_SLOT_BYTES)` (`:318-320`); the code subtracts `BASE_BYTES` first (`slots.rs:30`). Same omission in both places, and the table never tells the operator that 8 GiB is the floor. Finding 3 |
| R2-8 | `Caveat:` on the nine constants and the two `config.rs` default blocks | **met** | all present: `serve.rs:26,31`, `health.rs:10,16`, `observe.rs:20`, `error.rs:12`, `keys.rs:18,22,26,30,35` (the mode mask), and one block above `config.rs` `read_limits` (`:157-174`) and `read_connections` (`:198-211`) naming every default literal in each. `BASE_BYTES` now carries one too (`slots.rs:17-21`), and `caps.rs:6` gets one | `svc-clippy` + `svc-fmt` PASS (`svc-floor.summary.txt:1-2`) | none |
| R2-9 | `config/tests.rs` asserts the real 4 GiB row directly | **met** | `src/config/tests.rs:81-85` asserts `default_workers(8, Some(4 << 30)) == 0` by name, beside `:86` `8 << 30 -> 1`, `:87-91` the doc's 64 GiB row, `:80` `PER_SLOT_BYTES - 1 -> 0` | `svc-test` PASS (`svc-floor.summary.txt:3`) | none |

## Round-2 findings, re-checked

| # | severity | file:line | status | evidence |
|---|---|---|---|---|
| 1 | high | `docs/deploy/service.md:47-50`, `scripts/orch/drun:21`, `docs/decisions/memory-guard.md:26-29` | **fixed** | `service.sh:22,108` `DRUN_MEM=8g`; `service.md:50,57`; `memory-guard.md:29-32`; `config/tests.rs:81-86` asserts both the 0 and the 1 |
| 2 | high | no `svc-limits` row in any rows file | **fixed** | `service-limits.rows:3-4`, both PASS; `service-caps.md:268-296` records the four checks and the negctl's OOM kill |
| 3 | medium | `service-caps.md:13-19` (caps measured off-image, host under load) | **fixed** | measured on the image twice, `service-caps.md:253-371`; 55 rows 0 FAIL, worst 65% of the mark under load1 33.84 |
| 4 | medium | `auth.rs:24`, `observe.rs:81`, `embed.rs:142`, `layout.rs:80` (four breaks, no row) | **fixed** | `service.rows:53,58,68,77`, four negctls PASS, each naming the function it reddens |
| 5 | medium | `scripts/orch/rows/*.rows` vs contract `:128-136` (contract named rows that did not exist) | **fixed** | every row name in the Gates table now resolves; `svc-sdk` is gone, `svc-sdk-live` answers condition 6 |
| 6 | medium | `deploy/service.Dockerfile.dockerignore` excluded nothing | **fixed** | the file is deleted; the staged context is the control, named at `docs/deploy/service.md:49` |
| 7 | medium | condition 1's hashgate clause | **open, deferred** | no `develop-full` summary; owed on develop. The `crates/` delta is small enough for that order — R2-5 above |
| 8 | low | nine constants and `config.rs` defaults with no `Caveat:` | **fixed** | all present, plus `BASE_BYTES` and `caps.rs` |
| 9 | low | `config/tests.rs` no `4 << 30` case | **fixed** | `config/tests.rs:81-85` |
| 10 | low | contract `:28,29,103-105,269-271` stale | **mostly fixed** | routes and variables reconciled; the `GRAPH_WORKERS` formula is still stale — finding 3 |
| 11 | low | `observe.rs` `mark_sensitive` untested; `GRAPH_MAX_CONNECTIONS`/`GRAPH_MAX_HEADER_BYTES` enforcement untested | **half fixed** | `observe.rs:231` now asserts the marking; `grep` over `server/graph-server/tests/` finds **no** test naming either variable — finding 5 |
| 12 | low | `app.rs:39-42` `Hooks::before_run` not behind `#[cfg(feature = "negctl")]` | **open** | still a `pub` field compiled into the shipped binary, unlike every other break — finding 4 |
| 13 | low | `embed.rs:145-162` startup TOCTOU on the read-by-path | **moved, documented** | `embed.rs:113-114` carries an explicit `Caveat (TOCTOU)` naming the residual and why the image forbids the write access it needs |

## Security pass on the new code (`risk.md`: auth, untrusted input, the host page, the caps path)

| question | answer | evidence |
|---|---|---|
| Can the new host page leak or forge anything? | **no** | `deploy/nav/serviceproxy.py` is gate scaffolding, not shipped surface: it binds `127.0.0.1:0` (`:110`), it is started only by `scripts/service-image.sh`, and its pages are three static strings. The CSP is the least the docs ask of (`CSP` at `:23`), the host's half of `host-api.md` condition 3 is a separate file `host.js` because the CSP refuses an inline script (`:39-45`), and the fixture it serves is a repo file read at import (`:46`). `forward()` (`:83-99`) forwards `/v1/` with `Authorization` intact and drops hop-by-hop headers, which is what a reverse proxy must do |
| Is the manifest hash, or any new constant, trusted from outside the repo? | **no** | `manifest.json` is `include_str!`-ed (`tests/digest/manifest.rs:24`) and hashed by `tests/digest/seam.rs`; `service-limits.sh:77-81` greps `slots.rs` for the two budget constants rather than carrying its own copy, and `service-caps-time.sh:74-78` quotes `MAX_BENCH_NODES` with its source. The negctl that would have caught a doctored table edits a scratch copy (`scripts/orch/rows/service-limits.rows:5`), never the committed file |
| Does the caps path admit anything untrusted? | **no** | `caps.admit` (`src/caps.rs:75`) refuses an id with **no** row, so a missing row can never mean uncapped, and the table is a `BTreeMap` keyed by id — no HashMap iteration reaches output order. The new `crates/graph-cli/src/bench/cap_probe.rs:164 window` reads `/proc/self/status` and emits `ms`/`peak_mib` into a measurement file, not into the motor |
| Is the new `negctl` feature reachable in a release build? | **no, except one field** | `src/breaks.rs` is the `on()` helper and every break is a string literal at its call site gated on the `negctl` feature, which is not a default feature and is not passed by `scripts/service.sh`. The one exception is `Hooks::before_run` (`src/app.rs:39-42`) — finding 4, low, unreachable today because `from_settings` sets `Hooks::default()` |
| Anything else new and reachable? | **no** | `app/src/embed-bundle.ts` and `app/vite.embed.config.ts` build the bundle the image serves; no new route, no new variable (`src/config.rs:18-33` `NAMES` still has 14), no new log field. `scripts/caps-ladder.sh` and `scripts/caps-table.sh` are measurement tools writing under `target/`. The residual from Round 2 stands unchanged and is now the subject of new condition 1: a request path is logged verbatim, JSON-escaped (`src/observe.rs`) |

## House limits on the changed files

Checked every `.rs` file in `git diff --name-only origin/develop...HEAD` (52 files).

- **Over 300 lines: none.** Longest changed source file is `server/graph-server/src/config.rs` at 276.
- **Over 40 lines per function: none.** I counted bodies by brace balance, not by eyeball; the five candidates all resolve to 4-parameter signatures well under the limit.
- **Over 4 parameters: none.** The widest are `src/caps.rs:75` `admit(&self, layout, post, size)`, `src/config.rs:252` `number(&self, name, default, range)`, `src/serve.rs:105` `spawn(&self, stream, slot, graceful, app)` and `crates/graph-cli/src/bench/cap_probe.rs:164` `window(subject, topology, input, started)` — 4 each.
- **`#[allow(`: none** anywhere in `server/` or the new `crates/` files.
- **`Caveat:` on heuristics: present** on every constant named in R2-8, plus the two new ones (`slots.rs:17` `BASE_BYTES`, `caps.rs:6`) and two the Round 2 sweep did not reach (`serve.rs:31` the post-accept pause, `embed.rs:113` the TOCTOU).

## New conditions

Each is checkable by a row with a negative control. They become acceptance criteria.

1. **`GRAPH_MAX_BODY` is inside what the per-slot budget accounts for.** `PER_SLOT_BYTES`
   (`src/config/slots.rs:12`) is the 64 MiB body plus a 64 MiB contract ingest peak of
   1,224,659,341 B (18.25× body) plus the run peak — while `src/config.rs:178` accepts any
   `GRAPH_MAX_BODY` up to `graph_wasm_max_ingest()` = 1 GiB and nothing re-derives the budget when
   it moves. Either the range ceiling drops to 64 MiB, or `default_workers` divides what remains of
   `memory.max` by a per-slot figure recomputed from the **effective** `GRAPH_MAX_BODY`, and a start
   whose budget holds no slot is refused by the existing `read_workers` path.
   **Row `svc-max-body`** (`scripts/orch/rows/service-limits.rows`): with `GRAPH_MAX_BODY` at its
   ceiling and `GRAPH_WORKERS` unset, the server either refuses to start with the existing
   `GRAPH_WORKERS: unset, and memory.max holds no slot` line, or answers 200 with
   `memory.peak <= memory.max` — the same four checks `scripts/service-limits.sh:212-226` already
   writes. **Negctl:** raise the ceiling back to 1 GiB over the recomputed figure and expect the row
   red on `FAIL oom`. The `Caveat:` at `src/config.rs:163-164` must change with it: it calls
   `GRAPH_MAX_BODY` "a buffer bound, not a work bound" and never says it is a *memory* bound, which is
   the edit that invites the mistake.
2. **`svc-limits` is re-run under the ceiling condition 1 lands.** If condition 1 lands as a lower
   range ceiling, `scripts/service-limits.sh` needs no change and the existing PASS stands. If it lands
   as a recomputed per-slot figure, the run must be repeated at the new figure and
   `docs/measurements/service-caps.md` "Memory per slot" rewritten from the new numbers.
   **Negctl:** the existing `SERVICE_LIMITS_MEM=1g` control, unchanged.
3. **Every summary that ran a container records the image tag it ran.** `scripts/orch/gate.sh:24`
   writes `name exit expect seconds` and nothing else, and the image tag is
   `graph-motor:$(content_hash "$stage")` (`scripts/service.sh:92`). The tree cites three different
   tags for image-backed rows — `4412ecaad92ed124` for `svc-limits`
   (`docs/measurements/service-caps.md:255`), `5145db1479a10e68` for the 55-row `svc-caps-time`
   (`:308`, and in `target/evidence/service-caps-time.report.tsv:1`), `5f01f980602fc2dd` for
   `service-image.rows` (`docs/contract/service-api.md:358`) — and nothing ties any of them to a commit.
   `scripts/service.sh image` must print its tag to a file each image-backed script reads, and
   `gate.sh` must copy it into `summary.txt`.
   **Negctl:** a row whose script does not print the tag leaves `image=?` in the summary.
4. **`Hooks::before_run` is behind `#[cfg(feature = "negctl")]`,** like every other break. It is
   carried from Round 2 finding 12 and is unchanged.
   **Row:** `svc-test` still compiles with the field gone from the release build; **negctl:** the four
   `negctl-` rows that use the hook still find it.
5. **The develop full gate and `hashgate --seeds 1000` run on the landing commit,** per
   `prompts/RESUME.md` and CLAUDE.md "Parallel branches". This is R2-5 restated as an acceptance
   criterion, with my judgement that the `crates/` delta is small enough to land first: 528 lines,
   `#![cfg(test)]` plus a CLI subcommand, nothing on the shipped surface.
   **Row:** `develop-full.rows`; **negctl:** n/a — a gate, not a break.

## Findings

| severity | file:line | defect | failure scenario | fix |
|---|---|---|---|---|
| high | `server/graph-server/src/config.rs:178,224-226`, `src/config/slots.rs:12,17-21` | `PER_SLOT_BYTES` is computed from a 64 MiB body while `GRAPH_MAX_BODY`'s range reaches 1 GiB, and nothing cross-checks them. The `Caveat:` at `config.rs:163-164` calls the variable "a buffer bound, not a work bound" and never a memory bound | an operator on the documented 8 GiB shape sets `GRAPH_MAX_BODY=1g` — in range, and the image (`deploy/service.Dockerfile:12`) sets nothing — and posts a contract body. The ingest peak scales with the body at the 18.25× `service-caps.md` measured, so ~19.6 GB is asked of a 1-slot, 8 GiB container. The kernel OOM-kills the process mid-request; every in-flight response dies, `HEALTHCHECK` fails, the container restart-loops, and no row is red: `svc-limits` runs at the default limit, `svc-caps-time` raises `GRAPH_MAX_BODY` to 1 GiB but only ever sends ~0.5 GiB **studio** bodies and never a contract one | new condition 1 |
| medium | `scripts/orch/gate.sh:24`, `target/evidence/*.summary.txt` | no summary records which image tag ran, and three different tags are cited across the tree | a `svc-image` PASS is attributed to a build nobody can identify; the Round 2 condition-6 claim "passed 5/5 on image `graph-motor:5f01f980602fc2dd`" cannot be checked against the evidence set, and `svc-limits`'s numbers come from a *third* tag | new condition 3 |
| medium | `docs/contract/service-api.md:115,318-320` | the operator-facing `GRAPH_WORKERS` default is `min(cores, memory.max / PER_SLOT_BYTES)` and the "As built" §3 repeats it; the code is `min(cores, (memory.max − BASE_BYTES) / PER_SLOT_BYTES)` (`src/config/slots.rs:30`). The table also never says 8 GiB is the floor, though `docs/deploy/service.md:50` does | an operator sizes a host from the contract's table, lands on a boundary where `BASE_BYTES` decides the answer, and gets either an unexplained exit 2 or one slot more than intended | new condition 7 below |
| low | `server/graph-server/src/app.rs:39-42` | `Hooks::before_run` is not behind `#[cfg(feature = "negctl")]`, unlike every other break | a public field compiled into the shipped binary; unreachable only because `from_settings` sets `Hooks::default()` | new condition 4 |
| low | `server/graph-server/tests/` (no match), `src/config/tests.rs:43` | `GRAPH_MAX_CONNECTIONS` and `GRAPH_MAX_HEADER_BYTES` **enforcement** has no test, and the preflight branch has no row of its own | the pre-accept permit at `src/serve.rs:93-104` or `max_buf_size` at `:118` is dropped, or a preflight is answered to an unconfigured origin, and every row in the gate stays green | add cases to `tests/preauth.rs` |
| low | `scripts/service-caps-time.sh:38`, `docs/measurements/service-caps.md:305,370` | the header still says "22 of the 50 rows of 2026-10-03" and the section says "All 50 rows answered 200" / "on all 50 rows" where the run covered 55 | a reader diffing the report against the doc finds a 5-row discrepancy and cannot tell whether five rows were skipped or added | say 55, or drop the count |

## Not verified here (UNKNOWN, and treated as FAIL)

- `develop-full.rows` and `hashgate --seeds 1000`. No summary exists in `target/evidence/`. Condition 1's
  process clause and Round-2 condition 5 are therefore *partly*, not met. I judged the merge-policy
  order sound; I did not and could not run the gate.
- Which image tag each of the 45 image-backed PASS rows exercised. `gate.sh` does not record it, and I
  was instructed not to run docker. I established instead that the summaries are *tree*-valid: the
  delta between `86362211` and `825e6a08` is 10 lines of `manifest.json`, read only by
  `svc-digest-wasm.sh` and `tests/digest/manifest.rs`.
- `svc-limits`'s four check lines. Only the summary's `PASS svc-limits exit=0` reached
  `target/evidence/`; the numbers I quote are the author's, from `docs/measurements/service-caps.md:268-296`.
  Exit 0 does require all four checks to pass (`scripts/service-limits.sh:230`), so the row's PASS is
  sound, but the peak figure itself is second-hand.
- The `GRAPH_MAX_BODY=1g` shape in finding 1. No row reaches it. My 19.6 GB figure is the documented
  18.25× ratio (`service-caps.md` "Memory per slot") applied to 1 GiB, not a measurement — which is
  exactly why condition 1 asks for a row rather than for the arithmetic to be trusted again.
