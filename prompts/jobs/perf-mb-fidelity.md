# Job perf-mb-fidelity (agent build: measure how far each many-body solver is from the exact sum)

Goal: `docs/decisions/obsidian-force.md` decision 2 lets a many-body solver stand in for Barnes-Hut
θ = 0.9 only if its force error against the exact all-pairs sum is no larger than Barnes-Hut's own
error on the same positions. Build the instrument that measures this, and report the particle
mesh's number.

Facts (verified on develop b2cbbcd9):
- Barnes-Hut's many-body pass is a port of `manyBody.js` (`crates/graph-core/src/layout/force/barnes_hut/charge.rs:1-12`).
  Its parameters are `theta = 0.9`, `distance_min = 1.0` and `distance_max = 520.0`
  (`crates/graph-core/src/layout/force/params.rs:39-43,66-68`). The pass is
  `charge::apply_with(sim, runner, workers, deltas, split)` (`charge.rs:39-51`), and it is
  `pub(super)`.
- The particle mesh's many-body pass is `particle_mesh/charge.rs:39` `apply`, also private: it
  adds `charge * alpha * E(x_i)` to each node's velocity.
- `ForceSession::from_positions(topology, params, xs, ys)` (`session/warm.rs:13-22`) places a
  session at given positions with velocities at rest. `with_particle_mesh()` (`session.rs:147`)
  switches the engine. `xs()`/`ys()` (`session.rs:214,219`) read positions back.
- The pinned d3 source is at `$GM_SCRATCH/refs/npm/d3-force-3.0.0/src/manyBody.js` (fetched by
  `scripts/orch/fetch-refs.sh`). Its per-pair rule:
  - skip a pair when `l >= distanceMax2`;
  - jiggle when `x == 0` or `y == 0` in the difference;
  - when `l < distanceMin2`, use `l = sqrt(distanceMin2 * l)`;
  - add `x * strength * alpha / l`.
- `graph-cli` records gate evidence the way `oracle_python.rs:172-209` (`ingest`, `verdict`) does,
  through `crates/graph-cli/src/evidence.rs`.

Do, in order:
1. Add one public probe to graph-core, in a new child module `session/fidelity.rs`:
   `ForceSession::charge_deltas(&self, theta: f64) -> (Vec<f64>, Vec<f64>)`.
   - It clones the session's state, zeroes the velocities, sets alpha to 1 and runs only the
     many-body pass of the session's engine (Barnes-Hut at the given `theta`, or the mesh, which
     ignores it). It returns the velocities.
   - Its doc line says it is an instrument for `graph-cli mb-fidelity` and nothing in the product
     calls it.
   - If `theta` cannot reach the pass without changing a frozen default, thread it through the
     pass's context only. `ForceParams::default()` must not change, and the hash gate proves it.
2. Write the exact reference in `graph-cli`, never in graph-core: an O(n²) all-pairs sum ported
   line for line from the `manyBody.js` per-pair rule above, at alpha 1, with the same
   `distance_min`/`distance_max` and the session's `chargeStrength`. Skip coincident pairs and
   count them; a run with any coincident pair says so in its output.
3. Self-check the reference before trusting it: Barnes-Hut at θ = 0 opens every cell, so it is the
   exact sum in a different order. The relative RMS difference between the two must be ≤ 1e-9.
   If it is not, stop and report: one of them is wrong.
4. Add `graph-cli mb-fidelity` with options `--n <list>` (default `1000,10000,50000`) and
   `--require <solver>` (repeatable). A solver is `pm` or `bh:<theta>`.
   - Positions, two sets per n:
     - `start`: the session's own seed positions;
     - `settled`: after 100 Barnes-Hut ticks on the scale model (`bench/scale.rs` `scale_model`).
   - For each set and solver, report:
     - the relative RMS error, `sqrt(Σ|F − F*|²) / sqrt(Σ|F*|²)`, where F* is the exact sum;
     - the per-node relative error's median and p99, over nodes where `|F*| > 0`.
   - A solver passes on a set when its relative RMS error ≤ that of `bh:0.9` on the same set.
   - Print a markdown table: n, set, solver, rms, p50, p99, pass.
   - Exit codes: 0 = ran, the step-3 self-check held, and every `--require` solver passed on every
     set; 1 = ran and either failed; 2 = could not run.
   - Record the result through `evidence.rs` as `mb-fidelity`, the same way the oracles record.
5. Controls, each a test in `crates/graph-cli/tests/` (look the gate up by id, never by row
   index):
   - positive: `--n 1000 --require bh:0.5` exits 0;
   - negative: `--n 1000 --require bh:2.0` exits 1.
6. Run `--n 1000,10000,50000 --require pm` once, release build. Report the table whatever the exit
   code: a failing mesh is the result, not a defect of this job.

Out of bounds: the frozen defaults, the wire format, the wasm ABI, `packages/`. No `unsafe`, no new
dependency, no `HashMap` where order is observable (D4). House limits: 40 lines a function,
4 parameters, 300 lines a file, nesting ≤ 3. Heuristic thresholds carry a `Caveat:` line.

Paths you may edit: `crates/graph-core/src/layout/force/session.rs` (the `mod` line only),
`crates/graph-core/src/layout/force/session/fidelity.rs` (new), the Barnes-Hut charge context if
step 1 needs θ threaded, `crates/graph-cli/src/` (a new `mb_fidelity` module plus its line in
`command.rs`/`main.rs`, edited additively), `crates/graph-cli/tests/`,
`docs/measurements/perf-mb-fidelity.md`.

Done when:
- `scripts/orch/gr cargo fmt --all --check` exits 0.
- `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- `scripts/orch/gr cargo test --workspace --no-fail-fast` exits 0, including both step-5 controls.
- `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` exits 0.
- `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` exits 0, and the same run with
  `-e GM_MUTATE_REFERENCE_DEGREE=9` exits non-zero.
- `docs/measurements/perf-mb-fidelity.md` holds:
  - the step-3 self-check numbers;
  - the step-6 table and its exit code;
  - the host load;
  - what the instrument does not measure: the forces other than many-body, and trajectories over
    many ticks.
