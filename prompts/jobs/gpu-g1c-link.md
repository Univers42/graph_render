# Job gpu-g1c-link (build: the particle mesh's link pass in WGSL, and the harness's `--pass` switch)

Why: the perf plan's GPU tier (1M nodes live in the browser). G1b put the charge pass on the device.
This slice does the same for the **link** pass: a WGSL kernel checked against the fixture's
`delta_link` columns, the way G1b checks charge against `delta_charge`.

Read first, in full:
- the plan's Task 3, `docs/superpowers/plans/2026-10-06-gpu-g1.md:1209-1405`;
- the ruling `docs/decisions/gpu-g1.md`, conditions 7-9. **Condition 8 replaces the plan's link
  ceiling:** the guard is `k_measured · 5 · 2⁻²³`, where `k_measured` is the maximum degree read from
  the fixture's own edge columns. `8e-7` and `k = 4` are wrong; never write them;
- G1b's charge pass as the pattern to copy: `crates/graph-sdk-js/src/gpu/charge.ts`,
  `charge-api.ts`, `bounds.ts`, `readback.ts`, `pipelines.ts`, `buffers.ts`, `adapter.ts`, and
  `deploy/perf/gpu-mesh.py` with `deploy/perf/gpu_mesh_page.py`;
- the CPU pass you transcribe: `link_pass` (`crates/graph-core/src/layout/force/particle_mesh.rs:69-79`),
  `link::pass_with` in `crates/graph-core/src/layout/force/barnes_hut/link.rs`, and
  `motion::merge`;
- the fixture format, `fixtures/gpu/README.md`. The deltas come from rest at `alpha = 1`, on a copy
  at tick 0.

**Base.** Branch `gpu-g1c-link` starts from `gpu-g1c`, which is G1b's tip; G1b is not on develop yet.
G1b's own red rows (its fault controls, its 1M guard) belong to G1b. Do not touch G1b's charge files,
apart from the additive edits named below.

**A parallel slice.** `gpu-g1c-collide` is built at the same time, on its own branch, from the same
base. Both slices code to this shared interface exactly, so the two merge without a conflict.
1. `crates/graph-sdk-js/src/gpu/pass-report.ts`. Both slices create it with exactly this content,
   byte for byte:
   ```ts
   /**
    * `pass-report.ts` — the verdict of one non-charge pass (link, collide) against its fixture
    * column, the shape the harness prints. Charge keeps its own `ChargeReport`, whose deposit and
    * bounds checks no other pass has.
    */

   /** A pass the per-pass probe runs besides charge. */
   export type PassKind = "link" | "collide";

   /** One case's verdict. `pass` is false when any guard, ceiling or exactness check failed. */
   export interface PassReport {
     readonly kind: PassKind;
     readonly n: number;
     readonly state: 0 | 1;
     readonly rmsAbs: number;
     readonly rmsRef: number;
     readonly rmsRel: number;
     readonly maxAbs: number;
     /** Two runs on one device, byte for byte. */
     readonly repeatEqual: boolean;
     /** The pass's own exactness checks by name, each true when it held (collide: `order`). */
     readonly exact: Readonly<Record<string, boolean>>;
     readonly pass: boolean;
     readonly failures: readonly string[];
     /** `vendor/architecture` as the browser reported them. */
     readonly marks: string;
     readonly fallback: boolean;
   }
   ```
2. **The page dispatch is yours to write.**
   - `deploy/perf/gpu-mesh.py` takes `--pass charge|link|collide`, default `charge`. Today's
     charge output stays byte for byte the same.
   - The page imports `/target/gpu-js/gpu/<pass>.js` and calls its `run<Pass>(request, fault)`
     export: `runCharge`, `runLink`, `runCollide`. The collide slice adds its module and nothing
     else to the harness.
   - `line()` prints every field of a `PassReport` and of a `ChargeReport`.
   - `gpu-mesh.py` is 299 lines today. Move code into a new module (for example
     `deploy/perf/gpu_mesh_args.py`) so every file stays at most 300 lines.
3. **Ceilings live in one file per pass**: `gpu/bounds-link.ts`, `gpu/bounds-collide.ts`.
   - Each is keyed by `(arm, n, state)` like `bounds.ts`.
   - Each reuses `compare()` from `bounds.ts` and never copies it. `bounds.ts` is 243 lines, so
     another table there would pass 300.
   - A missing row means guard only. This is how the slice runs once before anything is measured.
4. **Faults are `--break <name>`**, passed as `run<Pass>`'s second argument and never as a request
   field (the reason is `charge-api.ts:1-13`). The link fault names start `link-`, the collide
   ones `collide-`.

