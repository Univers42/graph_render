# Job fix-core-base (agent build, review repairs: graph-core topology base)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-base.md`.
Ids: F-03, F-06, F-20, F-21, F-22, F-23, F-24, F-35 … F-46.

Judgement notes:
- F-06 (degree pass in `weights.rs` is a scatter). Read `prompt.md` §6 D10 verbatim. If D10 binds
  per-step kernels of iterative layouts only, this one-pass ingest step is outside it: record `false`
  with the quote. If D10 binds every pass, rewrite as a gather over the CSR and prove the output is
  byte-identical on `hashgate --seeds 8`.
- F-03, F-20, F-21 (silent truncation, panics, overflow). Wire integers are `u32` (D7). Fix by
  refusing (an error value) where the function already returns a `Result`, by `expect` with a
  message where the caller guarantees the bound; a new public error variant is additive.
- F-44 … F-46 (`synthetic.rs`). The synthetic generator feeds the hash gate and committed fixtures:
  a change that alters the graph generated for any existing seed is "decisions needed". Refusing an
  out-of-range parameter is fine.

Paths: `crates/graph-core/src/{csr.rs,csr_petgraph.rs,index.rs,index/**,diff.rs,arena.rs,
columns.rs,legend.rs,ids.rs,stage.rs,synthetic.rs,weights.rs}`.

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with its degree control
non-zero; paste both.
