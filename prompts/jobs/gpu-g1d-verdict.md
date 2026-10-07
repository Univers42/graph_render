# Job gpu-g1d-verdict (agent devil, docs only: rule on G1d's amended surface before code)

Why: G1d (the GPU tier's SDK option, studio toggle and fallbacks) was ruled
PROCEED-WITH-CONDITIONS in `docs/decisions/gpu-g1.md` (ruling 6, conditions 10-12) on the plan
`docs/superpowers/plans/2026-10-06-gpu-g1.md` Task 4. Building it shows that Task 4 cannot be
built as written. The orchestrator's amendment below changes the public surface: graph-core,
the wasm ABI, the SDK and the studio's worker loop, which is a concurrency change. The house
rule (`.claude/rules/devil/risk.md`) needs a verdict before code. You rule on the amendment.
You write no code.

Read, in full:
- `docs/decisions/gpu-g1.md`, `docs/decisions/gpu-force-tier.md`, `docs/decisions/compute-tiers.md`;
- the plan's Task 4 (`docs/superpowers/plans/2026-10-06-gpu-g1.md:1407-1500`);
- `docs/measurements/gpu-g1.md`, section "G1c — the resident tick, measured";
- every file cited below, at the lines cited.

## Why Task 4 cannot be built as written (facts, develop 85858136; re-check each)

1. **The GPU arm has no live input.**
   - `runTick` reads `.gmfx` bytes only (`crates/graph-sdk-js/src/gpu/tick.ts:87`).
   - A live session's link graph is graph-core's `SimpleGraph`: deduplicated, no self-loops, the
     first raw edge keeps its strength. It is `pub(crate)` (`crates/graph-core/src/layout/force/mod.rs:71-80`).
   - The SDK holds only a graph handle (`crates/graph-sdk-js/src/force-create.ts:17`).
   - So the plan's line "Nothing in `crates/graph-wasm/`" (plan `:1429`) cannot hold for a live tick.
   - The same goes for the session's velocities: there is no export for them.
2. **`startGpuMesh({ticks}) -> report` is one-shot** (plan `:1446`). A settle that a person drags
   needs a handle that ticks, pins, takes parameters and releases.
3. **The GPU tick hard-codes the frozen parameters and has no gravity.**
   - The constants: `gpu/link.ts:93-94`, `gpu/charge.ts:132`, `gpu/charge-kernel.ts:27-28`,
     `gpu/tick-step.ts:47`, `gpu/collide-grid.ts:15`, `gpu/motion.ts:35-38`, `gpu/tick.ts:79`.
   - Gravity is not in the tick (`gpu/tick-step.ts:7-11`).
   - The studio's nine knobs (`packages/graph-studio/src/motor/liveSession.ts`) would do nothing
     on the GPU arm, and nothing would say so.
4. **The studio's live session is Barnes-Hut under 5 000 nodes**
   (`packages/graph-studio/src/motor/settle.ts:37`). The GPU arm's CPU twin and its fallback
   must be the mesh.
5. **The worker's loop steps synchronously** (`packages/graph-studio/src/motor/loop.ts:94`). A
   GPU tick is async: two `mapAsync` read-backs per tick (`gpu/tick-step.ts:85-102`).

## Amendment 4 — the proposal you rule on

### A. graph-core: three read accessors, no new type

- Add to `ForceSession`, in a new child module `session/columns.rs` (`session.rs` is at 298 of 300 lines):
  - `pub fn simple_edges(&self) -> (&[u32], &[u32], &[f64])`: `lo`, `hi`, `strength`, a borrow of `Sim.graph`;
  - `pub fn vxs(&self) -> &[f64]` and `pub fn vys(&self) -> &[f64]`, the velocity columns.
- There is no mutating method. The host writes velocities through the wasm view, as it already
  writes positions (`crates/graph-wasm/src/session.rs:218-247`).
- Tests:
  - `simple_edges_are_the_probes_own`: equal to `mesh_probe()`'s `lo`/`hi`/`strength`;
  - `the_accessors_leave_the_next_tick_byte_identical`.

### B. graph-wasm: four additive exports, `ABI_VERSION` stays 2

- **Velocity columns.** `gm_force_session_velocity_ptr(session, axis) -> u32` and
  `gm_force_session_velocity_len(session, axis) -> u32` mirror `gm_force_session_column_ptr/len`:
  - axis 0 is `vx`, axis 1 is `vy`;
  - an empty column, a dead session or a bad axis reads `0`, as C3 says.
- **The simple graph.** `gm_force_session_edge_ptr(session, column) -> u32` (column 0 `lo` u32,
  1 `hi` u32, 2 `strength` f64) and `gm_force_session_edge_len(session) -> u32`.
- **Additive.** `docs/contract/wasm-abi.md:31` says an added export never bumps the version
  (precedent: `gm_graph_extend`, `gm_force_session_grow`).
- **Placement.** The code goes in new files `session/handoff.rs` and `exports/session_handoff.rs`:
  `session.rs` is at 296 lines and `exports/session.rs` at 295.
- **Docs and SDK.** `docs/contract/wasm-abi.md` gains the four rows and the C7 lifetime line.
  `crates/graph-sdk-js/src/wasm.ts` names the four in `EXPORT_NAMES`.

### C. SDK

**The engine value and its refusal.**
- `ForceEngine` gains `"particle_mesh_gpu"`, as ruling 6 says; the constant is `GPU_ENGINES`.
- `createForceSession` refuses it with `GpuMeshRefusedError`, a new class in `errors.ts`, before
  any wasm call, so no session is left behind.

**Starting a GPU mesh.** `ForceSession.gpuMesh(options?: { host?: GpuHost; arm?: "hardware" | "any" }): Promise<GpuMesh>`.
- It replaces the plan's one-shot `startGpuMesh`.
- It refuses, with `GpuMeshRefusedError`:
  - a session that is not `particle_mesh`;
  - a released session;
  - a session that already has a live GPU mesh.
- It never rejects for "no adapter". It resolves to a `GpuMesh` whose `tier` is `"cpu-no-adapter"`
  and whose `reason` is the refusal's text. With `arm: "hardware"`, a software adapter is refused
  the same way, and the reason names it.

**`GpuMesh`, the handle.**
- Fields: `tier: "gpu" | "cpu-no-adapter" | "cpu-device-lost"`, `reason: string | null`, `marks: string`.
- `tick(ticks): Promise<ForceTick>`:
  - ticks are serialized, so a second call waits for the first;
  - after each batch the `f32` positions are written into the session's own `f64` columns, so
    `session.positions()` stays the only read path;
  - alpha is decayed on the host with the session's `alpha_decay` toward 0, and written back with `reheat`.
- `pin/drag/unpin/unpinAll/reheat/setParams` forward to the session at once. They are staged for
  the device and applied at the start of the next tick, never mid-tick, so a verb takes effect on
  the next tick, as on the CPU (`force.ts:113-114`).
- `release(): Promise<void>` hands the state back:
  - it reads back `x` and `v` and writes both into the session;
  - it destroys the device and the held read-back;
  - it is idempotent.
- **One driver at a time.** While a `GpuMesh` is live, `session.tick` and `session.grow` refuse
  with `GpuMeshRefusedError`, because two drivers of one simulation diverge without a word. The
  handle's CPU fallback uses a private path.
- **A session that grew.** `GpuMesh.grow(handle)` runs the CPU grow and rebuilds the rig at the
  next tick:
  - new edges come from `simple_edges`;
  - old velocities are read back from the old rig, and new nodes take the session's velocities.
- **Parameters.** All 13 are honoured except `theta` (Barnes-Hut only) and `initial_alpha` (used
  at creation only):
  - `charge` and `alpha_decay`;
  - `distance_min` and `distance_max`: the kernel's rung key gains both;
  - `link_distance` and `link_strength_scale`: the link geometry is re-uploaded;
  - `collide_radius`: `gridFor` takes it;
  - `center_strength`;
  - `velocity_decay`: a motion uniform;
  - `alpha_min`: `settled`;
  - `gravity`: a new WGSL term in the integrate kernel, `v += ((0 - x) * g) * alpha`, after
    collide and before the decay. It is skipped at `g = 0`, as `particle_mesh.rs:194-197` does.
- **Live inputs.** The FFT twiddles come from a TypeScript port of `fft.rs` `Plan::new`, in `f64`
  and narrowed once. The kernel spectrum comes from the existing device refresh
  (`gpu/charge-kernel.ts`) at the first `place`. The frame comes from `frameOf`.
- **Fallbacks.** Each sets `tier` and `reason`, and the next tick runs on the CPU session:
  - **No adapter:** as above.
  - **Device lost, or a validation error caught in the tick's scope:** the batch in flight is
    dropped. The session keeps the last written-back positions, so `a_lost_device_resumes_from_the_last_readback`
    is bit for bit. The session's velocities are zeroed through the velocity view.
    Caveat: the momentum of the GPU's last ticks is dropped, and it decays by `velocity_decay` per tick anyway.
- **The held read-back.** The f32 `x` read at the end of a tick is the next tick's start, which
  saves one of the three read-backs. It is 8 MB at 1M, and `release()` drops it. `force.ts`
  documents it, as condition 12 asks.
- **Caveat, the collide jiggle.** The GPU keys the jiggle from its own tick count, from 0 at
  `gpuMesh()`. The CPU keys it from `tick_no`, so only exactly coincident nodes see the difference.
- **Exports.** `index.ts` exports `GpuMesh`, `GpuMeshOptions`, `GpuTier`, `GpuMeshRefusedError`
  and `GPU_ENGINES`. The probes stay in `gpu.ts`, not exported.

### D. Studio

- **Setting.** `Settings.gpuForces: boolean`, default `false`. A settings document without the
  field reads `false`.
- **Action.** `forces.gpu` is a switch in the Forces section. When the page has no `navigator.gpu`,
  it is available-with-reason: aria-disabled, the reason "no WebGPU adapter in this browser"
  shown, never hidden.
- **Protocol.** A new ForceRequest `{ type: "force.gpu"; on: boolean }`, and an unsolicited Result
  `{ type: "force-tier"; tier; reason }` that the panel shows on its own line.
- **Engine.** When `gpuForces` is on, `planRun` picks `"particle_mesh"` at every size. Toggling
  restarts the live session the way a layout change does.
- **The loop becomes async-aware.**
  - `LiveForce.step` may return `Promise<number>`.
  - `ForceLoop` holds at most one step in flight: `wake`, `resume` and `deltas` do not schedule
    while one is in flight.
  - A step that resolves after `halt`, `pause` or a session swap publishes nothing and schedules
    nothing (a generation counter).
  - A rejected step releases the loop with the error's text.
  - The synchronous CPU path is unchanged byte for byte: same frames, same order.
- **The worker's port.** It wraps the motor session's `GpuMesh` as a `LiveForce`, and releases it
  (`await release()`) on `forget`/`renew`.

### E. Tests and rows (`scripts/orch/rows/gpu-g1d.rows`)

**Floor and motor rows.**
- The merge floor, wasm32 graph-core, `hashgate --seeds 8` with its negctl (the accessors are
  read-only, so no hash moves), and `capabilities --check`.

**SDK tests** (`sdk:typecheck`, `sdk:lint`, `sdk:test`):
- `a_gpu_engine_is_refused_by_create_force_session`
- `no_adapter_falls_back_to_the_cpu_mesh_and_says_so`
- `a_lost_device_resumes_from_the_last_readback`
- `a_barnes_hut_session_is_refused_a_gpu_mesh`
- `verbs_wait_for_the_next_tick`
- `the_session_refuses_a_second_driver`
- `release_hands_back_positions_and_velocities`
- `the_twiddles_are_the_motors`: TS port vs `mesh_probe()` twiddles, through a fixture.

**Studio tests** (`studio.sh check`):
- `the_toggle_off_is_the_default`
- `the_toggle_is_disabled_with_a_reason`
- `an_async_step_never_overlaps`
- `a_halt_during_an_async_step_publishes_nothing`
- `the_sync_loop_is_unchanged` (an existing loop test kept green unmodified)

**Browser twin gate** (hardware GPU, under `gpu.lock`):
- Two fresh mesh sessions over one graph built in the wasm motor (1k and 10k).
- `A.gpuMesh().tick(1)` vs `B.tick(1)`, `displacementRelRms` ≤ 1e-4.
- Run at the defaults, and at non-default parameters with `gravity > 0`, plus the other eight
  knobs moved.
- `negctl-gpu-params` (`GM_GPU_BREAK=params`: the device ignores `setParams`) must fail the
  non-default twin.

**Fallback and scale rows.**
- `negctl-gpu-no-adapter` (`GM_GPU_BREAK=adapter`) must exit non-zero on a row that requires tier `"gpu"`.
- `gpu-live-1m`: `GpuMesh.tick(1)` at 1M ≤ 100 ms median. The write-back cost is stated.
- A studio browser gate with the toggle on at 10k: tier `"gpu"` shown, frames move, no page error.
  Its negctl: no adapter, the panel says so, and CPU frames move.

## Rule on each. One line each: OK, or a condition the build must meet.

1. Is the amendment needed: can the live arm get the simple graph and the velocities without a
   graph-core or wasm change? Rule on these alternatives:
   - the SDK rebuilds `SimpleGraph` from host-supplied edges, a second generator;
   - `mesh_probe()` through wasm: about 48 MB transient at 1M and three passes;
   - starting the GPU at rest with no velocity export.
2. The three graph-core accessors and the host writing velocities through a view. Is a write
   through a view of a `&[f64]` acceptable, given the positions precedent?
3. The four exports at `ABI_VERSION` 2.
4. `GpuMesh` vs the plan's `startGpuMesh`, and whether `particle_mesh_gpu` as an always-refused
   `ForceEngine` value still earns its place.
5. The concurrency:
   - the session refusing `tick`/`grow` while a GPU mesh is live;
   - verbs staged to the next tick;
   - `ForceLoop`'s one-in-flight rule and its generation counter.
   Name every interleaving that breaks it.
6. The fallbacks:
   - zeroed velocities on a lost device;
   - "no adapter" as a resolved handle rather than a rejection;
   - a software adapter under `arm: "hardware"`.
7. The parameter plumbing: is any parameter unhonoured without a word? Does the twin gate's
   negctl catch a parameter the device ignores?
8. The gates: does each row have a negative control that fails? Is 1e-4 at one tick right for the
   live arm, where the spectrum is refreshed on the device, not loaded? See the caveat in
   `gpu/charge-kernel.ts:12-14`.

Score the four axes 1-5 (blast radius, reversibility, cost on failure, confidence) and name the
worst. Then give one verdict: PROCEED, PROCEED-WITH-CONDITIONS (numbered conditions, each
checkable by a command or a test name), or BLOCK (what to resolve).

Write it to `docs/decisions/gpu-g1d.md`:
- a title;
- `Status: G1d amendment 4 <verdict>, 2026-10-07`;
- the eight rulings as a table `| # | Question | Ruling | Evidence (path:line or command) |`;
- the scores;
- the conditions.

Nothing else.

You may run `git grep`, `sed -n` and read any file. Do not run builds, tests, benches or gates
other than the one below.

Paths you may touch: `docs/decisions/gpu-g1d.md`. Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-gpu-g1d-verdict scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS;
- `grep -E '^Status: G1d amendment 4 (PROCEED|PROCEED-WITH-CONDITIONS|BLOCK), 2026-10-07$' docs/decisions/gpu-g1d.md`
  prints one line.

Return: the branch tip, the verdict, each condition, and every deviation from this brief.
