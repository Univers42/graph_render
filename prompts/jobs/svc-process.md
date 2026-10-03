# Job svc-process (agent build, branch svc-server, worktree ~/goinfre/wt/svc-server)

Contract: `docs/contract/service-api.md`, section "### Round 1", conditions 3, 4, 9, 11, 12. Read them
verbatim first. The service is the separate Cargo workspace `server/` (crate `server/graph-server`).
Build/test it with `scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace <filter>`.
Do NOT touch conditions 1, 7, 8 (svc-features, digest, lock-parity): another job owns them.

Already written by a previous worker, never run (commit afbd86f6): `server/graph-server/tests/common/child.rs`
(process harness), `tests/preauth.rs` (C4), `tests/start.rs` + `tests/reload.rs` (C9), `tests/health.rs` (C11).

Exact tasks, in order:
1. Run `tests/preauth.rs`, `tests/start.rs`, `tests/reload.rs`, `tests/health.rs` with a name filter.
   Fix every failure at its root (in `src/` when the server is wrong, in the test when the test is).
2. Write `tests/shutdown.rs` (C12): start the server, open a slow in-flight request, send SIGTERM; the
   request completes with its normal status, then the process exits 0 within its drain budget; a new
   connection after SIGTERM is refused. Use the existing `common/child.rs` harness.
3. Negative controls. Each test above has a break in `src/breaks.rs` style (read it: how existing breaks
   are switched on). Add the missing breaks so that each of these tests turns red for its own reason:
   `preauth` (buffer the body before auth), `slow headers` (no header timeout), `sighup` (ignore SIGHUP),
   `exit 2` (accept a group-writable key file), `healthcheck` (healthcheck always exits 0),
   `shutdown` (no drain: exit at once on SIGTERM). A control passes only when the test exits 1 and its
   failure message is the control's own assertion.
4. Create `scripts/orch/rows/service.rows` in the `name|expect|cmd` format of
   `scripts/orch/rows/quick.rows`: one row per test file above and per condition 2 / 11 test that already
   exists (`tests/caps.rs`, `tests/health.rs`), each followed by its `negctl-*` row
   (`...; test $? -eq 1`). Add row `svc-memory` (C3): run the built image's server under
   `scripts/orch/drun --memory 8g` and assert the worker count it logs is the one `src/caps.rs` derives
   for 8 GiB (2); its negctl gives `--memory 1g` and expects the refusal to start: non-zero exit and the refusal message of `src/caps.rs` (read it for the exact exit code).
   If the image cannot be built in this job, write the row anyway and report it NOT RUN.
5. Append a short "As built" note per condition 3, 4, 9, 11, 12 to `docs/contract/service-api.md`
   (what holds, at file:line, and which row proves it). No other doc edits.

Paths allowed: `server/**`, `scripts/orch/rows/service.rows`, `docs/contract/service-api.md`.
Limits: ≤40 lines per function, ≤4 params, ≤300 lines per file; `Caveat:` on every timeout/estimate.

Done when: `scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast`
exits 0, and every negctl in service.rows exits 0 (i.e. its test exits 1). Paste each command + exit.
