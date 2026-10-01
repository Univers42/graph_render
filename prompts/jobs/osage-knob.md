# Job osage-knob (agent build, follow-up to graphviz-verdict)

Why: graphviz-verdict (2026-10-01) taught the ledger to read every Graphviz differential record, but could not
flip `layout.packing.osage` to `gated`: `Status::Gated` needs a negative control that turns the hash gate red
**on the row's own stage**, and none exists. Its diagnostic run: `hashgate --seeds 8` with
`GM_MUTATE_REFERENCE_DEGREE=9` or `GM_MUTATE_PACKING_SCALE=2` exits 1, but neither diverges
`layout.packing.osage` (finding HIGH, `crates/graph-cli/src/hashgate/knobs.rs:33,127`). A second, older finding:
`crates/graph-cli/tests/common/mod.rs:17` (`KNOBS`, the one knob list) lacks `GM_MUTATE_SPLIT_SUM`.

Do:
1. Read `hashgate/{knob.rs,knobs.rs,stages.rs}`, `hashgate/knob/{arms,records,setting,igraph}.rs`,
   `crates/graph-cli/tests/common/mod.rs`, `capabilities/tests/graphviz.rs` and `capabilities/verdict.rs`.
   If p12-t3-knobs has landed on develop, follow the per-layout knob pattern it added.
2. Add one knob (e.g. `GM_MUTATE_OSAGE_NODES`) that perturbs the osage stage alone, with its control record
   name, following `igraph.rs`.
3. `hashgate --seeds 8` with it set exits 1 and names `layout.packing.osage` as the first divergence; without
   it, exit 0.
4. Add the row `negctl-osage-nodes|nonzero|...` to `scripts/orch/rows/develop-full.rows` next to the other
   negctls.
5. `KNOBS`: add `GM_MUTATE_SPLIT_SUM` if it belongs there, or state with `file:line` why the list excludes it.
6. Flip `layout.packing.osage` to `gated` in `capabilities/registry/unproven.rs` only if the tests in
   `capabilities/tests/graphviz.rs` prove the ledger accepts it with the new control and a current record;
   otherwise leave it `implemented` and say what is missing.

Paths: `crates/graph-cli/src/hashgate/**`, `crates/graph-cli/src/capabilities/**`,
`crates/graph-cli/tests/common/mod.rs`, `crates/graph-cli/tests/**` (knob tests only),
`scripts/orch/rows/develop-full.rows` (one row), `docs/measurements/p13-gv1-osage.md` (a section).

Done when: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, `hashgate --seeds 8` exit 0,
the knob's run exits 1 with the osage stage named. The return block pastes each real exit code.
Leave everything uncommitted; the orchestrator commits.
