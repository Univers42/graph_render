# Job open-core-slot (agent build; the 1M open path, option (a) of open-core)

Why: open-core landed `StringArena::with_capacity` and option (b) and took `index_model` 1809 → 1613 ms
and `check_ids` 892 → 584 ms at 1M nodes (−18.7% median, target −20%; `docs/measurements/open-core.md`).
Its report: the two `IndexMap<Interned>` probe rows (~540 ms) did not move, and option (a) is where the
next 20% is. Goal: 1M nodes open fast in the studio.

Do: option (a) from `prompts/jobs/open-core.md` step 3: in `crates/graph-core/src/index.rs`,
`admit_edge` resolves each endpoint once, via a `Vec<u32>` from arena slot to dense node index built while
nodes are admitted (a local of `index_model`, never a new `Topology` field), instead of a `strings.find`
followed by a second `IndexSet<Interned>` lookup (`index/view.rs:33-42`). Keep it only if it wins by more
than 3% and every hash is unchanged.

Measure exactly as open-core did (its step 4: `scripts/studio.sh build`, then
`deploy/perf/open.py 1000000 webgl2`, 3 runs before and 3 after, interleaved) and append a section to
`docs/measurements/open-core.md`.

Determinism and bounds: as `prompts/jobs/open-core.md` ("Determinism", "Out of bounds"). Any change to
a snapshot hash or to a capabilities row means the change is wrong: revert it.

Done when: quick.rows green including wasm32-core, hashgate-8 and its negctl, and the measurement
committed. If the win is under 3%, revert the code and commit only the measurement: the numbers are the result.
