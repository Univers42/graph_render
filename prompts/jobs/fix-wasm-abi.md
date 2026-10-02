# Job fix-wasm-abi (agent build, review repairs: the wasm ABI and the JS SDK)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-base.md`.
Ids: F-01 (BLOCKER), F-16, F-17, F-18, F-30, F-31, F-32, F-33, F-79 … F-98.

Judgement notes:
- F-01 (`child_first` optional in `gm_build`). The contract is `docs/contract/wasm-abi.md` (the
  `gm_build` document, from line ~403). If it lists `child_first` as a required edge member, the
  reader is lax and the fix is to refuse its absence with the existing refusal code; first prove the
  SDK (`crates/graph-sdk-js`), the studio worker and every committed document already send it
  (`git grep -n child_first`), and paste that. If the doc says optional with a default, F-01 is
  `false` and the doc line is the evidence.
- F-17, F-18, F-30, F-31 (view lifetime: a `(ptr, len)` read after `gm_release` or after memory
  growth; an empty column's pointer). Fix inside the module where possible (an empty column
  returns a stable, documented value; a stale handle is refused with a refusal code). A change to
  what JS must do goes into `wasm-abi.md` and the SDK in the same change, additively.
- F-33 (no ABI version). Add `gm_abi_version() -> u32` (additive export), document it in
  `wasm-abi.md`, and make the SDK refuse a module whose version differs, with a message naming both
  numbers. RED: an SDK test that loads a module reporting another version and expects the refusal.

Paths: `crates/graph-wasm/**`, `crates/graph-sdk-js/**`, `docs/contract/wasm-abi.md`.

Done when, in addition to fix-common: `scripts/orch/gr cargo build -p graph-wasm --release --target
wasm32-unknown-unknown`, `scripts/orch/node-slim.sh npm run sdk:typecheck`, `npm run sdk:smoke`,
`hashgate --seeds 8` (exit 0) with its degree control (non-zero), then `scripts/studio.sh wasm`
and `scripts/studio.sh check` (the studio runs this SDK in its worker) all pass; paste each.
