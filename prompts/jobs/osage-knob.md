# Job osage-knob (agent build, follow-up to graphviz-verdict and p12-t3-knobs)

Why: graphviz-verdict (landed 2026-10-01) taught `capabilities/verdict.rs` to read any oracle record by its
name, and `oracle-osage` reads and passes (1000 cases, worst 6.309e-2 pt, ceiling 1e-1). The row
`layout.packing.osage` still cannot be `gated`: no hash-gate control perturbs the osage stage alone
(`hashgate --seeds 8` with `GM_MUTATE_REFERENCE_DEGREE=9` and with `GM_MUTATE_PACKING_SCALE=2` both leave
`layout.packing.osage` equal). That job's recommendation: a per-stage knob such as `GM_MUTATE_OSAGE_NODES`
recorded as `hashgate-control-packing-osage-nodes`, and a `negctl-osage-nodes` row.

Do:
1. Read `hashgate/{knob.rs,knobs.rs,stages.rs}`, `hashgate/knob/*.rs`, `hashgate/tests/knob/table.rs`,
   `crates/graph-cli/tests/common/mod.rs`, and the per-layout knobs p12-t3-knobs added (the model to follow).
2. Add the osage knob the same way; it must make `hashgate --seeds 8` exit 1 naming `layout.packing.osage` as
   the first divergence.
3. Flip `layout.packing.osage` to `Status::Gated` in `capabilities/registry/unproven.rs`, with its comment
   rewritten to say what now backs it. `capabilities/tests/graphviz.rs` already expects this.
4. Add the `negctl-osage-nodes` row (expect non-zero) to `scripts/orch/rows/quick.rows` only if quick.rows
   already carries per-layout negctl rows; else to a new `scripts/orch/rows/osage.rows`. Say which.
5. Re-run the osage differential (`emit-graphviz-fixtures --engine osage`, the oracle image,
   `oracle-graphviz --engine osage`) and `hashgate --seeds 8` plus the knob on the final tree, so the records
   are current, then `capabilities --check`: the osage row must read `gated` with no problem of its own.

Paths: `crates/graph-cli/src/hashgate/**`, `crates/graph-cli/tests/common/mod.rs`, `crates/graph-cli/tests/**`
(knob tests), `crates/graph-cli/src/capabilities/**`, `scripts/orch/rows/`, `docs/measurements/p13-gv1-osage.md`.

Done when: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast` exit 0; the knob run exits 1
naming the osage stage; `capabilities --check` lists no osage problem. The return block pastes each real exit
code. Leave everything uncommitted; the orchestrator commits.
