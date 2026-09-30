# Job tier-closed-form (agent build, three layout ids)

Why: `prompts/RESUME.md` queues "Every layout gets the Phase 11 thread tier and a bench-driven
optimization pass. Order: easiest first." After `prompts/jobs/tier-settle.md`, these are the next
three: the closed-form point layouts whose hot loop is already a pure per-node gather with no
reduction crossing elements. `layout.grid`, `layout.circular.ring` and `layout.spiral` all end in
the **same** `coords::rescale` (`crates/graph-core/src/layout/coords.rs:15`), so one serial merge
loop serves all three and one new knob covers all three.

Facts:

- **`layout.grid`** — `layout/grid.rs:54` `Stage::run` → `:75` `positions`, hot loop `:84-85`
  (`(0..n).map(|i| offset(i % cols, cols))` and the `/ cols` twin). No edge work, no reduction, no
  `f64`: the whole body is one `f32` product per coordinate. `cols = ceil(sqrt(n))` at `:65-70`.
  Existing stage-level knob `GM_MUTATE_GRID_SPACING` (`hashgate/knob.rs:42-43`, variant
  `GridSpacing`) changes `spacing` and so every cell centre.
- **`layout.circular.ring`** — `layout/circular/ring.rs:21` `run`, hot loop `:27-28`:
  `(0..count).map(|k| k as f64 * step * TAU)` then one `libm::cos`/`libm::sin` per node
  (`:28`). Transcendentals: libm only, native and wasm32 bit-identical (D1–D2, and
  `graph-cli determinism-probe` is the check).
- **`layout.spiral`** — `layout/spiral.rs:26` `run` → `:33` `run_with`. **The default is the
  archimedean branch**: `run` passes `equidistant = false` (`spiral.rs:27`), so the hot loop is
  `archimedean_points` `:67-75`, a per-node gather. The **equidistant** branch (`:52-64`) carries
  `theta += CHORD / radius` across iterations at `:60` and is **sequential by nature**. Thread the
  archimedean branch; declare the equidistant branch serial in one line and never route it
  through a runner.
- **The shared reduction** — `coords.rs:15` `rescale`: the centroid is
  `x.iter().sum::<f64>() / count` and `y.iter().sum::<f64>() / count` at `:20-21`, then
  `limit = limit.max(px.abs()).max(py.abs())` at `:26`. The module doc already states the rule
  (`coords.rs:14`): "The centroid is summed in index order, so the result is fixed on every
  target." **Keep that sum a serial loop.** The precedent for exactly this shape is
  `barnes_hut/charge.rs:24-27`: "The merge is a straight loop over `deltas` in ascending node
  index, which is the one place the division could go wrong and the reason it is written as a loop
  rather than left to the runner." Copy that reasoning verbatim into the new merge.
- **The runner** — `Threads`, `crates/graph-cli/src/exec_native.rs:34`. It lives in graph-**cli**;
  graph-core takes `runner: &impl crate::exec::Runner` from the caller, the shape
  `BarnesHut::run_with` has (`barnes_hut.rs:151`). Each layout gains one `StepRange` impl
  (`exec/partition.rs:41`) whose `Out` is that layout's own per-node tuple — `(f32, f32)` for
  grid, `(f64, f64)` for ring and spiral. No `unsafe`, no shared borrow: `Out: Copy + Default +
  Send` (`:43`) plus disjoint ranges is the whole contract.
- **`rescale` mutates in place after the gather**, so the runner fills a `Vec` of pairs, the merge
  loop runs once over it, and the values are subtracted and divided in a second serial pass
  (`coords.rs:22-31`). That pass is already a plain sequential loop over `x.iter_mut()` and must
  stay one — it is a `f64` max fold plus a divide, and splitting it would split nothing worth
  splitting.

Do:

1. Per layout: a `run_with(topology, [params], runner, workers)` whose serial entry point calls it
   with `&Serial, 1`, exactly as `barnes_hut.rs:116` does. One `StepRange` impl each. Extract the
   shared merge loop **once** — it is the same function for all three, and three copies of a
   float-sum merge is three things to keep in agreement.
2. Refactor `coords::rescale` into the two halves (serial merge, serial rescale) so the runner can
   fill the first without touching the second. Keep `rescale`'s existing signature working for the
   layouts that do not thread yet (`layout.circular.radial`, `layout.bipartite`, `layout.random`),
   or thread those callers in the same commit if it is the smaller diff — either is fine, pick the
   smaller one and say which you picked.
