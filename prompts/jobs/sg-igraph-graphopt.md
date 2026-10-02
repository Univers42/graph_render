# Job sg-igraph-graphopt (agent build, SciGraphs conformance: IGRAPH_GRAPHOPT)

Read `prompts/jobs/sg-common.md` first. Needs `sg-mt19937` (the `Mt19937` type) and
`sg-igraph-dims` (`_igraph_fit_positions` in the conformance arm, the corrected
`G_IGRAPH_SEED`) landed. Row: `IGRAPH_GRAPHOPT`. Target: f32 bitwise, or a named gap.

Why this row can go bitwise when DH cannot. Graphopt draws random numbers only for its start
(`docs/layouts/layout.force.graphopt.md`, "Randomness"); the iteration is deterministic and its
summation order is fixed by the spec. python-igraph installs Python's `random` as igraph's RNG
(see `sg-igraph-dims`), and SciGraphs reseeds `random` before every layout
(`common.py:53-62`, called at `dispatcher.py:22`). So the reference start is
`random.seed(981798123)` followed by `random.random()` draws: Python's generator, not igraph's,
which the conformance arm may reproduce (the motor's own layouts keep Mulberry32; see f below).

The defects, against the spec (`docs/layouts/layout.force.graphopt.md`, amended in this commit by
a spec author; the implementer never opens igraph's C, layouts-igraph.md rule 1):
a. Start domain. `graphopt.rs:69` reuses FR's `start_positions` (side sqrt(n),
   `fruchterman_reingold.rs:95-104`); the spec's start is each coordinate uniform on [-1, 1],
   x then y per node, drawn as `u * 2.0 + (-1.0)` with `u = random.random()`. The module doc at
   `graphopt.rs:60` says so itself ("not igraph's random layout").
b. Coulomb association. `graphopt.rs:110` computes `COULOMB * charge * charge / (d * d)`; the
   spec now says `C * ((q * q) / (d * d))`. Same value, different rounding.
c. Python's `random.seed(int)` is MT19937 `init_by_array([s])` (32-bit key words of |s|), not
   numpy's `init_genrand(s)`; `random.random()` is the same 53-bit formula as numpy's `rand`.
   Test vector (CPython 3, measured 2026-10-02, `random.seed(981798123)`, four draws):
   0x1.07f8cb06dcdf3p-1, 0x1.ed3a77cefffbep-2, 0x1.9a0acc6fa6591p-1, 0x1.19ed4aef77f6ep-1;
   so lesmis node 0 starts at (0x1.fe32c1b737cc0p-6, -0x1.2c58831000420p-5) and node 1 at
   (0x1.341598df4cb22p-1, 0x1.9ed4aef77f6e0p-4). Assert with `f64::from_bits`.
d. Edge order. The spring pass visits edges by igraph edge id, which is the order of
   `add_edges(list(G.edges()))` (`common.py:355-359`); networkx's `G.edges()` walks nodes in
   insertion order and each node's neighbours in insertion order, emitting an edge the first
   time it is seen, with the first endpoint as FROM. Prove the motor visits the fixture edges in
   that order with that orientation (4-node test with an edge listed as (c, a)); if not, the
   conformance arm hands graphopt the reordered edge list. Orientation decides which endpoint
   gets `+=` and which `-=`, which matters for rounding, not for value.
e. The p12 ceiling: `oracle-igraph.py` hands igraph our start (`seed=`), yet graphopt's recorded
   worst is 15.39 (`docs/measurements/p12-igraph-ceilings.md:54-75`). From the same start and
   the same arithmetic the ratio should be 1. Once a and b are fixed, re-run `oracle-igraph` and
   paste graphopt's worst; if it is not ~1, a difference remains and this job finds it.

Do:
1. Measure the row; paste its metrics line and both arms' shape ratio on lesmis. Show that
   nothing draws from `random` between `_reset_layout_rng` and `layout_graphopt` (read the path,
   paste the lines), and that the reference row is identical over two runs.
2. RED: `Mt19937::from_key(&[u32])` with the vector above; a graphopt test from an explicit
   two-node start pinning one step's displacement bit for bit (b); the edge-order test (d).
3. GREEN: graph-core graphopt gains `run_from(topology, params, start: &[[f64; 2]])`. Fix b in
   the shared kernel only if the registered default's hashgate record may move; it may (it is a
   port defect against the spec), say so in the doc and re-run hashgate. The registered start
   (a) stays Mulberry32 but in the spec's [-1, 1] domain: that is a registered-default change
   for a real defect; re-run `oracle-igraph` and paste the before/after table.
4. The conformance arm: `layout.force.graphopt` dispatches to an override that draws the start
   from `Mt19937::from_key(&[LAYOUT_SEED])` (per node: x then y, `u * 2.0 + (-1.0)`), calls
   `run_from`, then the fit from `sg-igraph-dims`. If the fit's mean is not numpy's summation
   order (row by row for `axis=0` on a C-order (n, 3) array; verify), bitwise stops at the fit:
   say so and fix it there, in sg-igraph-dims' function, with its own test.
5. Re-pin the row; close `G_IGRAPH_SEED` for this row only if f32 reaches N/N.
f. Deviation to report: layouts-igraph.md rule 4 says the start is our seeded generator only.
   The motor still is; the conformance arm (graph-cli, a test instrument) reproduces Python's
   `random`, which is not igraph's RNG. Add one paragraph to the decision saying this, with the
   reason, and list it under "deviations" in the report.

Paths: `crates/graph-core/src/rng*` (`from_key` only), `layout/force/graphopt*.rs` (split if a
file passes 300 lines), the conformance `motor.rs` and its tests, `rows.rs`, `gaps.rs`,
`baseline/table/igraph.rs` (IGRAPH_GRAPHOPT), the doc's row, `docs/decisions/layouts-igraph.md`
(one paragraph), `docs/measurements/sg-igraph-graphopt.md`.

Done when: sg-common's done-when, the RED tests green, the row at f32 N/N or the first differing
coordinate named with the operation that differs, and the `oracle-igraph` graphopt worst before
and after pasted.
