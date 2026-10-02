# Job fix-wasm-ingest (agent build, follow-up to fix-wasm-abi: F-16, sdk:test)

Read `prompts/jobs/fix-common.md` first. Source: `docs/decisions/wasm-ingest-limits.md` (decided;
read it whole) and the "Decisions needed" items 1 and 4 of `docs/measurements/fix-wasm-abi.md`.
Ids: `F-16`, `SDK-TEST`. F-01 and F-80 are decided "no change": give each a row, verdict `doc-only`,
citing the decision record.

F-16. `crates/graph-wasm/src/ingest.rs` `read` parses any length.
1. Measure first, as the decision record's steps 1–4 say. Write the generator as a `graph-cli`
   subcommand or a `harness/` script that emits the documents to `target/` (never commit them); run
   `gm_build` on each under `scripts/orch/node-slim.sh` against the real
   `cargo build -p graph-wasm --release --target wasm32-unknown-unknown` artifact. Paste a table:
   nodes, edges, bytes, built or trapped. Bound each run with `timeout`; a timeout is "could not
   run", never "built".
2. RED: a document one byte past the measured `MAX_INGEST_BYTES` is refused with the new code, and
   one at exactly the ceiling is not refused for its length. GREEN: the length check before
   `from_utf8`, in `read` or `gm_build`, whichever already owns the refusal path.
3. The new refusal code is appended after the last existing one (wire order is the contract:
   `errors/mirrors.rs`'s `every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order` must
   stay green), named in `docs/contract/wasm-abi.md` and in the SDK's code list under
   `crates/graph-sdk-js/src/`.
4. The constant's doc names the measurement (`docs/measurements/fix-wasm-ingest.md`) and carries the
   decision record's `Ponytail:` line.

SDK-TEST. Add `"sdk:test": "node --test crates/graph-sdk-js/test/"` to the root `package.json`
scripts (check whether the pinned Node needs `--experimental-strip-types` for the `.ts` imports;
`abi-version.test.mjs` is the one file today). Paste
`scripts/orch/node-slim.sh npm run sdk:test` with its last lines. Then confirm
`scripts/orch/node-slim.sh npm run sdk:smoke` still exits 0.

Paths: `crates/graph-wasm/src/ingest.rs`, `crates/graph-wasm/src/ingest/**`,
`crates/graph-wasm/src/errors/**`, `crates/graph-wasm/src/lib.rs` (additive), `docs/contract/wasm-abi.md`,
`crates/graph-sdk-js/src/**` (the code list only), `crates/graph-sdk-js/test/**`, `package.json`
(scripts only), `harness/` (a new generator file only), `crates/graph-cli/src/**` (a new subcommand
only, additive in `cli.rs` / `main.rs`), `docs/measurements/fix-wasm-ingest.md`.

Done when: fix-common's done-when; the measurement table; `npm run sdk:test` and `npm run sdk:smoke`
exit 0; `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` and `codegen --check`
exit 0.
