# Job gpu-g1c-collide (build: the particle mesh's collide pass in WGSL: hash, scan, scatter, resolve)

Why: the perf plan's GPU tier (1M nodes live in the browser). G1b put the charge pass on the device.
This slice does the same for the **collide** pass: WGSL kernels checked against the fixture's
`delta_collide` columns, the way G1b checks charge against `delta_charge`.

Read first, in full:
- the plan's Task 3, `docs/superpowers/plans/2026-10-06-gpu-g1.md:1209-1405`, in particular "Collide
  on the device";
- the ruling `docs/decisions/gpu-g1.md`, conditions 7-9. **Condition 9 binds you:** collide gets a
  control that is O(1) wrong, `collide-window`, and keeps `collide-order` besides;
- G1b's charge pass as the pattern to copy: `crates/graph-sdk-js/src/gpu/charge.ts`,
  `charge-api.ts`, `bounds.ts`, `readback.ts`, `pipelines.ts`, `buffers.ts`, `adapter.ts`;
- the CPU pass you transcribe: `collide_pass` (`crates/graph-core/src/layout/force/particle_mesh.rs:81-95`),
  `crates/graph-core/src/layout/force/particle_mesh/collide.rs` and `collide/` (hash, gather
  `WINDOW = 256`, the coincidence jiggle), and `motion::merge` with the grid's slot map;
- the fixture format, `fixtures/gpu/README.md`. The collide column is the pass **at tick 0**, even in
  the settled fixture (its Caveat at `:36-41`), so a jiggle that reads `tick_no` reads 0.

**Base.** Branch `gpu-g1c-collide` starts from `gpu-g1c`, which is G1b's tip; G1b is not on develop
yet. G1b's own red rows (its fault controls, its 1M guard) belong to G1b. Do not touch G1b's charge
files, apart from the additive export named below.

**A parallel slice.** `gpu-g1c-link` is built at the same time, on its own branch, from the same base.
Both code to this shared interface exactly, so the two merge without a conflict.
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
2. **The page dispatch is the link slice's.**
   - `deploy/perf/gpu-mesh.py --pass collide` makes the page import `/target/gpu-js/gpu/collide.js`
     and call its `runCollide(request, fault)`.
   - Do not edit `deploy/perf/`. Until the link slice's harness is merged into your branch (the
     orchestrator does that and tells you), check your kernels with the node tests below.
3. **Ceilings live in one file per pass**: yours is `gpu/bounds-collide.ts`.
   - It is keyed by `(arm, n, state)` like `bounds.ts`.
   - It reuses `compare()` from `bounds.ts` and never copies it.
   - A missing row means guard only.
4. **Faults are `--break <name>`**, passed as `runCollide`'s second argument and never as a request
   field (the reason is `charge-api.ts:1-13`). Your fault names start `collide-`.

Steps (TDD: each test is written and seen RED before its code; one commit per stage, so a control
bisects to one stage):
1. `collide_hash`: one invocation per node gives `(cell_of, bucket_of)` exactly as the CPU's hash
   does, and an `atomicAdd` on a `u32` count per bucket.
2. `collide_scan`: a three-dispatch Blelloch prefix sum over
   `buckets = max(4, (2n).next_power_of_two())`.
   - Each dispatch is `@workgroup_size(256)` and stays within 65 535 workgroups per dimension.
   - Node test `the_collide_scan_is_stable_at_every_bucket_count`: a JS reference prefix sum
     against a JS simulation of the three-dispatch version, at 4, 256, 65 536 and 2 097 152 buckets.
3. `collide_scatter`: invocation `i` handles node `i` and writes it in ascending node index, so
   each bucket's member list is sorted.
   - The check `order` reads the member lists back and confirms that each one is ascending.
     `exact.order` reports it, with the failure word `order`.
4. `collide_resolve`: one invocation per node walks its 3×3 neighbourhood. It sums the candidates
   in ascending order and pushes exactly as `collide.rs` does.
   - **The window.** The plan leaves the shape open (`:1340-1351`). The default is a fixed window
     of 256, the CPU's `WINDOW`, which is exact. Take it unless a measurement shows it cannot run.
   - Record the choice under "decisions taken". Its `Caveat:` line says which contacts it can lose.
5. `gpu/collide.ts` exports `runCollide(request, fault)`; `gpu/collide-api.ts` is a public
   `probeCollide(request)` with no fault parameter. Export `probeCollide` from
   `crates/graph-sdk-js/src/gpu.ts`. That export is your only edit to a G1b file.
6. `gpu/bounds-collide.ts`.
   - The guard is `rmsRel ≤ 1e-4` (the plan's table).
   - The ceilings are measured, rounded **up** to two significant digits.
   - Its `Caveat:` line: one device on one driver stack.
7. Faults:
   - `collide-order`: the scatter writes `n - 1 - i`. It must fail with `order`.
   - `collide-window`: the resolve drops the last populated candidate of each window. It must fail
     with `rms` or `guard`, an O(1) error (condition 9).
8. Node tests in `crates/graph-sdk-js/test/gpu-collide.test.mjs`:
   - `the_collide_scan_is_stable_at_every_bucket_count`;
   - `the_collide_hash_matches_the_cpu_s_cells`: positions and expected cells taken from the 1k
     fixture and the CPU's hash formula;
   - `every_collide_ceiling_sits_under_its_guard`.
   - G1b's `every_kernel_is_256_wide_and_f32` must cover your module. Register it in
     `gpu-wgsl.test.mjs` if that test lists its modules.
9. Browser measurement, once the orchestrator has merged the link slice's harness into your branch:
   - run the hardware arm and the software arm, `--pass collide --only 1k,10k,50k`, with the table
     empty (guards only);
   - write the measured rows;
   - run again, green;
   - run both controls.
   - If a guard is breached and `collide-window` is caught, stop and report both numbers.
   - The 1M row is not yours.
10. Append `## G1c — the collide pass, measured` to `docs/measurements/gpu-g1.md`:
    - the rows;
    - the window decision;
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
  - new files: `crates/graph-sdk-js/src/gpu/{collide,collide-api,bounds-collide,pass-report}.ts`,
    `crates/graph-sdk-js/src/gpu/kernels/collide.wgsl.ts` (split it into
    `collide-*.wgsl.ts` past 300 lines), `crates/graph-sdk-js/test/gpu-collide*.mjs`;
  - edits: `crates/graph-sdk-js/src/gpu.ts` (the export),
    `crates/graph-sdk-js/test/gpu-wgsl.test.mjs` (registration only), and an append to
    `docs/measurements/gpu-g1.md`.
  - Nothing in `deploy/`, `crates/graph-core`, `crates/graph-cli`, `packages/`, `app/` or `src/`.
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

Done when `scripts/orch/gate.sh target/rows-gpu-g1c-collide /home/dlesieur/Documents/graph_render/scripts/orch/rows/gpu-g1c-collide.rows`
writes a `summary.txt` with every row PASS, and the branch is committed and pushed. Before the link
harness is merged in, return `status: blocked`, naming the browser rows as the only ones left.

Return:
- `status: done` or `status: blocked` and the reason;
- the branch tip;
- the files with line counts;
- the RED run of each new test;
- the measured table;
- each row's result;
- decisions taken;
- every deviation.
