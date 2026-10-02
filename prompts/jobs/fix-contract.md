# Job fix-contract (agent build, review repairs: graph-contract)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-base.md`.
Ids: F-02, F-08, F-09, F-10, F-11, F-27, F-28, F-29, F-66 … F-78, F-99.

Judgement notes:
- F-02 (no exponent form). `docs/contract/binary-layout.md:336-337` says numbers are Rust's `f32`
  `Display`, "never an exponent". The JS engine is the oracle for the 17 `core/model` functions,
  not for canonical JSON. Unless you find a reader in this tree that parses the JSON face with an
  exponent expectation, F-02 is `false`, with the doc line as evidence.
- F-27 (`-0`). The JSON face must round-trip to identical binary bytes (`graph-cli roundtrip`);
  `-0` reads back as `-0.0`, `0` would not. Expected verdict `false`; prove it with a roundtrip
  test on a `-0.0` coordinate, and add one sentence on `-0` to the Numbers bullet of
  `binary-layout.md`.
- F-29 (`"z": null`). The rule is `binary-layout.md:326-331` (`z` array iff `dim` = 1). Whichever of
  the reader and the schema disagrees with that rule is the defect. The schemas are generated:
  change the generator in `graph-contract`, then `graph-cli codegen` rewrites them; never hand-edit
  `docs/contract/*.json`.
- F-08 … F-11, F-71 … F-76 (ingest validation). Tightening the reader must not refuse any committed
  document: run the reader over `fixtures/ingest/*.json` and every adapter fixture, and paste that
  they all still pass. A committed document the fix would refuse is "decisions needed".

Paths: `crates/graph-contract/**`, `docs/contract/binary-layout.md`, the generated
`docs/contract/*.json` and TypeScript declarations (through `codegen` only).

Done when, in addition to fix-common: `graph-cli codegen --check`, `capabilities --check`,
`roundtrip --seeds 100` and `hashgate --seeds 8` exit 0 (paste each).
