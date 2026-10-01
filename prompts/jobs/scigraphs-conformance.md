# Job scigraphs-conformance (agent build, measurement only: no motor change)

Why: the user wants every SciGraphs layout compared **byte by byte** with the motor and every shape checked
by eye. Today each oracle checks one family against a tolerance (`docs/measurements/scigraphs-coverage.md`),
several against a library rather than SciGraphs' own code, and nothing shows the shapes side by side.
This job produces the one matrix that says, per SciGraphs name, how close the motor is and why it is not
closer. Repairs are separate jobs written from your matrix.

Facts (checked 2026-10-01):
- `SciGraphs/core/scigraphs_core/mesh/layouts/dispatcher.py:14` `apply_graph_layout(obj, algorithm,
  iterations=50, scale=5.0, props=None, edge_pairs=None)` takes a plain dict `obj` with `edges_data` (or
  `edge_pairs`), calls `_reset_layout_rng()` first (`common.py:53`: numpy `RandomState(get_layout_seed())`
  and the stdlib `random`), and writes `obj["node_positions"]` (flat xyz) and, on a fallback,
  `obj["layout_substituted"]`. So every SciGraphs name is deterministic per call: the reference is one run.
- `ge-python-oracle` has numpy 2.3.3, scipy 1.16.2, networkx 3.6, igraph 0.11.9; it has NO pygraphviz,
  matplotlib or fa2_modified. SciGraphs' Graphviz engines and Yifan Hu go through `scigraphs_utils`
  (`common.py:145`, `yifan_hu.py:342,366`), absent here; those names use the Graphviz 16.1.0 arm we already
  have (`ge-graphviz-oracle`, `harness/oracle-graphviz.py`), and the matrix says so per row.
- Existing pattern to follow (emit, arm, judge): `graph-cli emit-basic-3d-fixtures` ->
  `harness/oracle-basic-3d.py` -> `graph-cli oracle-basic-3d` (`crates/graph-cli/src/oracle_python/`).
- The 32 names and their motor ids: `docs/measurements/scigraphs-coverage.md`. `fixtures/scigraphs/lesmis.json`
  is the Les Miserables graph (77 nodes, 254 edges) SciGraphs' gallery uses.

Do:
1. Fixtures: lesmis, plus the gate model at seeds 0..19 capped at 200 nodes, plus one rooted tree, one DAG and
   one bipartite graph from graph-cli's existing generators. graph-cli emits them once; both arms read the
   same file. Node `i` of SciGraphs' graph is the motor node whose id sorts `i`-th (byte order); write the
   mapping into the fixture.
2. Motor arm: graph-cli `emit-conformance-fixtures` runs every motor id mapped to a SciGraphs name on every
   fixture, with SciGraphs' defaults wherever the motor has the parameter (iterations 50, scale 5.0, its seed),
   and writes positions as raw little-endian f64 (and the f32 the snapshot narrows to). A parameter the motor
   lacks is recorded per row as a convention gap with the motor's `file:line`, never silently normalised.
3. Reference arm: `harness/scigraphs-conformance.py` in `ge-python-oracle` with `SciGraphs/` mounted, calling
   `apply_graph_layout` itself for all 32 names (Graphviz names: the `oracle-graphviz.py` arm in
   `ge-graphviz-oracle`). Record `layout_substituted`, library versions, and the raw f64 bytes.
4. Metrics per name, over all fixtures: coordinates compared; bitwise-equal f64 count; bitwise-equal after f32
   narrowing; max ULP (f64); max absolute gap; Procrustes disparity (translation, uniform scale, rotation
   and reflection; `scipy.spatial.procrustes`) median and max. Compute in Python, judge in graph-cli
   (`graph-cli scigraphs-conformance`), which writes `target/gates/scigraphs-conformance.json` and exits
   0/1/2 like the other oracles.
5. Classify every row that is not bitwise equal by its FIRST cause, with evidence (`file:line` on both sides):
   `convention` (scale, centring, axis order, z, node order), `rng` (the motor's PRNG is not numpy's
   MT19937 `RandomState`/stdlib `random` stream), `arithmetic` (reduction order, transcendental, FMA),
   `algorithm` (a different method or step), `reference-absent` (could not run here). Tier each row:
   `bitwise` (achievable: closed form, or RNG-port away), `tolerance` (deterministic iteration), `shape`
   (stochastic). State the tier rule in the doc.
6. Shapes: the arm writes one SVG per name (no new dependency: SVG is text), SciGraphs left, motor right,
   motor Procrustes-aligned and overlaid in a second colour, on lesmis and on the tree; then
   `scripts/scigraphs-conformance.sh` renders them to PNG with the repo's `gm-chromium` image (see
   `scripts/studio-parity.sh` for how it drives Chromium) into `target/scigraphs-conformance/`, plus one
   contact sheet `sheet.png` of all 32. Look at every PNG yourself and write a one-line shape verdict per row
   (same shape / mirrored / rotated / different shape: what differs).
7. `scripts/scigraphs-conformance.sh` runs steps 1-6 end to end (header = its manual, like `studio-parity.sh`),
   with `--break`: flips one bit of one motor coordinate, and the run must then exit 1 naming that row.
8. Write `docs/measurements/scigraphs-conformance.md`: the 32-row matrix (name, motor id, reference reached,
   tier, bitwise f64 k/N, bitwise f32 k/N, max ULP, max gap, Procrustes median/max, cause, shape verdict),
   then a "repairs" list ordered by value: each a concrete change (file, what, expected new metric) that
   would move a row up a tier. Embed nothing; link the PNGs by path.

Paths: `harness/scigraphs-conformance*.py`, `crates/graph-cli/src/oracle_python/conformance*` (+ additive
registration in its `cli.rs`/`mod`), `scripts/scigraphs-conformance.sh`,
`docs/measurements/scigraphs-conformance.md`, one row `scigraphs-conformance|zero|...` and its negctl
`|nonzero|... --break` appended to `scripts/orch/rows/develop-full.rows`. No change under `crates/graph-core`.

Done when: the script exits 0 on the tree and 1 with `--break`; the matrix has 32 rows with no blank cell
(a cell that could not be measured says `not run: <reason>`); every non-bitwise row has a cause with
`file:line`; you looked at every PNG; quick.rows green.
