# Job gpu-g1a (agent build: `graph-cli emit-gpu-fixtures`, the CPU mesh's per-pass deltas)

Why: `docs/decisions/gpu-force-tier.md` (accepted 2026-10-03) moves the particle mesh's tick
to WebGPU as a new, explicitly requested tier, `layout.force.particle_mesh.gpu`. Its slice
G0 landed on 2026-10-05: `GM_GPU=1` gives a hardware adapter (`docs/measurements/gpu-adapter.md:30`),
so the tier can be gated in this container. G0 measured an adapter and nothing else — "no
force kernel, no throughput number, no port/canary work" (`gpu-adapter.md:190-194`). G1a is
the first slice that writes a byte: the fixtures G1b's WGSL charge kernels will be measured
against. Without them there is nothing to gate a kernel on, and a kernel with no oracle is a
kernel with a number.

G1a writes no force law, no WGSL and no browser code. It reads a live
`ForceSession` and writes six files.

Facts (2026-10-06; re-check each on your branch before editing, and stop if one no longer
holds):

- `crates/graph-core/src/layout/force/particle_mesh.rs:122-152` is the mesh's tick, in order:
  decay, link, charge, centre, collide, gravity, integrate. `charge::apply` at `:136`,
  `collide::apply` at `:138`, `motion::integrate` at `:150`.
- `crates/graph-core/src/layout/force/particle_mesh.rs:52-58` is `charge_pass`, the one
  single-pass entry point that exists, and its doc names the reason: each pass stays private
  to the module that owns it and the probe names the pass once. `link::pass_with` is at
  `crates/graph-core/src/layout/force/barnes_hut/link.rs:90`; `collide::apply` is
  `pub(super)` at `crates/graph-core/src/layout/force/particle_mesh/collide.rs:261`, and its
  merge is inside `motion::integrate`
  (`crates/graph-core/src/layout/force/particle_mesh/motion.rs:150-152`).
- `crates/graph-core/src/layout/force/session/fidelity.rs:56-74` is
  `ForceSession::charge_deltas(&self, theta) -> (Vec<f64>, Vec<f64>)`: the many-body probe.
  Its doc at `:11-17` is the property you must preserve — the pass *accumulates into*
  `vx`/`vy`, so the probe runs against a **copy** whose velocities start at zero, and then
  "at rest, `vx` after the pass is the force, to the last bit". Its constructor
  `charge_probe` is at `:84-92` and puts `alpha` at 1.
- `crates/graph-core/src/layout/force/session/fidelity.rs:26-29` is the caveat you inherit:
  a probe copy is at **tick 0** whatever the session's tick is, and the collide pass reads
  `tick_no` for its coincidence jiggle
  (`crates/graph-core/src/layout/force/particle_mesh/collide.rs:271-273`). Carry it into the
  fixture's README and the measurement doc.
- `crates/graph-core/src/layout/force/particle_mesh/mesh.rs:31-34` is `side_for(n) =
  ceil(sqrt n).next_power_of_two().clamp(128, 1024)`, and `MAX_SIDE = 1024` is at
  `crates/graph-core/src/layout/force/particle_mesh/fft.rs:21`. So `P` is 128 at 1k and 10k,
  256 at 50k, 1024 at 1M.
- `crates/graph-core/src/layout/force/particle_mesh/frame.rs:34-45` is `Frame`
  (`step`, `h`, `origin`, `cells`, `reach`) and it is **`pub(super)`**; `Mesh::frame` at
  `crates/graph-core/src/layout/force/particle_mesh/mesh.rs:43` is a private field with no
  accessor. `frame.rs:55` and `:74-89` are the rung ladder, `h = 2^(step/4)`; `:58-61` snaps
  the origin down to a multiple of `h`.
- `crates/graph-core/src/layout/force/particle_mesh/mesh.rs:119-125` recomputes the frame
  **from the positions, every tick**. So the start state and the settled state have different
  frames and different kernel spectra, and the fixture must carry both — one file per state.
