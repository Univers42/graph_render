# Job fix-scale-oracle (agent build, follow-up to fix-scale: U12)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-core-post.md` U12 and the
"decisions needed" items 1 and 5 of `docs/measurements/fix-scale.md`.

Why: the three scale rows (`scale.lod`, `scale.simplify`, `scale.adaptive`) claim to port
`SciGraphs/engine/scigraphs_engine/{lod.py,simplify.py,adaptive.py}` and no gate checks it. R4 (a
label budget of 0 read backwards) is the evidence that the defect class exists.

Do:
1. A differential in the house pattern, copied from the spectral one (read it first:
   `graph-cli emit-spectral-fixtures`, `harness/oracle-spectral.py`, `graph-cli oracle-spectral`,
   `crates/graph-cli/src/oracle_python/spectral.rs`). New: `graph-cli emit-scale-fixtures`,
   `harness/oracle-scale.py` (runs inside `ge-python-oracle`, imports the SciGraphs functions
   unchanged; never re-implements them), `graph-cli oracle-scale`. Fixtures are data emitted once
   and read by both arms. The outputs are masks and index lists, so the check is exact equality,
   not a tolerance; a float input (viewport, camera) is written as its exact f64 bits.
   Cover at least: lod label budget 0 / 1 / n, a viewport that culls half the nodes, simplify on a
   star, a path, two communities joined by one edge, a self-loop on a leaf; adaptive on the same.
   Where SciGraphs takes a parameter the motor does not expose, record it as a gap row in the
   report and leave it.
2. Run it on develop. Each mismatch is a finding: RED unit test in `crates/graph-core/src/scale/**`,
   GREEN fix, as fix-common's loop says. Check in particular fix-scale's item 5: a self-loop on a
   folded leaf or a contracted interior node stays drawn; an external edge between two collapsed
   members stays drawn (`s.edges[6] == 1` pins it today); a chain step's links naming a node a later
   community step hides. The reference decides each one; a test that pins the old behaviour and
   disagrees with the reference is changed, with the reason in the report.
3. The scale rows' oracle text in `crates/graph-cli/src/capabilities/` names the new differential,
   and `docs/measurements/phase09-lod.md:26-28` drops "a budget of zero still lets the most
   important visible node keep its label" (stale since R4: 0 means no limit, `lod.py:76-79`).

Paths: `harness/oracle-scale.py`, `crates/graph-cli/src/oracle_python/scale.rs` (+ its tests),
`crates/graph-cli/src/oracle_python.rs` / `cli.rs` / `main.rs` (additive only),
`crates/graph-cli/src/capabilities/**` (scale rows only), `crates/graph-core/src/scale/**`,
`fixtures/scale/**`, `docs/measurements/phase09-lod.md`, `docs/measurements/fix-scale-oracle.md`.

Done when: fix-common's done-when; the three commands of the differential pasted with their last
lines (exit 0 after the fixes); a negative control (one fixture's expected mask flipped by an env
knob or a `--break` flag) exits non-zero; `capabilities --check` and `codegen --check` exit 0.
