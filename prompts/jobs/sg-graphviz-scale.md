# Job sg-graphviz-scale (agent build, SciGraphs conformance: the eight Graphviz rows)

Read `prompts/jobs/sg-common.md` first. Rows: `GRAPHVIZ_TWOPI` and `GRAPHVIZ_PATCHWORK` (cause
`convention`, same shape to 2e-10 and 4e-10, max gap 303 and 139), and with them every row whose
reference is `-Tplain`: `YIFAN_HU`, `GRAPHVIZ_NEATO`, `GRAPHVIZ_FDP`, `GRAPHVIZ_SFDP`,
`GRAPHVIZ_CIRCO`, `GRAPHVIZ_OSAGE` (`GRAPHVIZ_DOT` has no motor layout; leave it).

Why: SciGraphs never returns Graphviz's raw points. `_scigraphs_utils_graphviz_layout`
(`yifan_hu.py:278-337`) takes the engine's positions and, at `yifan_hu.py:318-325`:
```python
raw = raw - raw.mean(axis=0)
raw_range = raw.max(axis=0) - raw.min(axis=0)
extent = float(raw_range[:dims].max())
raw = raw / (extent if extent > 0 else 1.0)
positions[:, :dims] = raw[:, :dims]; positions *= scale      # z stays 0 for 2D
```
The gate's reference arm takes the engine's `-Tplain` points raw (`G_GV_UTILS`, because
`scigraphs_utils` is in no oracle image), and the motor arm takes each Graphviz port's output in
its own units. So the matrix compares two unit systems for all eight rows.

The constraint (user decision 2026-09-30): the Graphviz ports in graph-core match Graphviz output
and are gated against Graphviz 16.1.0 (their registry `oracle`). This job changes **nothing under
`crates/graph-core`**. The conformance doc's repair 2 proposes a `SCALE` const inside `twopi.rs` and
`squarify.rs`; that is wrong under this decision: rewrite it.

Do:
1. Measure: for one fixture, print the ratio between the motor's and `-Tplain`'s spans for
   twopi and patchwork, and say which unit each side is in (inches vs points: `-Tplain` writes
   inches) with the line that says so.
2. RED: a graph-cli unit test of a `scigraphs_graphviz_post(points, dims, scale)` function
   (name it for what it does) on a 3-node input whose expected output you compute in numpy
   inside `ge-python-oracle` and paste as hex.
3. GREEN: apply that one function to **both** arms of every Graphviz row, so each arm emits what
   SciGraphs would: the reference arm in `harness/` (Python, the four lines above verbatim) and the
   motor arm in the conformance `motor.rs` (Rust). Caveat to write next to the Rust mean: numpy's
   `mean` sums pairwise in blocks of 8/128, so for n > 8 the Rust sum must follow the same order or
   the row stops at `tolerance`; say which one you did.
4. Re-pin the eight Graphviz rows that moved (this job owns them; it lands before
   sg-osage-rows, sg-sfdp-step, sg-circo and sg-neato-start). Narrow `G_GV_UTILS`'s note to what
   is still true (the engine binary instead of `scigraphs_utils`), or close it if nothing is.

Paths: the conformance module (`motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs` Graphviz rows),
the reference harness file that writes the Graphviz rows, doc rows 22-30 and repair 2,
`docs/measurements/sg-graphviz-scale.md`.

Done when: TWOPI and PATCHWORK reach f32 1020/1020 or `tolerance` with the reason measured; the
other six rows' disparity is unchanged to 1e-12 (the transform is a similarity, so Procrustes must
not move: if it does, the transform is wrong); the sg-common done-when holds.
