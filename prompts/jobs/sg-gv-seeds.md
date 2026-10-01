# Job sg-gv-seeds (agent build, SciGraphs conformance: GRAPHVIZ_NEATO, GRAPHVIZ_FDP)

Read `prompts/jobs/sg-common.md` first. Needs `sg-graphviz-ref` landed (the reference is then
run with SciGraphs' attributes and a seeded C RNG). Rows: `GRAPHVIZ_NEATO` (0.424, cause `rng`)
and `GRAPHVIZ_FDP` (0.661, `rng`).

Why: SciGraphs passes `start=<seed>` to both engines (`yifan_hu.py:231`, `:246`). The ports
cannot take that seed:
- neato's `run_with` takes only `epsilon` (`neato.rs:118`, `G_NEATO_START`). Its start is
  therefore the port's default, not the engine's `start=<seed>` placement.
- fdp seeds `Rand48::new(START_SEED)` inside graph-core (`fdp/model.rs:103-107`, `G_FDP_SEED`).
graph-core already reproduces `srand48`/`drand48` exactly (`fdp/rng.rs`, `Rand48`) and glibc
`rand()` (`fdp/rng.rs` `GlibcRand`, `sfdp/start.rs` `Glibc`).

Do:
1. Measure both rows. Read where Graphviz 16.1.0 turns `start=<int>` into neato's initial
   positions (`neatoinit.c`, `setSeed`, and the init path `mode=major` takes by default). Write
   the draw order down with file:line: which generator, how many draws per node, which node
   order, and the scaling of the draws. Read the sources under `$GM_SCRATCH/refs`; if they are
   absent, stop (a missing reference is a stop).
2. Library first: if neato's start draws `drand48`, move `Rand48` out of `fdp/` into a shared
   `layout/graphviz/rng.rs` (both ports use it). Do the same for `GlibcRand` vs `sfdp`'s `Glibc`
   if they are one generator twice: one implementation, each test kept.
3. RED: graph-core tests that pin the engine's seeded start for a 4-node path at seed
   981798123 (take it from `neato -Gstart=981798123 -Gmaxiter=0 -Tplain`, or the nearest
   engine output that exposes the start; say which). Do the same for fdp at that seed.
4. GREEN: `neato::run_seeded(topology, seed)` and `fdp::run_seeded(topology, seed)`, with the
   registered `run` unchanged and byte-identical (`hashgate --seeds 8` proves it). `motor.rs`
   calls both with `LAYOUT_SEED`; close `G_NEATO_START` and `G_FDP_SEED`.
5. Re-measure and re-pin the two rows. NEATO's tier is `bitwise` in the matrix: if it stops at
   `tolerance`, name the first iteration whose stress differs.

Paths: `layout/graphviz/neato*`, `layout/graphviz/fdp*`, a shared `layout/graphviz/rng.rs` if
step 2 applies, the conformance `motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs` (the two
rows), `docs/measurements/sg-gv-seeds.md`.

Done when: both rows' before/after numbers are pasted, `oracle-graphviz` for neato and fdp stays
within its ceiling at its own `-Gstart`, and the sg-common done-when holds.
