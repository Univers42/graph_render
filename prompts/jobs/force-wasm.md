# Job force-wasm (agent build, wasm ABI for the live force session)

Why: the live-forces chain (plan step 6). `sim` is on develop: `crates/graph-core/src/layout/force/session.rs`
(`Session::new`, `from_frozen`, `from_positions`, `step`, `set_params`, `xs`, `ys`, `alpha`, `reheat`,
`set_alpha_target`; pin/drag in `session/pin.rs`; contract in `docs/decisions/live-force-session.md`).
The studio branch `studio-force` (not merged, `git diff develop...origin/studio-force -- packages`) already
calls a force session from the worker; this job gives it a wasm ABI to call. Studio wiring is the next job.

Do:
1. Derive the ABI from what `studio-force` calls (read its `packages/graph-studio/src/motor/` worker and
   protocol) and from `Session`'s public API. Write it first as a table in
   `docs/decisions/force-wasm-abi.md`: export name, params, return, error codes, who owns the memory.
2. Implement it in `crates/graph-wasm` as `extern "C"` exports in a new module (`src/session.rs` +
   children), following the existing handle pattern (`src/handle.rs`, `src/errors.rs`, `src/views.rs`):
   create from a loaded graph handle (+ params), tick(n) returning a status, set params, pin/unpin, drag
   (pin at a point), reheat, positions as a zero-copy f64 view (`ptr`,`len`), free. No wasm-bindgen.
   Wire integers are u32/u64 (CLAUDE.md "Determinism").
3. SDK: `crates/graph-sdk-js` gets a typed `ForceSession` wrapper over those exports, type-checked by
   `scripts/orch/node-slim.sh npm run sdk:typecheck`, and one case in `sdk:smoke` that creates a session,
   ticks, drags one node and sees a neighbour move.
4. Determinism: a Rust test that N ticks through the wasm-facing functions (called natively) produce the
   same bits as `Session::step(N)` directly; and a hashgate-style native-vs-wasm32 check of the positions
   after 50 ticks for seeds 0..3 (reuse `crates/graph-cli/src/hashgate/` machinery if it fits; if not,
   say why and add a `graph-cli` test that runs the wasm artifact under Node like hashgate's arms do).
   Its negative control: perturb one tick (an existing `GM_MUTATE_*` knob if one reaches the session,
   else a new one added to the single knob list) and show the check turns red.

Checks (paste each last line): the quick.rows set (fmt, clippy -D warnings, `cargo test --workspace
--no-fail-fast`, wasm32 graph-core, hashgate 8 + negctl), `cargo build -p graph-wasm --release --target
wasm32-unknown-unknown`, `sdk:typecheck`, `sdk:smoke`, the new native-vs-wasm check and its negctl.

Paths you may touch: `crates/graph-wasm/**`, `crates/graph-sdk-js/**`, `crates/graph-cli/**` (the check
and its knob only), `docs/decisions/force-wasm-abi.md`. Not `packages/`, not graph-core's math.

Done when: the ABI doc exists, the checks pass, and the negctl fails as required.
