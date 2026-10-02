# Job perf-p3-pm-tiers (agent build): the particle-mesh layout under the thread tiers

Why: `layout.force.particle_mesh` (`docs/measurements/perf-p2-pm.md`) is the fastest force layout at
1M nodes (361 ms a tick, single thread), and the plan's P3 asks for it threaded. Its gathers already
take a runner: `ParticleMesh::run_with(topology, params, runner, workers)`
(`crates/graph-core/src/layout/force/particle_mesh.rs:66`). Nothing outside graph-core calls it with
`Threads`, so neither `bench --tiers` nor the hash gate's threaded arms ever ran it. This job routes it
and makes the existing split control reach it. No kernel changes.

Facts:

- **The negative control is hard-coded off.** `ParticleMeshRun::step_with` builds `How` with
  `split: Split::None` (`particle_mesh.rs:107`). The passes already read it:
  `how.split.splits(Split::Link)` (`:133`), `Split::Charge` (`particle_mesh/charge.rs:50`),
  `Split::Collide` (`particle_mesh/collide.rs:260`).
- **The precedent.** `BarnesHut::run_under(topology, params, runner, workers, split)`
  (`crates/graph-core/src/layout/force/barnes_hut.rs:163`), with its doc comment on why it is a separate
  function and not a defaulted argument (`:155-161`). `run_with` delegates to it with `Split::None`.
- **Bench routing.** `crates/graph-cli/src/bench/tiers/route.rs`: `ROUTES` (`:33`), the `run_once`
  match (`:75-86`), one helper fn per layout (`barnes_hut` at `:90`), the module doc. The report's
  sentence per layout: `crates/graph-cli/src/bench/tiers/markdown.rs` `stage_sentence` (`:93`). Tests:
  `crates/graph-cli/src/bench/tiers/tests/layout.rs` iterates `ROUTES`.
- **Hash gate routing.** `crates/graph-cli/src/hashgate/tiered.rs`: `THREADED_STAGES` (`:25`) and the
  `geometry` match (`:79-110`). The force arms take `setting.split_sum`. The arm doc in
  `crates/graph-cli/src/hashgate/tier.rs:17` lists the threaded stages in words. A test holds
  `THREADED_STAGES` against what the arm compared, and the `GM_MUTATE_SPLIT_SUM` tests say which stages
  must diverge; find them with `git grep -n 'THREADED_STAGES\|SPLIT_SUM' crates/graph-cli`.
- **Determinism** (CLAUDE.md "Determinism", `prompt.md` §6). The serial bytes must not move: the 65
  goldens and the registry hash of `layout.force.particle_mesh` stay as they are. The deposit and the two
  FFTs stay serial; say so in the stage sentence.
- **Limits.** Each file ≤ 300 lines, each fn ≤ 40 lines and ≤ 4 parameters. `particle_mesh.rs` is 140
  lines. `run_under` has five parameters, as `BarnesHut::run_under` does; copy its existing
  `#[allow]` line, if it has one, with its reason, and do not add a new suppression otherwise.
- Paths you may edit: `crates/graph-core/src/layout/force/particle_mesh.rs`, the four graph-cli files
  above and their tests, `docs/measurements/perf-p3-pm-tiers.md`. Nothing else.

Do:

1. `ParticleMesh::run_under(topology, params, runner, workers, split)`. `run_with` calls it with
   `Split::None`. `ParticleMeshRun::step_with` takes the split (or gains a `step_under`; pick the
   smaller diff and say which).
2. Add `ParticleMesh::ID` to `ROUTES` (after `YifanHu::ID`), with its `run_once` arms, its helper and
   its `stage_sentence`. Add it to `THREADED_STAGES` and the `geometry` match under `setting.split_sum`.
   Update the tests that list stages; each change keeps its intent.
3. Gates, in this order, each with its exit code in the report:
   - `scripts/orch/gr cargo fmt --all --check`
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings`
   - `scripts/orch/gr cargo test --workspace --no-fail-fast`
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all` (expect 0)
   - `scripts/orch/gr -e GM_MUTATE_SPLIT_SUM=1 cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all`
     (expect non-zero, and the divergence report names `layout.force.particle_mesh` among the stages)
4. Measure: `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.particle_mesh --tiers scalar,threads --n 100000,1000000 --past-ceiling`
   (read `bench --help` for the exact flags first; if it differs, use what it says and write the
   command you ran). Record the host load (`cat /proc/loadavg`) before and after each run.
5. Report `docs/measurements/perf-p3-pm-tiers.md`: what changed, the gate table with exit codes, the
   tier table (n, workers, ms, speed-up, the `equal` column), the load, and a "what it does not do"
   list (the serial deposit and FFTs, with their share of a tick if the bench prints passes).

Done when: the five gates exit as stated, the bench ran at both sizes, and the report states each.
