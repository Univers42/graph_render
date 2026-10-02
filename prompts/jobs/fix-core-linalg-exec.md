# Job fix-core-linalg-exec (agent build, review repairs: linalg and the execution tiers)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-base.md`.
Ids: F-04, F-05, F-07, F-25, F-26, F-47 … F-65.

Judgement notes:
- F-25 (`Caps` differ native vs wasm, so the tier differs). Tiers may differ by host; their output
  must not. Find the test or gate that proves every tier gives identical bytes (`force-gate`, the
  tier jobs' tests, `git grep -n Threads crates/*/tests`). If one exists, F-25 is `false` with its
  name. If none does, add one test that runs each tier on one fixture and compares bytes.
- F-05 (5 parameters). Bundle into a struct; update callers inside graph-core and graph-cli. A
  caller in graph-wasm is "decisions needed" (fix-wasm-abi owns it).
- F-04, F-07 (whole-slice scan; `n == 0` panic). Spectral and Pivot MDS run on this code: the
  conformance rows SPECTRAL_3D and MDS_3D must not move.

Paths: `crates/graph-core/src/linalg/**`, `crates/graph-core/src/exec/**`, callers of
`exec::select::resolve` in graph-core and graph-cli.

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with its degree control
non-zero, and `force-gate --seeds 4` exits 0; paste each.