- `crates/graph-core/src/layout/force/particle_mesh/fft.rs:61-68` is `Plan` with private
  `swaps`, `forward`, `inverse`; `fft.rs:88-95` computes the twiddles with
  `libm::cos`/`libm::sin`, and `fft.rs:130` reads stage `half` out of the contiguous run
  `forward[half..2*half]`. `crates/graph-core/src/layout/force/particle_mesh/kernel.rs:28-32`
  is `Kernel` with a private `spectrum`, and `kernel.rs:66-76` samples it pre-scaled by
  `1/P²`. Both are private to their own modules, so two one-line accessors are needed.
- `crates/graph-core/src/layout/force/mod.rs:64-69` states the adjacency collapse rule:
  undirected, deduplicated, self-loop-free, **the first raw edge by ascending index keeps
  its strength**. `mod.rs:71-80` is `SimpleGraph { lo, hi, strength }`, `pub(crate)`.
- `crates/graph-core/src/layout/force/params.rs:27-60` is `ForceParams` and `:62-79` its
  `Default`; `distance_max = 520.0` at `:68` is what sets the kernel's reach, and `TICKS =
  112` at `:83` is the frozen tick count.
- `crates/graph-core/src/mb_fidelity/measure.rs:19` is `SETTLE_TICKS = 100`, with the reason at
  `:16-18`: 100 is the number every other measurement in this repo calls settled. Reuse it.
  `crates/graph-cli/src/mb_fidelity/measure.rs:6-8` is why both a start and a settled set
  exist at all.
- `crates/graph-core/src/stage/topology.rs:26-35` is
  `seeded_model(seed, count, reference_degree) -> (Vec<NodeRecord>, Vec<EdgeRecord>)`, the
  repo's own gate model, and `crates/graph-core/src/oracle_python/cli.rs:193-195` is where
  another emitter says so in a doc comment. Underneath,
  `crates/graph-core/src/synthetic.rs:92-97` and `:131-170` draw preferential attachment
  plus 5 % `note_link` extras from `Mulberry32::new(SEED)` with
  `SEED = 0x05_1042` at `:31`, so `m ≈ 1.55n`.
- `crates/graph-core/src/layout/force/barnes_hut/seed.rs:12-13` is `spiral_point`,
  `r = 12·√(i+1)`, and `:8` is `GOLDEN_ANGLE`. This is where the record's "the seed spiral
  reaches 12·√n = 12 000 units at 1M" comes from
  (`docs/decisions/gpu-force-tier.md:91-93`), and it is why **no fixture value is `f32`**.
- `crates/graph-cli/src/oracle_python/cli.rs:17-24` is `EmitSpectralFixtures { seeds, out }`
  and `crates/graph-cli/src/oracle_python/cli/run.rs:15` its dispatch — the pattern, split
  across two files by the 300-line limit (`cli/run.rs:1`). **That emitter has no `--check`**;
  the byte-compare to copy is `crates/graph-cli/src/ingest_cmd.rs:102-124` (`up to date` /
  `STALE` / `first difference at byte {at}`, exit 1 stale, 2 unreadable).
- `crates/graph-cli/src/oracle_python.rs:160-180` is the emit's manifest: `sha256` of the
  payload plus the tree fingerprint, and `:204-211` the verdict that refuses a result
  computed from other fixtures. `crates/graph-cli/src/fingerprint.rs:54-73` is the
  fingerprinted list, and it **includes `fixtures/`**.
- `crates/graph-contract/src/snapshot.rs:7-20` is the 28-byte header table to model, and its
  line 20 is the rule this format inherits: "Every integer on the wire is `u32` or a single
  tag byte — never `usize` (D6)". `docs/contract/binary-layout.md` is the authoritative
  statement of the discipline.
- `crates/graph-cli/src/command.rs` is **297 lines** (`command.rs:170-173` is the last entry)
  and `crates/graph-cli/src/oracle_python/cli.rs` is **277**. `command.rs:170-173` is the
  `#[command(flatten)]` pattern; the new command needs its own `Cli` module and exactly two
  lines in `command.rs`.
- `crates/graph-sdk-js/package.json:8-14` is the `exports` map and
  `package.json:59` in the repo root is `sdk:test`,
  `node --test --experimental-strip-types "crates/graph-sdk-js/test/**/*.test.mjs"`.
  `crates/graph-sdk-js/src/index.ts` is the barrel.
