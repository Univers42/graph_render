# Job studio-smoke (agent build, studio follow-up to fix-stale-wasm)

Why: on 2026-10-01 the dev studio died on load with `TypeError: exports.gm_dim is not a function` and the
"Unexpected studio error" overlay. `scripts/studio.sh` staged a stale `target/` wasm (fixed: staging now
rebuilds it; the SDK loader refuses a module missing any `RawExports` name). No gate looks for a page
error on load: each browser gate checks its own rows only. Second defect: `studio.sh` run in a fresh
worktree creates `target/` as root (the node container), and `queue.sh land` then cannot write
`target/land-<label>/`.

Do:
1. Read `scripts/studio.sh`, `scripts/studio-live.sh`, `deploy/nav/drive.py`, `deploy/perf/cdp.py` first.
2. `scripts/studio.sh`: `mkdir -p "$root/target"` before any container runs, so `target/` is the user's.
3. New gate `scripts/studio-smoke.sh` over app/dist in the gm-chromium image (same shape as studio-live.sh,
   `deploy/nav/smoke.py`): open the studio, collect `Runtime.exceptionThrown`, console `error` calls and
   `Log.entryAdded` errors from load until the drawing settles, read `studio.store.get().error` and the
   shadow-root text. Rows: `smoke-no-exception`, `smoke-no-console-error`, `smoke-no-store-error`,
   `smoke-no-overlay`, `smoke-drew-nodes` (node count > 0). Save a screenshot under target/studio-smoke/.
4. Negative control `STUDIO_SMOKE_BREAK=1`: serve a wasm that exports `memory` only in place of
   graph_wasm.wasm; every row must go red.
5. Add the gate to the header list in CLAUDE.md's studio command block (one line, `scripts/studio-smoke.sh`).

Paths: `scripts/studio.sh`, `scripts/studio-smoke.sh`, `deploy/nav/**`, `CLAUDE.md` (one line). Nothing else.

Use the `pw` MCP on the dev server (`scripts/studio.sh` on 5174) to confirm your rows read what a user sees.

Done when: `scripts/studio.sh check` exits 0; `scripts/studio.sh build` then `scripts/studio-smoke.sh` exits 0
with every row PASS; `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits non-zero with every row FAIL.
The return block pastes each command's real exit code.
