# Job p13-gv1 (agent build, native twopi port)

Why: the user decided the native Graphviz engines must match Graphviz's own output
(2026-09-30). The Graphviz oracle is built and deterministic (`docs/decisions/graphviz-oracle.md`),
so the first native port can be gated against it. twopi is first because it is the
simplest of the eight: a radial layout with a BFS tree and concentric rings, no
iteration, no force model.

Facts:

- The oracle is `ge-graphviz-oracle`, built from Graphviz 16.1.0 pinned by sha256 in
  `scripts/orch/fetch-refs.sh`. The harness `harness/oracle-graphviz.py` runs in the
  image, reads `target/spectral-fixtures/spectral.jsonl` (emitted by
  `graph-cli emit-spectral-fixtures --seeds 1000`), writes DOT, runs
  `twopi -Tplain -Gstart=1`, and records node positions in points keyed by node id with
  the graph bounding box. The output is `target/gv-twopi/graphviz-twopi.jsonl`.
- Determinism is proven: the oracle run twice over the same fixtures for twopi and circo
  produces byte-identical output (`cmp` is silent for both), and a 1e-6-point
  perturbation of one coordinate makes that `cmp` fail, so the check is not vacuous.
- **The oracle is deterministic without a seed.** `-Gstart` is INERT for twopi: the same
  fixture hashes identically with start=1, 7, 99 and with no `-Gstart` at all. twopi is a
  closed-form radial layout, not an iterative one. So a native twopi port has no seed to
  match, no initial-position dependence, and no chaos to blame for a gap: any difference
  from Graphviz is a real algorithmic difference, not drift. Do not gate on seed
  stability — there is none to gate.
- The differential metric is the largest absolute coordinate difference in points, after
  both arms are rescaled to the same bounding box. The ceiling is the next power of ten
  above the worst measured gap over 1000 seeds, measured and recorded in
  `docs/measurements/`, never guessed. The comparison is against the Graphviz oracle
  output, not against a second native run.
- twopi is closed form, so **both arms are also compared byte-for-byte on the
  analytically-determined small cases** (one node, two nodes, a path, a star, a cycle),
  where the layout has a closed answer and a tolerance is weaker than the truth. A layout
  that is within 1e-6 of Graphviz on a star but not on a 3-path has a bug a ceiling would
  round away.
- The registry is `crates/graph-core/src/registry.rs:167` (`LAYOUTS`, length is a
  compile-time literal). `Metadata` is `registry.rs:32-64` (tier, stage, nodes, edges,
  oracle, complexity, scale_ceiling, degradation, ponytail) and `Capability` is
  `registry.rs:55-64`. Append the new entry at the end of the array — do not insert.
- Every entry must satisfy `crates/graph-core/src/registry/tests.rs:7-21` (unique id,
  `layout.` prefix, `stage == "layout"`, `scale_ceiling > 0`, and
  oracle/complexity/degradation/ponytail all non-empty) and `registry/tests.rs:24+` (it
  emits the node and edge kinds it declares, at its default parameters).
- A layout joins the hash gate merely by being in `LAYOUTS`
  (`crates/graph-cli/src/hashgate/stages.rs:47-49`), so the "hashgate entry" is the
  registry entry plus one literal to update: the exact per-stage record pinned by
  `crates/graph-cli/src/hashgate/tests/report.rs:64`.
- Ledger routing is `crates/graph-cli/src/capabilities/registry.rs:249-282`. An id in none
  of the lists falls through to `roundtrip` with `Status::Gated`
  (`registry.rs:250-260`), and `capabilities --check` refuses a `gated` row with no
  recorded run behind it. A force row instead gets an explicit entry in
  `crates/graph-cli/src/capabilities/registry/unproven.rs:29-43` with
  `Status::Implemented`, which is the honest status for a layout whose differential has
  not been run to the full seed count.
- The differential shape to copy: `harness/oracle-graphviz.py` plus
  `graph-cli emit-spectral-fixtures --seeds 1000` plus a `graph-cli oracle-graphviz`
  subcommand that checks and records, in the three-arm shape of
  `harness/oracle-closed-form.py` and `graph-cli emit-closed-form-fixtures`. The
  iterative precedent is `harness/oracle-fa2.py`.
- Determinism rules that bind the kernel: CLAUDE.md "Determinism" and `prompt.md` §6
  D1-D10 — libm transcendentals only, no `mul_add`/`powi`/relaxed-simd, fixed-order
  reductions, no wall-clock, no randomness except the seeded generators, every kernel in
  gather form.
