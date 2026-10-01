# Job trap-followups (agent build, follow-up to wasm-gm-build-trap)

Why: wasm-gm-build-trap (landed 2026-10-01) fixed a circo panic that trapped the wasm module
(`circo/blocks.rs` `Walk::parent` mixed local and global node indices). Its review left three findings:
1. MEDIUM — `packages/graph-studio/tests/session.motor.test.ts:141-160` (the `decode-fixtures` sweep) counts
   every non-decoded answer as `refused`, so a wasm **trap** in `layout` passes as a refusal; that is how the
   circo trap stayed green in this test. A trap must fail the test; only a refusal with a motor error code
   counts as refused. Make the test tell them apart from what the session returns (read
   `packages/graph-studio/src/motor/` for how a trap surfaces versus a refusal), and add a negative control
   proving a trap now fails it (a test-only hook, or a reasoned alternative stated in the return block).
2. LOW — `crates/graph-core/src/layout/graphviz/circo/blocks.rs` is 359 lines, over the 300-line house limit.
   Split it into child modules with no behaviour change; the circo tests and `hashgate --seeds 8` prove it.
3. LOW — `crates/graph-wasm/src/session.rs:~189`: `u32::try_from(address).unwrap_or(0)` turns an address or
   length above 4 GiB into 0 with no error code. Return the existing out-of-range `Code` (read `Code`) instead,
   with a unit test on the conversion.

Paths: the three files above, circo's new child modules, `crates/graph-wasm/src/**` tests,
`packages/graph-studio/src/motor/**` (read; edit only if a trap is indistinguishable without it).

Done when: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, the wasm32 build of graph-core,
`hashgate --seeds 8` and `scripts/studio.sh check` exit 0, and the trap negative control exits non-zero.
The return block pastes each real exit code. Leave everything uncommitted; the orchestrator commits.
