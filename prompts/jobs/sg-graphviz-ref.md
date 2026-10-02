# Job sg-graphviz-ref (agent build, SciGraphs conformance: what the Graphviz reference arm runs)

Read `prompts/jobs/sg-common.md` first. Needs `sg-graphviz-scale` landed. Rows: the nine Graphviz
rows (`YIFAN_HU`, `GRAPHVIZ_DOT`, `NEATO`, `FDP`, `SFDP`, `TWOPI`, `CIRCO`, `OSAGE`, `PATCHWORK`).
This job replaces the planned sg-osage-rows and sg-circo: their measured gaps are most likely
artefacts of this arm, not of the ports.

Why: the reference arm runs `<engine> -Tplain -Gstart=<seed>` on `gv_plain.write_dot`'s DOT
(`harness/scigraphs-conformance/sc_graphviz.py`). SciGraphs' real call
(`yifan_hu.py:278-306`) differs from that in four ways, and each one moves a row:
1. **Attributes.** It passes `_graphviz_default_attrs(engine, iterations, None, num_nodes=n)`
   (`yifan_hu.py:210-275`): sfdp `K=0.3, overlap=scale, start=seed, repulsiveforce=1.0,
   smoothing=spring, quadtree=normal`; fdp `K=0.3, overlap=scale, maxiter=50, start=seed`
   (Graphviz's own fdp default is 600 iterations); neato `start=seed`; osage `pack=True,
   packmode=array`; dot `rankdir=TB, splines=false`. `_optional_*` (`:170-179`) drops `0`,
   `0.0` and `"DEFAULT"`, so those are absent, not zero.
2. **The C RNG.** `_seed_graphviz_rng(seed)` (`:58-78`) calls `srand(seed)` and `srand48(seed)`
   in-process before the layout, because sfdp's coarsening (`gv_permutation`) draws `rand()`
   before `spring_electrical.c` seeds it. A fresh `sfdp` process draws that permutation from
   glibc's default seed 1 instead. FDP's tie-break jitter also draws `rand()`
   (`fdp/rng.rs:12-15`), which may be why finding 5 saw two FDP runs differ.
3. **Node boxes.** `write_dot` names nodes `n0..n{n-1}` with default labels, so Graphviz sizes
   `n10` and up from the label text (57.942pt, not 54; `oracle-graphviz.py:140-160`). Every
   motor port uses `NodeBox::DEFAULT` (54x36pt, `G_OSAGE_BOX`). That fits finding 7: osage and
   circo agree on the small fixtures and break only on lesmis (77 nodes). The sized writer in
   `oracle-graphviz.py` already pins boxes and holds osage to 6.31e-2pt over 1000 seeds
   (`docs/measurements/p13-gv1-osage.md`).
4. **Direction.** `dot` is directed in SciGraphs and undirected here (`G_GV_DIRECTED`).
What `scigraphs_utils` 0.2.0 itself does (its node names, so its label widths) cannot be read:
the wheel is not in the submodule (`SciGraphs/wheels/` holds only `.gitkeep`) and is not on PyPI
(`pip index versions scigraphs-utils`: no match). Its author maintains it (`SciGraphs/README.md:126`).
Keep `G_GV_UTILS` open for that, narrowed to what is still unknown.

Do:
1. Measure all nine rows. Then for lesmis, print the `-Tplain` node widths and confirm (or
   refute) point 3 before changing anything.
2. Attributes: import `_graphviz_default_attrs` from the SciGraphs submodule in the arm (do not
   copy it) and pass each attribute as `-G<k>=<v>`. If the Graphviz image has no numpy, compute
   the attribute set in `ge-python-oracle` and hand it over as a file.
3. RNG: reproduce `_seed_graphviz_rng` for the CLI. Use a small `LD_PRELOAD` shim built in the
   `ge-graphviz-oracle` Dockerfile, with a constructor that calls `srand(seed)` and
   `srand48(seed)` from an env var, or an in-process `libgvc` driver. Pick one and say why.
   Prove it: the shim changes sfdp's output against the unshimmed run on one fixture, and two
   shimmed FDP runs are byte-identical (if they are, FDP becomes pinnable; re-pin it and drop
   the "not reproducible" note).
4. Boxes: write every node with the sized writer and the motor's default box (0.75 x 0.5 in,
   `fixedsize=true`, `label=""`, `margin=0`); reuse `oracle-graphviz.py`'s writer, do not
   write a second one. Record in `G_GV_UTILS` that this assumes `scigraphs_utils`' labels fit
   the default box.
5. Directed `dot` (close `G_GV_DIRECTED`).
6. Motor arm: where a port already takes the knob, pass SciGraphs' value from `motor.rs` (fdp
   `maxiter=50` through a parameterised entry point beside `run`; the registered default stays
   Graphviz's). Where it does not (sfdp `smoothing=spring`), add a `Gap` naming it. Changing
   ports is other jobs' work (sg-gv-seeds, sg-sfdp-step).
7. Re-measure. This job may re-pin all nine Graphviz rows (it moves the reference). Report
   every row's disparity and max gap before and after in one table. For osage and circo, if
   lesmis still differs, name the first node whose position differs and why.

Paths: `harness/scigraphs-conformance/sc_graphviz.py`, `harness/gv_plain.py` (only if the sized
writer moves there for reuse), the graphviz-oracle Dockerfile (shim), the conformance `motor.rs`,
`rows.rs`, `gaps.rs`, `baseline/table.rs` (Graphviz rows), doc findings 5, 7, 8 and repairs 7, 11,
`docs/measurements/sg-graphviz-ref.md`. Nothing under `crates/graph-core` except an additive
fdp entry point.

Done when: the table above is in the measurement doc. Every Graphviz oracle gate the ports
already pass (`oracle-graphviz`, `oracle-circo`, `oracle-twopi`) is unchanged, because their
harness DOT stays as it is. The sg-common done-when holds.