- `scripts/orch/gate.sh:24` is the rows parse (`name|expect|cmd`), `:29-31` the two allowed
  `expect` values, `:40-41` the `summary.txt` line. `scripts/orch/rows/gpu-g0.rows:1-4` is
  the four-row style, `scripts/orch/rows/force-warm.rows:22-38` the merge floor plus the sdk
  and studio rows, and `scripts/orch/rows/dag-lanes.rows:12-18` a rows file with the merge
  floor and slice rows together.
- `docs/decisions/gpu-force-tier.md:88-90` is the deposit quantum, **2⁻¹¹** of a unit charge
  at 1M, with the `3·10⁻⁴` per-cell figure and the 3.6e-3 rms the mesh already carries. The
  `scale_for(n) = 1 << (31 - (32 - n.leading_zeros()))` derivation in Task 1 reaches
  `2^11 = 2048` from that, in integer arithmetic and no `libm`. G1b needs both sides of it,
  so the TS twin ships with this slice.


## The verdict's conditions, made concrete (these override the plan's Task 1 code)

1. **One type and one method.** The public surface is exactly this. Field names are yours, but
   keep the plan's meaning for each:
   ```rust
   /// The CPU mesh at one session state: its adjacency, frame, twiddles, kernel spectrum, and
   /// what each force pass adds to the velocities from rest.
   pub struct MeshProbe { /* public fields:
       lo, hi, strength;                                   (was MeshGraph)
       side, step, h, origin_x, origin_y, cells, reach;    (was MeshFrame, inlined)
       twiddle_re, twiddle_im, spectrum_re, spectrum_im;   (was MeshSolution)
       link_dx, link_dy, charge_dx, charge_dy, collide_dx, collide_dy  (was mesh_pass_deltas) */ }
   impl ForceSession {
       /// `None` when there is no field to solve (the plan's `mesh_solution` cases).
       pub fn mesh_probe(&self) -> Option<MeshProbe>;
   }
   ```
   - `MeshPass`, `MeshGraph`, `MeshFrame` and `MeshSolution` are not public types.
   - The pass enumeration stays `pub(in crate::layout::force)` on `particle_mesh::pass`.
   - `force/mod.rs` re-exports `MeshProbe` and nothing else new.
   - Rewrite every plan test that used the old names against these fields. Keep each test's
     assertion; only the access path changes.
2. No `#[allow]` and no `#[doc(hidden)]`.
3. Add the test `the_probe_leaves_the_next_tick_byte_identical`. It builds two identical mesh
   sessions, calls `mesh_probe()` on one, `step(1)` on both, and asserts that the position
   columns and `tick_no` are bit-equal. `a_delta_is_the_pass_and_not_the_tick` also pins
   `tick_no`, one pin and `params()`.
4. The hash gate does not move. `hashgate --seeds 8` keeps the same digests; a moved hash is a
   finding, not a re-baseline.
5. `fixtures/gpu/README.md` is normative-complete: every offset of the 64-byte header and every
   payload section, each length derived from `n`, `m` and `P`. Add the TS test
   `the_loader_refuses_a_version_it_does_not_know`: format major 2 must throw.
6. **Keep 1M.** `SIZES` gains `1_000_000`.
   - `emit-gpu-fixtures --out target/gpu-fixtures` writes `mesh-1m-start.gmfx` and
     `mesh-1m-settled.gmfx`, about 100.7 MiB each (estimated), never committed.
   - Strike the base64 handoff for any file. G1b will fetch fixtures over the harness's own
     origin; record that in the measurement doc as the decided transport.

Also fix, in `fixtures/gpu/README.md` and `docs/measurements/gpu-g1.md`, the two derivations the
verdict found wrong:
- condition 7: the charge `maxAbs` is `2⁻¹¹/√3 = 2.8e-4`, giving `3.7e-5`;
- condition 8: the link ceiling is `k_measured · 5 · 2⁻²³`.

These are the documents G1b reads.

`scripts/orch/rows/gpu-g1a.rows` is already on develop: do not edit it.

Rules (beyond `scripts/orch/common.md`):

