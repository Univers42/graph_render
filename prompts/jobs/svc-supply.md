# Job svc-supply (agent build, branch svc-server-supply, worktree ~/goinfre/wt/svc-server-supply)

Contract: `docs/contract/service-api.md`, section "### Round 1", conditions 1, 7, 8. Read them verbatim.
The service is the separate Cargo workspace `server/`. Its header in `server/Cargo.toml` names
`scripts/orch/lock-parity.sh`. Seam: `crates/graph-wasm/src/service*` (`build`, `run`, `layout_ids`,
`post_ids`, `snapshot_of`, `post_pass`). Do NOT touch `server/graph-server/tests/{preauth,start,reload,
health,shutdown}.rs` or `scripts/orch/rows/service.rows`: another job owns them.

Already written by a previous worker, never run (commits 6eff9d41, 16c316b2):
`server/graph-server/tests/digest/{manifest.json,contract.json,n400.json,wasm-arm.mjs}`.

Exact tasks:
1. `scripts/orch/svc-features.sh` (C1): runs `cargo tree --manifest-path server/Cargo.toml -e features
   -i graph-wasm` under `scripts/orch/gr`; exit 1 if `probe` or `threads` appears, 0 if neither, 2 if
   cargo could not run. `--break` enables `probe` in a scratch copy of `server/` (under `target/`, never
   the real tree) and must exit 1. Header = its manual.
2. `scripts/orch/lock-parity.sh` (C8): for graph-wasm's whole normal-dependency closure, compare
   (crate, version, enabled features) between the root workspace and `server/` — use
   `cargo tree -e normal,features --prefix none` in each, `-p graph-wasm`, sorted. Exit 1 on any
   difference, printing it. `--break-version` / `--break-feature` edit a scratch copy (one version
   bump in its Cargo.lock / one extra feature) and must exit 1.
3. `server/graph-server/tests/digest.rs` (C7): read `tests/digest/manifest.json`; for each
   (fixture, source, layout, post) run the seam and compare SHA-256 of the binary snapshot with the
   manifest's hash. Hash the way `graph-cli hashgate` does (find it: `git grep -n sha2\\|Sha256 --
   crates/graph-cli/src`); do not invent another. Also check: the manifest covers both sources, every
   id of `layout_ids()`/`post_ids()` whose cap admits the fixture (assert this from the registry, so
   dropping an id from the manifest fails), and contains a 257–700-node component (`n400.json`).
   JSON face round-trips to identical bytes. Break: `GM_SVC_DIGEST_BREAK=1` flips one byte → test fails.
4. wasm32 arm: if `tests/digest/wasm-arm.mjs` can produce the same hashes from the real wasm artifact
   via `scripts/orch/node-slim.sh` (build: `scripts/orch/gr cargo build -p graph-wasm --release --target
   wasm32-unknown-unknown`), add `scripts/orch/svc-digest-wasm.sh` comparing them with the manifest.
   If it cannot, report it NOT RUN with the reason; never fake it.
5. Check `server/` sets no `target-cpu` or float flag the root lacks (`.cargo/config*`, `RUSTFLAGS`,
   profiles); put that check inside `svc-features.sh`.
6. Write rows into `scripts/orch/rows/service-supply.rows` (`name|expect|cmd`, see quick.rows), each with
   its `negctl-*` row ending `; test $? -eq 1`.

Paths allowed: `server/**`, `scripts/orch/svc-*.sh`, `scripts/orch/lock-parity.sh`,
`scripts/orch/rows/service-supply.rows`. Limits: ≤40 lines/function, ≤300 lines/file, BTreeMap not HashMap.

Done when: every row and negctl in service-supply.rows exits as declared; paste each command + exit.
