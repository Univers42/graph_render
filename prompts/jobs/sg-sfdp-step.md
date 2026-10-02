# Job sg-sfdp-step (agent build, SciGraphs conformance: GRAPHVIZ_SFDP, YIFAN_HU)

Read `prompts/jobs/sg-common.md` first. Needs `sg-graphviz-ref` landed: its reference runs sfdp
with SciGraphs' attributes (`K=0.3, overlap=scale, repulsiveforce=1.0, smoothing=spring,
quadtree=normal, start=<seed>`) and a seeded C RNG (`srand(seed)`, `srand48(seed)`). Rows:
`GRAPHVIZ_SFDP` (0.848) and `YIFAN_HU` (0.829).

Why, three causes with file:line:
- Coarsening order. The motor matches in dense index order; Graphviz draws `gv_permutation`
  from glibc `rand()` (`Multilevel.c:104`), and the solver re-`srand`s with `random_seed`
  on every call (`spring_electrical.c:283`, `:423`, `:558`, `:719`). graph-core says so (`sfdp.rs:38-61`,
  escape hatch: "replace `multilevel::coarsen` and re-`srand` per level"). graph-core already
  has glibc `rand()` (`sfdp/start.rs` `Glibc`; if `sg-gv-seeds` unified it with
  `fdp/rng.rs` `GlibcRand`, use the shared one).
- `smoothing=spring`: SciGraphs sets it (`yifan_hu.py:236`) except on a graph with an
  isolated node, where it falls back to `none` (`yifan_hu.py:286-291`); the reference arm must
  apply the same rule per fixture. Graphviz parses it at `sfdpinit.c:151` and runs it after
  the multilevel solve (`post_process.c:1002`). The port has no smoothing stage.
- YIFAN_HU maps to the wrong motor layout. SciGraphs' YIFAN_HU is sfdp in '2Z' mode: 2-D sfdp,
  then `positions[:,2] = _generate_z_component(G, n, 'SPECTRAL') * 0.3 * scale`
  (`yifan_hu.py:330-333`). The SPECTRAL z (`networkx_layouts.py:304-335`) is the last spectral
  component coordinate per connected component, divided by its peak, then `_center_z` (mean 0,
  largest deviation 0.5). `layout.force.yifan_hu` is NOT sfdp (`yifan_hu.rs:1-9`: Barnes-Hut).

Do:
1. Measure both rows. Then diff ONE sfdp level against Graphviz 16.1.0
   (`lib/sfdpgen/Multilevel.c`, `spring_electrical.c`, `post_process.c` under
   `$GM_SCRATCH/refs`; absent = stop): the coarsening matching of lesmis level 0 (dump the
   Graphviz matching with a debug build or `-v` output, say which), then one force step.
   Name the first divergent quantity.
2. RED: a graph-core test pinning Graphviz's level-0 matching on a 6-node path at seed
   981798123. GREEN: `multilevel::coarsen` takes the permutation from the glibc generator,
   re-seeded per level as `Multilevel.c` does, reached only through `sfdp::run_seeded`.
   The registered `run` stays byte-identical (`hashgate --seeds 8`) unless `oracle-graphviz`
   sfdp improves; then justify the change with its before/after numbers in the report.
3. `smoothing=spring`: port the spring smoother if it is under 300 lines with its own test
   against a Graphviz output; otherwise record it as a new `Gap` with its file:line and stop
   there for this cause.
4. YIFAN_HU in the conformance arm only (no graph-core API change beyond one additive
   function if needed): motor = `sfdp::run_seeded` 2-D plus a z from graph-core's spectral
   kernel, same per-component peak division and `_center_z`, times `0.3 * scale`. The
   reference side already runs SciGraphs' own `_generate_z_component`; do not copy it.
   Library first: the per-component normalisation and `_center_z` are one function each.
5. Re-pin the two rows. If SFDP stops at `tolerance`, name the first level and step whose
   positions differ, with the magnitude.

Paths: `layout/graphviz/sfdp.rs`, `layout/graphviz/sfdp/*`, the conformance `motor.rs`,
`rows.rs`, `gaps.rs`, `baseline/table.rs` (two rows), `docs/measurements/sg-sfdp-step.md`.

Done when: before/after for both rows is pasted, `oracle-graphviz` sfdp is unchanged or
better at its own `-Gstart`, and the sg-common done-when holds.
