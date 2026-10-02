# Job graphviz-verdict (agent build, follow-up to p13-gv1-osage-gate)

Why: osage-gate (landed 2026-10-01) measured `layout.packing.osage` against Graphviz 16.1.0 over 1000 seeds at a
worst gap of 6.309e-2 pt under a 1e-1 ceiling, with its negative controls red, but could not flip the row to
`gated`: `Status::Gated` needs an evidence arm, and `crates/graph-cli/src/capabilities/verdict.rs:52-74` reads
only `oracle-diff`, `oracle-layouts`, `oracle-fa2` and `oracle-spectral`. With the row set to Gated,
`capabilities --check` prints "gated, but no oracle-osage record: run the gate".

Do:
1. Read `verdict.rs`, `capabilities/registry/unproven.rs`, `capabilities/tests/registry.rs` and
   `oracle_python/graphviz.rs` (what record name `oracle-graphviz --engine <e>` writes under `target/gates/`).
2. Add the evidence arm(s) for the Graphviz differential records in one place, data-driven from the
   `ENGINES` table rather than one hand-written arm per engine, so a new engine needs no verdict edit.
3. Flip only `layout.packing.osage` to `gated` (its record must be current for the tree). twopi, circo,
   patchwork, neato and fdp stay `implemented`: their measurements explain why.
4. Tests: osage reads `gated` with a current record; a stale or missing record reads as not gated (the
   negative control); the other Graphviz rows are unchanged.

Paths: `crates/graph-cli/src/capabilities/**`, `crates/graph-cli/src/oracle_python/graphviz.rs` (read, and
an export if needed), `docs/measurements/p13-gv1-osage.md` (one line). Nothing else.

Done when: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, `capabilities --check` exit 0,
after re-running `emit-graphviz-fixtures --engine osage`, the oracle and `oracle-graphviz --engine osage`
over the 1000 seeds on the final tree so the record is current. The return block pastes each real exit code.
Leave everything uncommitted; the orchestrator commits.
