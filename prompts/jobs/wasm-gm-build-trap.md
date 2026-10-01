# Job wasm-gm-build-trap (agent build, repair)

Why: studio-smoke (2026-10-01) found that `scripts/studio.sh check` is red on develop (713f077 and later).
`packages/graph-studio/tests/session.motor.test.ts:140` ("every bundled fixture decodes under every layout
that accepts it") dies with `RuntimeError: unreachable` inside `gm_build`: the `load` that follows
`fixtures/force/disconnected.json` being laid out under the force layouts traps instead of returning a handle
or a refusal. A wasm trap is a Rust panic under `panic = "abort"`; the ABI promises `0` plus `gm_last_error`
on any refusal (`docs/contract/wasm-abi.md`), never a trap.

Do:
1. Reproduce first: `scripts/studio.sh wasm`, then `scripts/studio.sh test`; note the failing case.
2. Find the panic at its root. Rebuild graph-wasm with debug assertions or run the same call sequence
   natively (a `#[test]` in `crates/graph-wasm` or graph-core that replays: build disconnected.json, run each
   force layout, release, build the next fixture). Suspects to rule in or out, by evidence: the handle table
   after `gm_release`, an allocation the caller frees twice, an index or `unwrap` in a force layout on a
   disconnected graph that poisons shared state.
3. Fix it in the shared function, not in the test or the studio. If the input is truly invalid, the fix is
   a refusal with an error code, not a trap.
4. One regression test at the level of the fix (cargo test), red before the fix and green after.

Paths: `crates/graph-wasm/**`, `crates/graph-core/src/**` (only the function at fault), and the test.
Nothing in `packages/` unless the studio itself double-frees, which you must prove.

Done when: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, the wasm32 build of
graph-core, `hashgate --seeds 8` exit 0; `scripts/studio.sh wasm` then `scripts/studio.sh check` exit 0.
The return block pastes each command's real exit code and names the root cause with file:line.
Leave everything uncommitted; the orchestrator commits.
