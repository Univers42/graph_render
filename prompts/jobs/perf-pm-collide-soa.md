# Job perf-pm-collide-soa (agent build): fewer instructions per collide candidate and hit, same bytes

Why: at 1M nodes, 8 workers, the particle-mesh collide `Gather` is 33.5 ms of a 107 ms tick
(`docs/measurements/perf-pm-stencil.md:60`), the largest pass. A profile shows it bound by
instructions, not memory, and shows where they go. Cut them without moving one output byte.

Facts (verified on develop 08722f5a):

- **Profile.** `valgrind --tool=callgrind --branch-sim=yes --dump-instr=yes
  '--toggle-collect=*Gather*step_range*'` on `graph-cli tick --layout particle-mesh --n 100000 --warm 1
  --ticks 1 --seed 1 --workers 1` (image `ge-profile`, line tables): 267.2 M instructions for 100 000
  queries, so about 2 670 per query; about 53 candidates and 17.3 hits (`resolve` calls) per query.
  Per query, about 700 instructions go to the filter, 750 to the hit loop and `resolve`, and 350 to
  the cell change (`reads`, `point`, `fill`).
- **The filter** (`particle_mesh/collide/gather.rs:127-145` `overlaps`). The window is AoS
  (`at: [[f64; 2]; WINDOW]`, `:51`), so each candidate costs about 13 instructions:
  `movupd`, `subpd`, `mulpd`, then a horizontal `unpckhpd` and `addsd`, `ucomisd`, `seta`, the hit store.
  The two SSE lanes hold one candidate's x and y, not two candidates.
- **The hit loop** (`gather.rs:88-92`). It loads `self.slot[j]` for every hit. The closure
  `ids = || (grid.order[k], grid.order[q])` (`:90`) is built per hit, and the profile charges its line
  about 5 instructions per hit. `ids` is only needed for a jiggle (`dx == 0.0` or `dy == 0.0`).
- **`resolve`** (`particle_mesh/collide.rs:202-225`) re-tests `l.is_nan() || l >= c.d2` (`:209`). The
  filter has already decided this with the same expression on the same operands, `dx * dx + dy * dy`
  where `dx = px - qx`: a hit is never NaN and is `< d2`.
- **The term order is the contract.** Each slot sums its hits in window order: `Reads` runs in order,
  slots ascending within a run (`gather.rs:1-8`, `docs/measurements/perf-p3-gather.md` "Design"). Any
  change that reorders, regroups or fuses the additions `out.0 += dx * push` moves bytes. So does
  any FMA. Rust does not contract `a * b + c`; do not write `mul_add`.
- **Tests that pin it.** `collide/tests.rs:177` `the_filtered_gather_is_the_branched_one_bit_for_bit`
  (every slot, every worker count, a crowd with more than one window) and `:68`
  `the_grid_finds_every_overlap_the_pairwise_scan_finds`.