- Gate rows are `name|expect|cmd` with `expect` in {`0`, `nonzero`} and `#` comments
  allowed (`scripts/orch/gate.sh:2-4`). The negative controls are env knobs, not rows in
  a per-layout list: `Knob::ALL` is at `crates/graph-cli/src/hashgate/knob.rs:105` with
  the env names at `knob.rs:122-132`.

Do:

1. `layout.twopi` — Graphviz `twopi` at its default parameters. New module
   `crates/graph-core/src/layout/radial/twopi.rs` (child modules plus `tests/` if it
   passes 300 lines; never compress). A full `Metadata` entry with all nine fields
   written out concretely: the `oracle` string naming Graphviz 16.1.0 `twopi -Tplain`
   at `-Gstart=1`, the `complexity` string, a MEASURED `scale_ceiling` with the `bench`
   command that measured it, a `degradation` string in the shape of
   `crates/graph-core/src/registry/closed_form.rs:18`, and a `Ponytail:` line naming the
   failing input, the direction of failure, and the escape hatch. The kernel is
   gather-form and the reductions run in a fixed order.
2. The oracle differential `harness/oracle-twopi.py` plus a `graph-cli emit-twopi-fixtures`
   and `graph-cli oracle-twopi` subcommand, in the three-arm shape. The metric is the
   largest absolute coordinate difference in points, after both arms are rescaled to the
   same bounding box. The ceiling is the next power of ten above the worst measured gap
   over 1000 seeds. The comparison is against the Graphviz oracle output
   (`target/gv-twopi/graphviz-twopi.jsonl`), not against a second native run.
3. Hashgate: the id joins by being appended to `LAYOUTS`; update the pinned record at
   `crates/graph-cli/src/hashgate/tests/report.rs:64`; add the `Status::Implemented`
   routing entry in `crates/graph-cli/src/capabilities/registry/unproven.rs:29-43` for
   `layout.twopi`. Do not let the row print `gated` on a hash alone.
4. `scripts/orch/rows/p13-gv1.rows` (new) with the fmt / clippy / test / wasm32 / hashgate
   rows copied from `scripts/orch/rows/quick.rows`, plus one `negctl-<knob>` row per
   relevant knob with `expect nonzero` — or, if a knob must be added for the new stage,
   add it to `Knob::ALL` (`hashgate/knob.rs:105`) with its env name at `knob.rs:122-132`
   and say why in the return block.
5. `docs/measurements/p13-gv1.md` (new) with the measured ceiling and the differential
   results, in the shape of `docs/measurements/closed-form-oracle.md`, each row carrying
   its command.
6. Flip the twopi row in `docs/measurements/scigraphs-coverage.md` off `planned: p13-gv1`
   and off `missing`, and update its counts block. Nothing else in that file changes.

Paths you may touch: `crates/graph-core/src/layout/radial/twopi.rs` and the child modules
and `tests/` it declares, `crates/graph-core/src/registry.rs`,
`crates/graph-core/src/registry/radial.rs`, `crates/graph-cli/src/capabilities/registry.rs`,
`crates/graph-cli/src/capabilities/registry/unproven.rs`,
`crates/graph-cli/src/hashgate/knob.rs` and its tests,
`crates/graph-cli/src/hashgate/tests/report.rs`, `crates/graph-cli/src/command.rs`
and the new oracle subcommand modules, `crates/graph-cli/src/snapshot_cmd/hand_oracles.rs`
and its `tests/` if a hand oracle is needed, `harness/oracle-twopi.py`,
`scripts/orch/rows/p13-gv1.rows` (new), `docs/measurements/p13-gv1.md` (new),
`docs/measurements/scigraphs-coverage.md`. Nothing else.

Done when: the id is in `LAYOUTS` with all nine `Metadata` fields written and every one of
them non-empty, so `crates/graph-core/src/registry/tests.rs:7-21` passes unmodified; the
`hashgate/tests/report.rs:64` literal carries the new id; `capabilities --check` shows the
row with an honest status and no `gated` claim resting on a hash alone; the oracle
differential is run at the full seed count with its worst gap and its ceiling written into
`docs/measurements/p13-gv1.md`; every new `negctl-` row in `scripts/orch/rows/p13-gv1.rows`
fails when its knob is set; `crates/graph-wasm/src/exports/build.rs` builds with the new
layout count; the twopi row in `docs/measurements/scigraphs-coverage.md` is updated and
its counts block still sums to the number of SciGraphs names; and the return block lists
every command run with its real exit code, the rung of the minimalism ladder each new
module sits on, and every file you created.
