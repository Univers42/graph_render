# Job review-harness-sdk (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). An oracle arm that diverges from the library it wraps makes every differential against it meaningless; the SDK is the public surface a third party reads.

Scope: `harness/**` other than `scigraphs-conformance*` (the oracle arms: `oracle-*.mjs`, `oracle-*.py`, `wasm-run.mjs`, `sdk-smoke*`, `stress-d3.mjs`), and `crates/graph-sdk-js/**` (the TypeScript wrapper over the wasm ABI).

Rules, checks and output shape: exactly as `prompts/jobs/review-core-post.md` says (read it first),
with `docs/reviews/review-harness-sdk.md` as the report and the only path you write. For the harness: does each arm call the library it names with the parameters the fixture states, read the fixture rather than regenerate it, and fail (non-zero) rather than print on a missing input. For the SDK: `docs/contract/wasm-abi.md` is the spec; check every export's argument validation, buffer ownership (`gm_alloc`/`gm_free` pairs on every path, refusal included), and that column views are re-read after any call that can grow memory.
Review `origin/develop` as it is when you start; fan out one `explore` subagent per module in ONE
message, then merge, deduplicate and verify each finding yourself.

Done when: every module in scope is named in the report with its finding count (0 is a valid count);
every BLOCKER/MAJOR row has a command and its output or a reference `file:line`.
