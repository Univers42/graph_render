# Job fix-core-ingest (agent build, review repairs: graph-core ingest)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-core-base.md`.
Ids: F-12, F-13, F-14, F-15, F-19, F-100 … F-109.

Judgement notes:
- F-12 (edge label and edge id derived from `Field::name`). The contract says the name is for
  diagnostics only and nothing derives from it (`crates/graph-contract/src/ingest.rs:181`), so the
  code is the defect. First check what the TypeScript oracle derives the id from (`src/`); if the
  oracle also uses the name, stop and report "decisions needed" (two sources disagree). Otherwise
  derive from the field id, regenerate the expected files with their own generator (never by hand),
  and list in the report every committed fixture whose edge ids changed. A changed id outside
  `fixtures/ingest/` (studio, SDK, hashgate inputs) is "decisions needed".
- F-15 (`Role::Group` unread). Either the role is wired to what `ingest-schema.json` says it does,
  or the doc is corrected; the contract decides.
- F-19 (`first_with_role` depends on insertion order). Determinism (D3): the result must not
  depend on map order; pin the order the contract documents.

Paths: `crates/graph-core/src/ingest.rs`, `crates/graph-core/src/ingest/**`, the expected files
under `fixtures/ingest/` (regenerated only).

Done when, in addition to fix-common: `hashgate --seeds 8` exits 0 with its degree control
non-zero, and the ingest differential tests (`cargo test -p graph-core ingest`, `-p graph-cli`
tests whose name contains `ingest`) pass; paste each.