- Every cargo call is
  `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo …` (the long form is in the
  plan's Global Constraints, constraint 9). Every node call is `scripts/orch/node-slim.sh …`.
- House limits: ≤ 40 lines a function, ≤ 4 parameters (not `self`), ≤ 300 lines a file,
  nesting ≤ 3. Split into child modules; never compress.
- graph-core: no new dependency, no `unsafe`, no `HashMap`/`HashSet` (D4), no `mul_add`
  (D2), `libm` for every transcendental (D1), wire integers `u32` (D6).
- **TDD**: write the test, run it, observe RED, then implement. Every gate row has a
  negative control that must fail.
- **The verdict is already on develop:** `docs/decisions/gpu-g1.md`, "G1a
  PROCEED-WITH-CONDITIONS, 2026-10-06". Its conditions 1 to 6 are this job's acceptance
  criteria. Where they differ from the plan or from this brief, the conditions win. Read the
  whole record before Step 2, and do not edit it.
- **The hash gate must not move.** graph-core gains a probe and no arithmetic, so
  `hashgate --seeds 8` stays green with the same hashes. If it does not, that is a finding
  and a report, never a re-baseline.
- No test that counts registered layouts changes and no `LAYOUTS` entry is added: the tier
  is **not** in graph-core (`docs/decisions/gpu-force-tier.md:36-38`).
- `fixtures/gpu/README.md` states the format, the estimated sizes with the word *estimated*
  beside every one, the `m ≈ 1.55n` assumption, and both caveats (tick-0 collide, 100-tick
  settled).
- Only the 1k pair is committed. `target/gpu-fixtures/` holds all six; the 10k and 50k files
  are never committed, and the reason is in the plan's "Decisions taken".
- Nothing in `crates/` may name a data source. Check with
  `git grep -n -i -E '\bgit\b|commit|repositor|activitywatch' -- crates/graph-core/src/layout/force crates/graph-cli/src/gpu_fixtures.rs crates/graph-cli/src/gpu_fixtures crates/graph-sdk-js/src/gpu`.
- Out of bounds: `crates/graph-wasm/`, `crates/graph-contract/`, `src/`, `tests/`,
  `verify/`, `server/`, `app/`, `harness/`, `deploy/`, `packages/`, `.claude/`, and every
  other file under `fixtures/`.

Paths you may touch:

- `crates/graph-core/src/layout/force/particle_mesh.rs`, `.../particle_mesh/mesh.rs`,
  `.../particle_mesh/fft.rs`
- `crates/graph-core/src/layout/force/session.rs`,
  `crates/graph-core/src/layout/force/session/mesh_probe.rs`,
  `crates/graph-core/src/layout/force/session/mesh_probe/tests.rs`,
  `crates/graph-core/src/layout/force/session/fidelity.rs`
- `crates/graph-core/src/layout/force/mod.rs`
- `crates/graph-cli/src/gpu_fixtures.rs`, `crates/graph-cli/src/gpu_fixtures/**`
- `crates/graph-cli/src/command.rs`, `crates/graph-cli/src/main.rs`
- `crates/graph-sdk-js/src/gpu/fixture.ts`, `crates/graph-sdk-js/src/gpu.ts`,
  `crates/graph-sdk-js/test/gpu-fixture.test.mjs`,
  `crates/graph-sdk-js/test/gpu-fixture.control.mjs`
- `fixtures/gpu/**`
- `docs/measurements/gpu-g1.md`

Done when:

- each of the verdict's conditions 1 to 6 is met, and `docs/measurements/gpu-g1.md` has one line
  per condition naming the test, row or command that shows it;
- `scripts/orch/gate.sh target/rows-gpu-g1a scripts/orch/rows/gpu-g1a.rows` writes a
  `summary.txt` with every row PASS, including the three negative controls
  (`negctl-pass`, `negctl-rung`, `negctl-absent`) and the loader's
  (`negctl-loader-scale`);
- `fixtures/gpu/` holds `mesh-1k-start.gmfx`, `mesh-1k-settled.gmfx` and the README, and
  `graph-cli emit-gpu-fixtures --check fixtures/gpu` is green on a fresh clone;
- `docs/measurements/gpu-g1.md` holds the gate table, the six real file sizes against the
  plan's estimates, the four frames the emitter computed, and the two Caveat lines.

Return: the plan's Task 1 return block, filled — including the six real byte counts, the
verdict, and every deviation.