Steps (TDD: each test is written and seen RED before its code):
1. `gpu/kernels/link.wgsl.ts` and `gpu/link.ts`, exporting `runLink(request, fault)`.
   - The per-edge force is in the CPU's terms (`barnes_hut/link.rs`), merged per node the way
     `motion::merge` merges it.
   - Use the gather form (D10): build a CSR of each node's incident edges on the host, once. Then
     one invocation per node sums its own edges in ascending CSR order. No atomics, no float
     scatter.
   - `@workgroup_size(256)`, f32 only. G1b's test `every_kernel_is_256_wide_and_f32`
     (`crates/graph-sdk-js/test/gpu-wgsl.test.mjs`) must cover the new module. Register it if
     the test lists its modules.
2. `gpu/link-api.ts`: a public `probeLink(request)` with no fault parameter, as `charge-api.ts`
   does. Export it from `crates/graph-sdk-js/src/gpu.ts`. This export and `gpu.ts`'s module doc
   are the only edits to G1b files apart from the harness.
3. `gpu/bounds-link.ts`.
   - The guard is `k_measured · 5 · 2⁻²³`, with `k_measured = max degree + 1` read from the
     fixture's `edgeLo`/`edgeHi`.
   - Its doc says that an f32 ULP is `2⁻²³` and that `2⁻²⁴` is the unit roundoff (condition 8).
   - Write a `Caveat:` line: the degree tail of the preferential-attachment model has no closed
     form, so the guard is only as good as the fixture's measured `k`.
4. The checks in a link report:
   - `rms`, `max` (from its ceiling row);
   - `guard`;
   - `repeat`: two runs, byte-equal.
   - Failure words: `rms`, `max`, `guard`, `repeat`.
5. Fault `link-bias`: swap each edge's source and target bias. The mismatch is O(1), so `rms` or
   `guard` must catch it. Under it, `gpu-mesh.py` must exit 3 with a `FAIL ... rms` or
   `FAIL ... guard` line.
6. Node tests in `crates/graph-sdk-js/test/gpu-link.test.mjs`:
   - `the_link_guard_reads_the_measured_degree`: a toy fixture whose maximum degree is 6 gets the
     guard `7 · 5 · 2⁻²³`;
   - `every_link_ceiling_sits_under_its_guard`;
   - `the_link_csr_lists_each_node_s_edges_in_ascending_order`.
7. Measure:
   - run the hardware arm and the software arm, `--only 1k,10k,50k`, with the table empty (guards
     only);
   - write the measured rows, rounded **up** to two significant digits;
   - run again, green.
   - If a guard is breached and `link-bias` is caught, stop and report both numbers.
   - The 1M row is not yours; the orchestrator measures it alone.
8. Append `## G1c — the link pass, measured` to `docs/measurements/gpu-g1.md`:
   - the measured rows;
   - each guard with its arithmetic and its `k_measured`;
   - the adapter strings;
   - each control's exit.

Rules:
- Toolchain: only the wrappers (`scripts/orch/node-slim.sh`, `scripts/orch/gr`,
  `scripts/studio-probe.sh`, `scripts/studio.sh`). Never a bare `node`, `npm`, `cargo` or
  `docker run`.
- **GPU safety.** The host froze on a GPU hang on 2026-10-02.
  - Wrap every `GM_GPU=1` run in `flock -w 3600 ~/goinfre/orch/queue/gpu.lock`.
  - Before each run, check that `free -g` shows at least 12 GB available.
  - Never run a 1M fixture.
  - Software-arm runs need no lock.
- Paths you may touch:
  - new files: `crates/graph-sdk-js/src/gpu/{link,link-api,bounds-link,pass-report}.ts`,
    `crates/graph-sdk-js/src/gpu/kernels/link.wgsl.ts`, `crates/graph-sdk-js/test/gpu-link*.mjs`,
    `deploy/perf/gpu_mesh_*.py`;
  - edits: `crates/graph-sdk-js/src/gpu.ts`, `deploy/perf/gpu-mesh.py`,
    `deploy/perf/gpu_mesh_page.py`, `crates/graph-sdk-js/test/gpu-wgsl.test.mjs` (registration
    only), and an append to `docs/measurements/gpu-g1.md`.
  - Nothing in `crates/graph-core`, `crates/graph-cli`, `packages/`, `app/` or `src/`.
  - Nothing exported from `crates/graph-sdk-js/src/index.ts`.
- House limits:
  - each file at most 300 lines, each function at most 40, nesting at most 3;
  - no type assertions (`as X`, `as unknown`, `: any`, `eslint-disable`);
  - no `f16`;
  - every heuristic carries a `Caveat:` line.
- Git:
  - commit in this worktree only, as `git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -m updated`,
    with no trailer;
  - push only this branch (`git push -q origin HEAD`);
  - never merge, rebase, or touch develop or main.

Done when `scripts/orch/gate.sh target/rows-gpu-g1c-link /home/dlesieur/Documents/graph_render/scripts/orch/rows/gpu-g1c-link.rows`
writes a `summary.txt` with every row PASS, and the branch is committed and pushed.

Return:
- `status: done` or `status: blocked` and the reason;
- the branch tip;
- the files with line counts;
- the RED run of each new test;
- the measured table;
- each row's result;
- decisions taken;
- every deviation.
