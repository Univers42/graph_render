# Job gpu-g1a, fix round 1 (agent build, same branch, same session)

The orchestrator reviewed your branch and agrees with your no-source-names finding.
`crates/graph-core/src/layout/force/session/tests/golden.rs:12-13` was red before this job
started. The row on develop now excludes that file (`scripts/orch/rows/gpu-g1a.rows`). Merge
`origin/develop` into your branch with `git fetch origin && git merge -m updated origin/develop`
to get the row. Do not touch `golden.rs`.

The review found house-limit breaches in code this slice added. Fix each one. Change no
behaviour, no byte on the wire, and no test's expected value.

1. **`crates/graph-core/src/layout/force/session.rs` is 305 lines** (develop: 296); the limit is
   300.
   - Move the `#[cfg(test)] pub(crate) fn tick_no` accessor (`:287-292`) out of `session.rs`.
   - Put it in `session/mesh_probe.rs`, inside a `#[cfg(test)] impl ForceSession` block. A child
     module can read `self.sim`.
   - `session.rs` must end ≤ 300 lines.
2. **`crates/graph-cli/src/gpu_fixtures/emit.rs`:**
   - `write` (`:96-147`) is 52 lines with 5 parameters;
   - `header` (`:163`) has 6 parameters.

   The limits are ≤ 40 lines and ≤ 4 parameters.
   - Bundle what describes one fixture (the probe, `xs`, `ys`, `state`) into one borrowed struct.
   - Move the finite checks (`:104-119`) into their own function.
   - Each function must end ≤ 40 lines and ≤ 4 parameters.
3. **`crates/graph-cli/src/gpu_fixtures/check.rs:89` `compare_one`** has 5 parameters. Bring it
   to ≤ 4: the same struct, or pass `(dir, name)` as one value.
4. **`crates/graph-cli/src/gpu_fixtures/tests.rs`:**
   - `the_header_carries_the_state_and_the_rung` is 44 lines;
   - `every_integer_on_the_wire_is_u32` is 54 lines.

   Each must be ≤ 40. Extract a helper, or split one test into two named for what each pins.
   Keep every assertion.
5. **`crates/graph-core/src/layout/force/session/fidelity.rs:42-44` `PROBE_WORKERS`.** Its doc
   says "as a free function", but it is a const that only aliases `WORKERS`. Delete it: make
   `WORKERS` `pub(super)` and use it in `mesh_probe.rs`.

How to measure:
- Function length: from the `fn` line to its closing `}` at the same indent, inclusive.
- Parameters: count `self` as one.

Done when:
- `scripts/orch/gate.sh target/rows-gpu-g1a scripts/orch/rows/gpu-g1a.rows` writes a
  `summary.txt` with every row PASS, on the merged tree;
- `wc -l crates/graph-core/src/layout/force/session.rs` prints ≤ 300.

Return:
- the branch tip;
- for each of the five items, the file:line after the fix and its line or parameter count;
- each row's result;
- every deviation.
