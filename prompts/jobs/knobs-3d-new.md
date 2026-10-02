# Job knobs-3d-new (agent build: hash-gate controls for the two newest 3D layouts)

Read `prompts/jobs/fix-common.md` first. Released only after sg-spiral3d and sg-bipartite3d are
both on develop (check with `git merge-base --is-ancestor`; if either is not, stop and report).

Why: `layout.basic3d.spiral` and `layout.bipartite_3d` are hashed by `hashgate` but have no per-stage
negative control (`docs/measurements/sg-bipartite3d.md`, "No hash-gate knob"). The other natively 3D
layouts each have one in `hashgate::knobs::THREE_D_LAYOUT_STAGES` (`crates/graph-cli/src/hashgate/knobs.rs`),
and nothing fails when a new layout skips it.

Do:
1. Append both stages to `THREE_D_LAYOUT_STAGES` with the same shape as the others (re-drawn model,
   one more node), through `hashgate/knob/three_d.rs`, `knob/arms.rs`, `knobs.rs` and the twin list
   `crates/graph-cli/tests/common/mod.rs` `KNOBS` (count and doc), plus
   `hashgate/tests/knob/{three_d,table}.rs`. Additive only.
2. RED then GREEN: a unit test that lists every `registry::LAYOUTS` id with no per-stage knob, and
   compares it with an explicit allow list in the test, each entry carrying a one-line reason (measure
   the list; do not guess it). Before step 1 it names the two ids above; after step 1 it passes.
3. Each new knob turns `hashgate --seeds 8` red: paste both runs' exit codes and last lines.

Paths: the files named in step 1, the new test, `docs/measurements/knobs-3d-new.md`.

Done when: fix-common's done-when; both knob runs exit non-zero; `hashgate --seeds 8` exits 0
without them.
