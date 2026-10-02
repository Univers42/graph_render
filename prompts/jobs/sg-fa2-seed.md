# Job sg-fa2-seed (agent build, SciGraphs conformance: FORCEATLAS2)

Read `prompts/jobs/sg-common.md` first. Needs `sg-mt19937` landed. Row: `FORCEATLAS2` (cause
`rng`, disparity 0.241, f32 0/1020).

Step 0, a diagnosis that decides the rest. SciGraphs' `_forceatlas2_layout` (`forceatlas.py:150`)
has three back ends. ForceSim (`:167`, compiled into scigraphs-utils) when `FORCESIM_AVAILABLE`,
else networkx's `forceatlas2_layout` (`:178-191`), else the `fa2` package. scigraphs-utils is in
no oracle image, so the reference arm most likely ran networkx. Prove it: run the reference arm
for one fixture, or read the harness and the `FORCESIM_REASON` it would print, and paste what
you find. If the reference ran ForceSim or `fa2`, stop and report under "decisions needed":
graph-core's FA2 is a networkx port, and the matching port would be a different job.

If it ran networkx, three facts set the start:
- The seed is not 981798123. It is `_get_layout_rng().randint(0, 2**31 - 1)`, the first draw
  of the per-call `RandomState(981798123)`. Legacy `randint` over that range is a masked
  rejection draw on 32-bit outputs: `next_u32() & 0x7fff_ffff`, redrawn while `> 2**31 - 2`.
  Confirm the value in `ge-python-oracle` and pin it in a test.
- networkx then calls `random_layout(G, dim, seed=<that int>)` (`drawing/layout.py:1696`):
  `RandomState(int).rand(n, dim) + center`, then `.astype(np.float32)` (`:120-121`). So the
  start is f32-narrowed MT19937 output.
- Read `forceatlas2_layout` (`:1604` onward) for the dtype of `pos_arr` after an in-place
  update. A float32 array keeps float32 under `+=` while the forces are float64. Trace it and
  state it; the motor must narrow where numpy narrows.
SciGraphs then rescales the result (`_fa2_rescale`, `forceatlas.py:47-56`). Port that into the
conformance motor arm only if the arm does not already do it.

Three call arguments the motor's FA2 does not take today (`state.rs:1-7`: dim 2,
`distributed_action=False`, no weight). The networkx branch passes them explicitly
(`forceatlas.py:178-191`), overriding networkx's own defaults (`layout.py:1612` `dim=2`,
`:1619` `distributed_action=False`):
- `dim=int(dim)`, and `dim` defaults to 3 (`forceatlas.py:150-153`). The start is
  `rand(n, 3)` row-major (x, y, z per node) and every force has a z term. Build the 3-D
  solve the way the spring kernel did (`layout/force/spring/forces.rs`: `Field<const D>`,
  `Solver<'a, const D>`, axis loop x then y then z), so `D = 2` performs the same operations in
  the same order and the registered FA2 stays byte-identical (hashgate record unchanged). The
  output Geometry carries the z column.
- `distributed_action=True`: `attraction /= mass[:, None]` (`layout.py:1818-1819`), mass =
  degree + 1 (`:1725`). Add it as a `Fa2Params` field, default off.
- `weight="weight" if edge_weight_influence else None`. Read `_build_networkx_graph` in the
  SciGraphs submodule and say whether conformance edges carry a `weight` attribute; if none
  does, every weight is 1 and nothing changes, write that down with the line.
Every new `Fa2Params` field updates every literal constructor (`git grep -n "Fa2Params {"`:
`state.rs`, `oracle_python/fa2.rs`, the conformance `motor.rs` and its tests). `state.rs` is
249 lines: split it before it crosses 300.

Do: RED test for the derived seed and the first two f32 start rows (3-vectors). GREEN:
`Fa2Params` gains the start path (`layout/forceatlas2/state.rs:30`, `initial_positions` `:240`):
a numpy start that leaves the registered default byte-identical, as sg-spring-seed does. Then the
conformance `motor.rs` passes it. Keep `jiggle` (`state.rs:150`) for coincident nodes unless
networkx does something else there; read it and say which.

Expected result, stated up front: networkx diverges from itself under a 1-ulp change of the
start (`prompts/RESUME.md` item 4), so after 50 iterations the f32 count may stay low even with
an exact start. The deliverable is the start matching bit for bit (test), plus the row's
measured disparity before and after. A disparity that falls with no count is a result; record it
and do not tune.

Close `G_FORCEATLAS2_SEED` only if the start matches. Paths: `layout/forceatlas2/*`, the
conformance `motor.rs`, `rows.rs`, `gaps.rs`, `baseline/table.rs` (FORCEATLAS2),
`docs/measurements/sg-fa2-seed.md`. Done when: the sg-common done-when holds and the FA2 oracle
gate (`graph-cli oracle-fa2`) still meets its ceiling.
