# Job gpu-g1-verdict (agent devil, docs only: rule on the GPU tier's G1 public surface before code)

Why: the plan `docs/superpowers/plans/2026-10-06-gpu-g1.md` puts the GPU force tier's G1 slices
into code. Two of its slices add public surface, which needs a verdict before code under the
house rule (`.claude/rules/devil/risk.md`):
- **G1a** adds graph-core `pub` items (`MeshPass`, `MeshGraph`, `MeshFrame`, `MeshSolution` and
  three `ForceSession` methods), re-exported from `layout/force/mod.rs`.
- **G1d** adds a `ForceEngine` value to the SDK.

You rule on both, on the plan as written. You write no code.

Read, in full:
- `docs/decisions/gpu-force-tier.md` (accepted 2026-10-03) and `docs/decisions/compute-tiers.md`;
- `docs/measurements/gpu-adapter.md` (G0's findings);
- the plan: its Global Constraints, Review Focus, Task 1 (G1a) and Task 4 (G1d) in full, and the
  bounds in Tasks 2 and 3;
- every file the plan's G1a "Why no existing item serves" paragraph cites, at the lines it cites.

Facts (develop, 2026-10-06; re-check each on the tree you start from):
- `crates/graph-core/src/layout/mod.rs:18` makes `force` a `pub mod`, so anything `pub` in
  `force/mod.rs` is graph-core's public surface.
- graph-core's dependencies are a closed list (`libm`, `indexmap`, `petgraph`). It has no
  `unsafe`, no I/O and no clock, and it builds for `wasm32-unknown-unknown` (`CLAUDE.md`).
- `fixtures/` is fingerprinted (`crates/graph-cli/src/fingerprint.rs`). Committing the two
  `.gmfx` goldens voids every gate record until the gates run again.
- The decision record says the tier is not in graph-core and is not hash-gated
  (`gpu-force-tier.md:36-38`). It is reproducible per device, not bitwise across devices.

Rule on each of these. For each, one line: OK, or a condition the build job must meet.
1. **G1a's `pub` items.** Are all seven needed `pub`? Rule on these alternatives:
   - a cargo feature that only graph-cli turns on (no new dependency);
   - `#[doc(hidden)]`;
   - one probe function instead of three methods.

   Say which, if any, is required, and why.
2. **No change to the motor's output.** The probe copies the session (`mesh_pass_deltas`) and
   re-solves the mesh (`mesh_solution`). Can either one change a later tick, a hash, or a
   session's state? Name the line that guarantees it, or make it a condition with a test name.
3. **The fixture format.** `.gmfx` is a new binary format, committed under `fixtures/gpu/`.
   - Must it be specified under `docs/contract/` or `graph-contract`, or is a
     `fixtures/gpu/README.md` enough for a test-only format?
   - Are 2 × ≈ 345 KiB of goldens (estimated in the plan) acceptable in git?
4. **The oracle's independence.** The GPU arm loads the CPU's twiddles and kernel spectrum from
   the fixture instead of computing them. Which GPU defects can the G1b and G1c comparisons still
   catch, and which become invisible? Is that acceptable for a per-device tier?
5. **The bounds.** G1b and G1c hold each pass to `rmsRel` and `maxAbs` figures derived from the
   deposit quantum 2⁻¹¹ and the f32 spacing at 12,000 units. Are they derived soundly from those
   numbers, and does each have a negative control that would fail a wrong kernel?
6. **G1d's public surface.** A `ForceEngine` value in the SDK, the studio toggle, and the
   fallbacks. Rule now, or name what G1c must measure before G1d can be ruled on.

Score the four axes 1–5 (blast radius, reversibility, cost on failure, confidence) and name the
worst. Then give one verdict per slice, G1a and G1d: PROCEED, PROCEED-WITH-CONDITIONS (numbered
conditions, each checkable by a command or a test name), or BLOCK (what to resolve).

Write it to `docs/decisions/gpu-g1.md`:
- a title;
- `Status: G1a <verdict>, G1d <verdict>, 2026-10-06`;
- the six rulings as a table `| # | Question | Ruling | Evidence (path:line or command) |`;
- the scores;
- the conditions.

Nothing else.

You may run `git grep`, `sed -n` and read any file. Do not run builds, tests, benches or gates
other than the one below.

Paths you may touch: `docs/decisions/gpu-g1.md`. Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-gpu-g1-verdict scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS;
- `grep -E '^Status: G1a (PROCEED|PROCEED-WITH-CONDITIONS|BLOCK), G1d (PROCEED|PROCEED-WITH-CONDITIONS|BLOCK), 2026-10-06$' docs/decisions/gpu-g1.md`
  prints one line.

Return: the branch tip, the two verdicts, each condition, and every deviation from this brief.
