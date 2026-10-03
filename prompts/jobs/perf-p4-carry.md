# Job perf-p4-carry (agent build): a live force session that survives a graph change

Why: the plan's P4 (`prompts/perf-plan.md`) is a graph that grows while an AI process emits nodes and
edges, with the layout continuing instead of restarting. The topology is immutable by design, so a
change is a **new** topology. This slice is the motor half: carry a running session from the old
topology to the new one, keeping every surviving node's state. The wasm ABI, the SDK and the studio
are later slices and are out of scope here.

Facts:

- **The session.** `ForceSession` (`crates/graph-core/src/layout/force/session.rs:85`) owns a `Sim`
  (`barnes_hut/sim.rs`) and a scratch `Vec`. The state a tick reads: `x`, `y`, `vx` (`sim.rs:55`), `vy`,
  the pins `fx` (`sim.rs:59`) and `fy`, `alpha`, `alpha_target`, `tick_no`, and the parameters.
  `session.rs` is already 316 lines, so the new code goes in a child module `session/carry.rs`,
  declared next to `session/pin.rs`.
- **The warm-start precedent.** `ForceSession::from_positions` (`session.rs:139`) with `set_positions`
  (`:264`): a session over a topology from given columns.
- **Ids.** `Topology::node_index(id) -> Option<u32>` and `Topology::node(i).id`
  (`crates/graph-core/src/index/view.rs:33,45`); adjacency through `Topology::incident(node)` (`:118`).
  Only ids map between the two topologies: a dense row means nothing across them.
- **Seeding.** A fresh session seeds on the golden spiral, d3's phyllotaxis
  (`barnes_hut/seed.rs:11`). No RNG anywhere (CLAUDE.md "Determinism", D8).
- **Complexity** (the user's rule for every perf phase): `O(n + m)` time and one pass per column; no
  `O(n²)`, no per-node allocation, buffers sized once.

Do:

1. `ForceSession::carry(&self, from: &Topology, to: &Topology) -> Result<ForceSession, SessionError>`:
   - a node of `to` whose id is a row of `from` keeps `x, y, vx, vy` and its pins;
   - a node of `from` absent from `to` is dropped;
   - `alpha`, `alpha_target`, `tick_no` and the parameters carry over unchanged;
   - a **new** node with at least one carried neighbour in `to` starts at the mean of those neighbours
     plus a small phyllotaxis offset by its index among the new nodes (the same angle as `seed.rs`,
     a radius you choose and justify in one line), so two new leaves of one hub never coincide; at
     rest (`vx = vy = 0`);
   - a new node with no carried neighbour takes the golden-spiral position of its row in `to`;
   - refuse with a `SessionError` when `from`'s node count is not the session's row count.
   Write the order of every loop so the result does not depend on hash order (`IndexMap`, ascending rows).
2. Tests in `session/tests/` (one file, ≤ 300 lines):
   - **identity:** `s.carry(t, t)` then `step(k)` is byte-identical to `s.step(k)` on a clone, for
     k = 1 and 30, on the seeded gate model (`graph_core::seeded_model`, `index_model`);
   - **grow:** carry from the first 90 % of a model's nodes (and the edges among them) to the whole
     model: every old id keeps its exact `x, y, vx, vy`; every new id with a placed neighbour sits
     within the offset radius of its neighbours' mean; no two positions are equal;
   - **shrink:** remove 10 % of the nodes; the survivors keep their bytes;
   - **pins:** a pinned node stays pinned at the same coordinates across a carry;
   - **refusal:** a `from` of the wrong size is refused and allocates no session.
3. Measure in graph-cli, beside `tick` (`crates/graph-cli/src/main.rs:105`, `bench/tick.rs`): a
   `grow` subcommand, or a flag on `tick`, whichever is the smaller diff. It builds the gate model at
   `--n`, the topology without the last `--batch` nodes, steps 3 ticks, then times (median of 3):
   building the new topology (`index_model`) and the `carry`. Print one markdown table with
   `/proc/loadavg`. Run it release at n = 100 000 and 1 000 000 with batch 10 000.
4. Gates, each with its exit code in the report:
   - `scripts/orch/gr cargo fmt --all --check`
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings`
   - `scripts/orch/gr cargo test --workspace --no-fail-fast`
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown`
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` (expect 0: no existing byte moved)
5. Report `docs/measurements/perf-p4-carry.md`: the API, the seeding rule and its reason, the gate
   table, the timing table (topology build and carry separately, at both sizes, with the load), the
   plan's budget (≤ 30 ms per batch at 1M) met or missed, and a "what it does not do" list.

Paths you may edit: `crates/graph-core/src/layout/force/session.rs` (the `mod` line and re-exports
only), `crates/graph-core/src/layout/force/session/`, `crates/graph-cli/src/main.rs`,
`crates/graph-cli/src/bench/`, `docs/measurements/perf-p4-carry.md`. Nothing in `graph-wasm`,
`graph-contract`, the SDK, `packages/` or `app/`. graph-core gains no dependency.

Done when: the five gates exit 0, the timing table has both sizes, and the report states each.
