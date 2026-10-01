# Job p12-t3-knobs (agent build, follow-up to p12-t3)

Why: p12-t3 (landed 2026-10-01) appended five 3D layouts to `LAYOUTS` (`layout.basic3d.sphere`,
`layout.basic3d.helix`, `layout.basic3d.cube`, `layout.hierarchical3d`, `layout.force.spring3d`), but no
hash-gate negative control reaches any one of them alone. Its findings:
- `crates/graph-cli/src/hashgate/stages.rs:100`: the `GM_MUTATE_SPRING_ITERATIONS` arm matches `Spring::ID`
  only, so no knob perturbs `layout.force.spring3d` (MEDIUM).
- The three controls it shipped (node-count, node-z, spring-iterations) cover the group, but none isolates
  a single stage, and `negctl-node-z` exited 2 (could not run) in its gate rather than 1.

Do:
1. Read `hashgate/{knob.rs,knobs.rs,stages.rs}`, `hashgate/knob/{arms,records,setting,igraph}.rs`,
   `hashgate/tests/knob/table.rs` and `crates/graph-cli/tests/common/mod.rs` (the one knob list).
2. Add one knob per new layout that perturbs that layout's stage alone, following the existing arm pattern
   (`igraph.rs` is the closest model: one knob per layout). Extend the spring-iterations arm to the 3D id
   only if it is the same kernel parameter; say which in the return block.
3. Find why `negctl-node-z` exits 2 (read p12-t3's rows file under `scripts/orch/rows/` and
   `docs/measurements/p12-t3.md`) and fix the cause, so it fails for the right reason (exit 1).
4. Each new knob is a hashgate negative control: `hashgate --seeds 8` with it set must exit 1 and name
   that layout's stage as the first divergence; without it, exit 0.

Paths: `crates/graph-cli/src/hashgate/**`, `crates/graph-cli/tests/common/mod.rs`,
`crates/graph-cli/tests/**` (knob tests only), the p12-t3 rows file, `docs/measurements/p12-t3.md` (a section).

Done when: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`, `hashgate --seeds 8` exit 0,
and each new knob's run exits 1 with its stage named. The return block pastes each real exit code.
Leave everything uncommitted; the orchestrator commits.
