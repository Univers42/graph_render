# Job sg-fa2-forcesim (agent build, SciGraphs conformance: FORCEATLAS2 through ForceSim)

Read `prompts/jobs/sg-common.md` first, then `docs/measurements/sg-fa2-seed.md` (this branch starts
from `sg-fa2-seed`; that file is the proof this job rests on). Row: `FORCEATLAS2` (cause `rng`,
median 2.406e-1, 0 bitwise of 1020).

Why: the reference arm does not run networkx. `forceatlas.py:167` always takes
`_forceatlas2_forcesim` (`forceatlas.py:92-147`), which runs SciGraphs' own `ForceSim`
(`simulation.py:230-1094`, `model='FA2'`). `layout.forceatlas2` is a networkx 3.6 port gated by
`oracle-fa2` (worst 3.156e-8 over 1000 seeds) and **stays exactly as it is**. This job adds a second
layout, `layout.forceatlas2.forcesim`, and points the conformance row at it.

References (read-only, all on disk):
- SciGraphs: `SciGraphs/core/scigraphs_core/mesh/layouts/{forceatlas,simulation,common}.py`.
- numpy 2.3.3 `default_rng`: `$GM_SCRATCH/refs/numpy-2.3.3/` holds `bit_generator.pyx`
  (`SeedSequence`, `hashmix`, `generate_state`), `_pcg64.pyx` (how the state words seed PCG64),
  `pcg64.h`/`pcg64.c` (the 128-bit LCG, `next64`), `distributions.c` (`next_double`:
  `(next_uint64 >> 11) * (1.0 / 9007199254740992.0)`), `_generator.pyx` (`random`).
  Missing directory: run `scripts/orch/fetch-refs.sh`; still missing is a stop.
- The oracle image `ge-python-oracle` (numpy 2.3.3) for test vectors. Vectors are pasted into
  tests as literals; the port is written from the sources above, never fitted to vectors.

The differences to port, from the measurement's table (re-verify each against the source):

| | ForceSim | anchor |
|---|---|---|
| start | `(default_rng(seed).random((n, 3)) - 0.5) * 5.0`, then f32 | `simulation.py:1088-1094` |
| seed | `RandomState(981798123).randint(0, 2**31 - 1)` = 1767573729 | `forceatlas.py:122` |
| state | f32 (`DTYPE = np.float32`) | `simulation.py:12` |
| move cap | `k = scale / max(cbrt(max(n, 1)), 1)` | `simulation.py:376`, `_integrate` `:735-766` |
| gravity | `gravity * 0.1` | `forceatlas.py:27,133` |
| forces | `_repulsion_direct`, `_attraction`, `_gravity`, `_force_field` | `:527`, `:694`, `:726`, `:1000` |
| steps | `sim.step(iterations)` | `:1056`, `forceatlas.py:141` |
| output | `_fa2_rescale(positions as f64, scale)` | `forceatlas.py:47-56,147` |

Do:
1. Measure (sg-common step 1). Paste the FORCEATLAS2 metrics line.
2. **PCG64 in `crates/graph-core/src/rng.rs`** beside `Mt19937`: `SeedSequence` for one `u32`
   entropy word, `generate_state(4)` as `u64`, the PCG64 seeding of `_pcg64.pyx`, `next_u64`,
   `next_f64`. `u128` arithmetic is allowed (it is integer, D1 holds on wasm32). RED first: a test
   that `default_rng(1767573729).random(6)` equals the oracle's six `f64` bit for bit; then the
   start rows the measurement pasted (`row0 = [1.4719001, 0.8315206, 0.57074285]` as f32).
3. **The kernel**, `crates/graph-core/src/layout/force/forcesim/` (child modules, house limits):
   only the path the fixtures take, `model='FA2'`, `repulsion_mode` direct (every fixture is under
   400 nodes; `_fa2_report_barnes_hut` says so). Larger `n` takes the same direct path, with a
   `Ponytail:` line naming the reference's TREE/GRID switch it does not port. f32 state, f32
   arithmetic **where numpy computes in f32**: under NEP 50 a Python `float` is weak (stays f32)
   but an `np.float64` scalar such as `self.k` (from `np.cbrt`) promotes the array to f64. Trace
   the dtype of every expression you port and write it in a comment where it changes. Reductions:
   name which numpy reduction (`sum`, `einsum`, `np.add.at`, `bincount`) each loop reproduces and
   its order (numpy's pairwise summation is not a sequential loop: record any reduction you could
   not reproduce as the row's residual cause, do not reorder sums by guesswork).
4. **Register** `layout.forceatlas2.forcesim` in `registry.rs` (additive edit only; every
   `Metadata` field filled; oracle = SciGraphs `ForceSim` via `scigraphs-conformance`). Its params:
   the ones `_forceatlas2_forcesim` takes, defaults from `forceatlas.py` and the dispatcher, `dim`
   3. `_fa2_rescale` is applied inside the layout, since the reference returns rescaled points.
5. **The conformance arm** (`crates/graph-cli/src/oracle_python/conformance/motor.rs`): FORCEATLAS2
   runs the new id. Re-measure; re-pin only FORCEATLAS2's `row(...)` from the proposed baseline
   (sg-common step 4); `--break` exits 1. Close `G_FORCEATLAS2_SEED` in `rows.rs`/`gaps.rs` if the
   start now matches; leave `G_SNAPSHOT_SCALE` alone.
6. **The transcript.** `harness/scigraphs-conformance/sc_reference.py:48,51` captures the tier's
   stdout and drops it. Write it into `ref/<NAME>.json` as a `transcript` field (list of lines),
   so the next reader sees `Computing ForceAtlas2 (3D, ForceSim)` without re-running anything.
7. `capabilities --check`, `codegen --check` if the registry change touches generated files,
   `hashgate --seeds 8` (exit 0) and its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero),
   `oracle-fa2` still green for `layout.forceatlas2` (its numbers must not move).

Stop and report under "decisions needed" if: the start rows do not match after step 2 (the seed
chain is then wrong, not the kernel); a fixture takes a non-direct path; or any row other than
FORCEATLAS2 moves.

Paths: `crates/graph-core/src/rng.rs` (or `rng/pcg64.rs` if it would pass 300 lines),
`crates/graph-core/src/layout/force/forcesim/**`, `crates/graph-core/src/registry.rs` (additive),
`crates/graph-cli/src/oracle_python/conformance/{motor.rs,rows.rs,gaps.rs,baseline/table/basic.rs}`,
`harness/scigraphs-conformance/sc_reference.py`, `docs/measurements/sg-fa2-forcesim.md` (before
and after rows, the dtype trace, the residual cause).

Done when: sg-common's done-when; FORCEATLAS2's median is below 1e-3 or the measurement names the
one reduction that keeps it above; the new layout is in `capabilities` with every field; the
hashgate pair and `oracle-fa2` are pasted.