- **Cross-tree tools.** `~/goinfre/bench/pm-stencil/xtree.sh <worktree>` prints the PM snapshot
  sha256 at 8 node counts. `~/goinfre/bench/pm-stencil/parity.sh` and
  `~/goinfre/bench/pm-stencil/serial.wasm` (develop's PM, serial build) feed
  `harness/wasm-threads.mjs hash --serial`.
- **Limits.** `gather.rs` is 145 lines, `collide.rs` 249. Each function ≤ 40 lines, ≤ 4 parameters,
  nesting ≤ 3, each file ≤ 300 lines. No `unsafe`, no `std::arch`, no new dependency. graph-core must
  still build for `wasm32-unknown-unknown`.

Do, in order:

1. **Pin first.** Copy `xtree.sh` into `~/goinfre/bench/pm-collide-soa/`. Run it on the untouched
   worktree and keep the output as `xtree-base.out`.
2. **Lazy ids.** The hit loop passes `(k, q)` and the grid to the jiggle path. `grid.order` and the
   window's `slot` are read only inside the `dx == 0.0` / `dy == 0.0` branches. Jiggle keys stay
   `(order[k], order[q])` in that order.
3. **A hit-only push.** Split `resolve` into the overlap test and the push. `Gather` calls the push
   on filtered hits only and skips the re-test. The push keeps `resolve`'s arithmetic verbatim:
   `l = dx * dx + dy * dy`, the two jiggle adds, `dist = sqrt(l)`,
   `push = (c.reach - dist) / dist * 0.5`, then `out.0 += dx * push` and `out.1 += dy * push`.
   Keep the full `resolve` wherever it is still called; tests use it as the branched reference.
4. **An SoA window.** Replace `at: [[f64; 2]; WINDOW]` with `wx: [f64; WINDOW]` and
   `wy: [f64; WINDOW]`.
   - `fill` writes both.
   - The NaN mask on the querying slot's own place masks `wx[mine]`, which is enough: `dx` is then
     NaN and `l < d2` is false. Restore it after the loop, as today.
   - Write the filter two candidates per iteration (`j`, `j + 1`, plus a tail):
     - compute both lanes' `l` first, with the same expression;
     - then store `hits[found % WINDOW] = j`, `found += c0`, `hits[found % WINDOW] = j + 1`,
       `found += c1`.
     This lets LLVM pair the arithmetic across two candidates. Read the release asm of the filter
     (`objdump -d -l` on a line-tables build, as `scripts/orch/profile.sh` builds it). Report
     whether `subpd`/`mulpd`/`addpd`/`cmpltpd` now span two candidates, and the instruction count
     per candidate. If they do not, say so and keep the step only if step 7's rule holds.
5. **Optional, measured separately:** process the hits two at a time, so that the `sqrt` and the
   division of two hits can share one `sqrtpd`/`divpd`. Keep the scalar jiggle path for a pair
   where either lane jiggles, and the additions into `out` sequential in hit order. Keep it only if
   the asm shows the packed ops and step 7's rule holds on top of steps 2–4. Otherwise drop it and
   say why.
6. **Tests.**
   - The existing collide and `particle_mesh` tests pass unchanged.
   - Add one test: the split push equals `resolve` bit for bit over a grid of offsets that includes
     `dx == 0`, `dy == 0`, both zero, and `l` just under `d2`.
   - **Negative control** (run it, then revert it, and report the failing test's name): make the
     filter's second lane use `wx[j]` instead of `wx[j + 1]`.
     `the_filtered_gather_is_the_branched_one_bit_for_bit` must fail.
7. **Gates** (exit codes in the report):
   - `scripts/orch/gr cargo fmt --all --check` → 0
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` → 0
   - `scripts/orch/gr cargo test --workspace --no-fail-fast` → 0
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → 0;
     `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` → 1
   - `xtree.sh` on the branch: `diff xtree-base.out xtree-branch.out` → 0
   - this tree's threads wasm (`scripts/orch/wasm-threads.sh`), then
     `harness/wasm-threads.mjs hash --serial ~/goinfre/bench/pm-stencil/serial.wasm` → rc 0, every row
     `equal`; with `--break` → rc 1.
8. **Bench.** Run only when `free -g` shows ≥ 12 GB available and the 1-minute load is < 14; wait
   otherwise.
   - Command: `GR_MEM=12g scripts/orch/gr cargo run --release -q -p graph-cli -- tick --layout
     particle-mesh --n 1000000 --ticks 7 --workers 8 --passes`.
   - Arms: base (develop at the branch point, in a second worktree) and branch, alternated, 3 rounds
     each. Print the load per run.
   - Also take the callgrind count of step 1's profile command on the branch, to compare with
     267.2 M.
   - **Keep rule:** keep only if the 8-worker collide `Gather` median drops by ≥ 3 ms and the tick
     median does not rise. Otherwise revert the code, keep the new test and the report, and say
     "not kept".
9. **Report** `docs/measurements/perf-pm-collide-soa.md`, shaped like `perf-pm-stencil.md`. It holds:
   - what changed, per step, kept or dropped;
   - the asm finding;
   - instructions per query, base vs branch;
   - the gate table and the per-pass table (base vs branch, 8 workers, medians);
   - a `Caveat:` on host load, and that the bench's 7 ticks from the start layout are denser than a
     settled layout (more hits per candidate);
   - "what it does not do".

Paths you may edit: `crates/graph-core/src/layout/force/particle_mesh/collide.rs`,
`crates/graph-core/src/layout/force/particle_mesh/collide/{gather.rs,tests.rs}` and new files under
`particle_mesh/collide/`, `docs/measurements/perf-pm-collide-soa.md`, and anything under
`~/goinfre/bench/pm-collide-soa/`.
Do not touch `exec/`, `barnes_hut/` (read it only), `graph-wasm`, `graph-cli`, `packages/` or `app/`.

Done when every step-7 gate has its expected exit code, the negative control's failing test is named,
the bench table has three rounds per arm, and the report says kept or not kept against the step-8
rule.

Return block:

```
status: done | partial | blocked
kept: yes | no (Gather 8w base → branch ms, tick base → branch ms, medians; Ir/query base → branch)
steps: 2 <kept|dropped>, 3 <..>, 4 <..>, 5 <..>
changed: <files>
commands: <each gate> -> <rc>
deviations: <none | list>
decisions needed: <none | list>
```