3. Extend the bench tier sweep to route by layout. `bench.rs:146-148` returns `tiers::entry(plan)`
   before any layout resolution and `bench/tiers/sweep.rs:113-119` always calls
   `BarnesHut::run_under`, so `bench --tiers` times one layout no matter what `--layout` says.
   Without this step the acceptance criterion cannot be run. Keep `BarnesHut` the default so every
   row in `docs/measurements/phase11-threads.md` still reproduces, and keep
   `refuse_unproved_widths` (`bench/tiers.rs:219-229`) refusing any `--workers` outside
   `hashgate::WORKER_COUNTS = [1,2,3,4,7]` (`hashgate/tier.rs:45`).
4. Add all three ids to the hashgate's threaded match: `crates/graph-cli/src/hashgate.rs:169` is
   `let bytes = if id == BarnesHut::ID { ... } else { bytes }`, and every other stage reuses the
   scalar run's bytes (`:181-183`). Until an id is in that match, every `--tiers all` arm hashes
   the same bytes for it and "equal" is vacuous. One match arm covering the three is fine and
   preferred over three arms.

Knob that must turn it red: **a new one, and one only.** `GM_MUTATE_SPLIT_SUM` is
Barnes-Hut's — it carries a `barnes_hut::Split` on `Setting` and reaches a stage only through
`BarnesHut::run_under(..., setting.split_sum)` (`hashgate.rs:175`), so it is **inert here**. Add
its sibling in the same shape: a knob whose perturbation makes `rescale`'s centroid merge read a
**neighbouring node's** term instead of its own — "the shape a wrong partition of the outputs would
take" (`knob.rs:85-86`) — so the threaded arms must diverge from the scalar one. Declare it in
`Knob::ALL` (`knob.rs:105-116`), give it an `env()` name (`:120`) and a `record()` name
(`:137`), and carry the setting on `Setting` like every other knob (`knob.rs:180`) so a knob
cannot change behaviour without being declared.

Two properties of the new knob you must not skip: it must reach the **merge**, not the gather (a
knob that perturbs the layout's own arithmetic only proves the stage is hashed, not that the
threaded arm recomputed it); and it must be a **stage-level** control that turns *only* the
`rescale`-using stages red, so a red run names which stage diverged.
`GM_MUTATE_GRID_SPACING` already exists as the grid stage-level knob and is a **pass** control, not
this one — keep both, and say in the report which question each answers.

Hashgate arm that must stay equal: `--tiers all`, which adds `native scalar` plus one
`native threads N` arm per `WORKER_COUNTS` (`hashgate/tier.rs:82-91`) — 10 arms, compared
line-for-line against arm 0 (`compare.rs:54-56`), printed as `{ways}-way equal`
(`hashgate.rs:227-231`). These three layouts are cheap; if a width is faster than serial and
different, that is a failure of the merge, not a footnote.

Bench commands (only after step 3):

```sh
scripts/orch/gr cargo run --release -p graph-cli -- bench \
  --layout layout.grid --layout layout.circular.ring --layout layout.spiral \
  --n 220,10000 --tiers scalar,threads --workers 2,4,7 --repeat 5 \
  --out docs/measurements/tier-closed-form.md
```

Deviations from the template in `prompts/jobs/tiers-audit.md:24`: `--tiers` needs a value;
`--workers 8` is refused by `refuse_unproved_widths` (`bench/tiers.rs:219`); `1` is omitted because
the sweep's scalar arm already is the one-worker run (`bench/tiers.rs:46-50`). Do not "fix" that
refusal. Also note `--n 220` is the size at which the one crossover measurement in the tree says
threads **lose** (`docs/measurements/phase11-threads.md`), so expect a `speedup` column below 1.0
and report it rather than dropping the size.

Gate:

```
hashgate --seeds 8 --tiers all                -> 0; grid, circular.ring, spiral: 10-way equal on 8/8
GM_MUTATE_<new rescale-merge knob>=1 hashgate --seeds 8 --tiers all -> non-zero, exactly those stages diverge
```

Done when: all three layouts have a `run_with` and a `StepRange` kernel, the bench sweep times them
per layout, the hashgate recomputes each under every width, all arms are byte-equal at every width,
the new merge knob turns the gate red on those stages and nowhere else, `workers = 1` produces the
exact bytes today's gate lines hold, and `docs/measurements/tier-closed-form.md` carries the
measured table plus the `Ponytail:` note. UNKNOWN = FAIL: no measurement means no speed-up claim,
so a row with an estimated speed-up is not done.

Paths you may touch: `crates/graph-core/src/layout/grid.rs`,
`crates/graph-core/src/layout/circular/ring.rs`, `crates/graph-core/src/layout/spiral.rs`,
`crates/graph-core/src/layout/coords.rs`, `crates/graph-cli/src/bench/tiers/`,
`crates/graph-cli/src/hashgate.rs`, `crates/graph-cli/src/hashgate/knob.rs`,
`docs/measurements/tier-closed-form.md`, and the matching tests. Nothing else.
