# Job git-plugin (agent build: a repository's commit graph as a studio document, outside the engine)

Why: the user wants to see any repository's commits, branches and merges through graph-render,
fast, using only the engine's public tools. The engine (motor, SDK, studio) must never learn
that this plugin exists (user rule, 2026-10-05). The plugin lives in `examples/plugins/git/`,
beside `examples/plugins/rows-file/`.

Your instructions are the plan: `docs/superpowers/plans/2026-10-05-git-plugin.md`. Execute
Tasks 1 to 3 in order, step by step, with the code it gives. The design is in
`docs/superpowers/specs/2026-10-05-git-history-source-design.md`.

Facts (develop, 2026-10-05; re-check each on your branch before editing, and stop if one no
longer holds):
- `crates/graph-sdk-js/src/adapters/rows.ts` exports:
  - `rowsToIngest(source: RowsSource): Ingest` (`:229`);
  - `RowsAdapterError` (`:23`);
  - the `Role` names `title label group tags link scalar weight parent` (`:29-38`).
- `RowsRecord.updatedAt` is a u32 (`:90-99`), and `cells.ts` skips a key whose value is
  `undefined` (absent cell).
- `crates/graph-core/src/ingest/build.rs:14-21`:
  - `titleField` gives the label, or the record id when the cell is absent;
  - `group` gives the group;
  - each `link` gives one `relation` edge per reference;
  - `tags` gives one hub node per value;
  - `scalar` gives nothing;
  - `updatedAt` gives `version` (`build/builder.rs:114`).
- The node id is `source:collection:record` (`fixtures/ingest/expected-graph.json`, `lib:task:t1`).
- `crates/graph-cli/src/ingest_cmd.rs:8-12`:
  `graph-cli ingest --from F --member ingest (--out G | --check G)`. Exit 0 pass, 1 differs,
  2 could not run.
- The studio's open-file action (`packages/graph-studio/src/actions/source.ts:48-60`) reads
  `{version, nodes, edges}` JSON. `packages/graph-studio/src/source/ingest.ts:80-93` keeps node
  `tags`, the member its `tag:#x` query reads.
- `examples/plugins/rows-file/sync.mjs` is the pattern for importing the SDK's `.ts` sources
  from a plugin, run with `node --experimental-strip-types`.
- Bare clones for the bench: `~/goinfre/gitviz/{contributor-stats,activitywatch,aw-server-rust,git}.git`
  (488, 1271, 989 and 85,928 commits).

Rules (beyond `scripts/orch/common.md`):
- Node only through `scripts/orch/node-slim.sh`; cargo only through
  `CARGO_BUILD_JOBS=3 scripts/orch/gr`.
- Never take `~/goinfre/orch/timed.lock`. The bench runs only under
  `flock ~/goinfre/orch/bench.lock`, with the preconditions the plan gives.
- If the contract, the SDK or the studio refuses something the plan expects, that is a finding.
  Write it under "decisions needed", with the exact message. Never edit `crates/`, `packages/`,
  `app/`, `src/`, `server/`, `harness/` or `fixtures/`.

Paths you may touch: `examples/plugins/git/**` and `docs/measurements/git-plugin.md`.
`scripts/orch/rows/git-plugin.rows` is already on develop: read it, do not edit it.

Done when:
- `scripts/orch/gate.sh target/rows-git-plugin scripts/orch/rows/git-plugin.rows` writes a
  `summary.txt` with every row PASS;
- `docs/measurements/git-plugin.md` holds the bench table, the end-to-end git/git number with
  container start-up apart, and the findings.

Return: the plan's return block, filled.
