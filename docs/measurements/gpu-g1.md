# GPU force tier G1 — what G1a wrote, and the numbers G1b is gated on

G1a is the slice that writes a byte: `graph-cli emit-gpu-fixtures` emits the particle mesh's
own per-pass velocity increments, at a start state and 100 ticks in, and
`--check` byte-compares the committed 1k pair against what this tree emits. No force law, no
WGSL, no browser code.

The design was ruled on before any of it was written and the ruling is
`docs/decisions/gpu-g1.md` — **G1a PROCEED-WITH-CONDITIONS, 2026-10-06**. Its conditions 1 to 6
are this slice's acceptance criteria and each is named below with what shows it.

## The six real file sizes against the plan's estimates

The plan's sizes came from `64 + 8 + 16m + 16n + 16P + 16P² + 48n` with `m ≈ 1.55n`. The
emitter printed every file's real byte count and its real `m`:

| file | `n` | `m` (real) | `P` | bytes (real) | plan's estimate | error |
|---|---:|---:|---:|---:|---:|---:|
| `mesh-1k-start.gmfx` | 1 000 | 1 532 | 128 | **352 776** | 353 000 | −0.06 % |
| `mesh-1k-settled.gmfx` | 1 000 | 1 532 | 128 | **352 776** | 353 000 | −0.06 % |
| `mesh-10k-start.gmfx` | 10 000 | 15 459 | 128 | **1 151 608** | 1 152 000 | −0.03 % |
| `mesh-10k-settled.gmfx` | 10 000 | 15 459 | 128 | **1 151 608** | 1 152 000 | −0.03 % |
| `mesh-50k-start.gmfx` | 50 000 | 77 462 | 256 | **5 492 136** | 5 493 000 | −0.02 % |
| `mesh-50k-settled.gmfx` | 50 000 | 77 462 | 256 | **5 492 136** | 5 493 000 | −0.02 % |
| `mesh-1m-start.gmfx` | 1 000 000 | 1 549 910 | 1024 | **105 592 232** | 105 594 000 | −0.002 % |
| `mesh-1m-settled.gmfx` | 1 000 000 | 1 549 910 | 1024 | **105 592 232** | 105 594 000 | −0.002 % |

All eight files: **225 177 504 B = 214.7 MiB**, against the plan's ≈ 219 MB for its six plus
the two 1M files the verdict added. **No file is within 2× of its estimate**, so the plan's
stop condition is not met and `m ≈ 1.55n` was not wrong.

`m/n` is 1.532, 1.546, 1.549 and 1.550 — the assumption holds to within a percent at every
size, and converges upward as the 5 % `note_link` extras amortise.

## The frames the emitter computed

Two per state, and the settled state's frame is **not** derived from the start one's: the frame
is recomputed from the positions every tick (`particle_mesh/mesh.rs:119-125`), so this table is
a measurement and not a prediction. The plan's start-state estimates are in the last column.

| case | `P` | `step` | `h` | origin | `cells` | `reach` | plan's `step` (est.) | plan's `h` (est.) |
|---|---:|---:|---:|---|---:|---:|---:|---:|
| 1k start | 128 | 14 | 11.314 | (−384.67, −384.67) | 69 | 46 | 14 | 11.314 |
| 1k settled | 128 | 18 | 22.627 | (−905.10, −972.98) | 88 | 23 | — | — |
| 10k start | 128 | 19 | 26.909 | (−1210.89, −1210.89) | 92 | 20 | 19 | 26.909 |
| 10k settled | 128 | 21 | 38.055 | (−1864.68, −1864.68) | 101 | 14 | — | — |
| 50k start | 256 | 19 | 26.909 | (−2690.87, −2690.87) | 202 | 20 | 19 | 26.909 |
| 50k settled | 256 | 19 | 26.909 | (−2744.69, −2933.05) | 217 | 20 | — | — |
| 1m start | 1024 | 19 | 26.909 | (−12001.27, −12001.27) | 894 | 20 | 19 | 26.909 |
| 1m settled | 1024 | 18 | 22.627 | (−9322.50, −10363.36) | 881 | 23 | — | — |

**Every start-state estimate in the plan was right**, including `cells` and `reach` to the
unit at 1k and 10k. The 1M row is the cross-check the plan asked for: `h = 26.909` with
`cells = 894` puts the mean density at `1e6 / 894² = 1.25`, which is the record's "a mean cell
density near 1 at `P = 1024`" (`gpu-force-tier.md:90`) reached from the other direction.

**The settled frames do not follow one rule, which is the point.** The 1k case *expanded* one
rung (`h` 11.314 → 22.627, `step` 14 → 18), the 1M case *contracted* one rung
(`h` 26.909 → 22.627, `step` 19 → 18), and the 50k case did not move at all (`step` 19 on
both). Two of the four settled frames sit on a different rung from their start and one does
not, so **a settled state's frame cannot be predicted from a start state's** — which is why the
fixture carries both files and the README says so. A reader must also not assume `settled`
means a different rung: at 50k the rung ladder is coarser than the contraction.

## The two Caveat lines

**Caveat: every delta column is from a probe copy at tick 0.** `ForceSession::mesh_probe` runs
each pass against a copy whose velocities start at zero and whose `alpha` is 1, and that copy
is at tick 0 whatever the session's own tick was (`session/fidelity.rs:26-29`). Collide reads
`tick_no` for its coincidence jiggle, so the **settled** fixture's collide column is the
collide pass *at tick 0*, not at tick 100. The header carries no tick number and no velocities,
so nothing downstream can mistake one for the other — but a measurement that reads the settled
file's collide column must say which clock it is on.

**Caveat: `settled` means "after 100 ticks", not "converged".** `SETTLE_TICKS` is 100, not the
frozen `TICKS = 112`, because 100 is the number every other measurement in this repo calls
settled (`mb_fidelity/measure.rs:19`) and a GPU number and a Barnes-Hut number have to land in
the same table to be comparable. A layout that has not converged by 100 ticks is still being
measured.

## The gate

`scripts/orch/gate.sh target/rows-gpu-g1a scripts/orch/rows/gpu-g1a.rows`, every row PASS:

| row | expect | what it shows |
|---|---|---|
| `fmt` | 0 | `cargo fmt --all --check` |
| `clippy` | 0 | `clippy --workspace --all-targets -D warnings`, no `#[allow]` anywhere new |
| `test` | 0 | the whole workspace, `--no-fail-fast` |
| `wasm32-core` | 0 | graph-core still builds for `wasm32-unknown-unknown` |
| `hashgate-8` | 0 | **the digests did not move** — a probe adds no arithmetic |
| `negctl-degree` | 0 | the hash gate's own control still goes red |
| `emit` | 0 | all eight files written to `target/gpu-fixtures`, sizes printed |
| `check` | 0 | the committed 1k pair is byte-equal to what this tree emits |
| `negctl-pass` | nonzero | swapping the charge column turns `check` red |
| `negctl-rung` | nonzero | moving the rung turns `check` red |
| `negctl-absent` | nonzero | a directory that is not there is exit 2, not a pass |
| `sdk-typecheck` | 0 | `tsc --noEmit` over the SDK, including `gpu/fixture.ts` |
| `gpu-fixture-loader` | 0 | four loader cases against the committed 1k pair |
| `negctl-loader-scale` | nonzero | the loader's scale control goes red |
| `pub-surface` | 0 | `mesh_probe.rs` names exactly `MeshProbe` and `mesh_probe` |
| `no-allow` | 0 | no `#[allow]` and no `#[doc(hidden)]` in any new file |
| `named-tests` | 0 | both new named tests are present |
| `emit-1m` | 0 | both 1M files exist and are non-empty |
| `no-source-names` | 0 | nothing under `crates/` names a data source |

All nineteen rows PASS. `target/rows-gpu-g1a/summary.txt`:

```
PASS fmt  PASS clippy  PASS test  PASS wasm32-core  PASS hashgate-8  PASS negctl-degree
PASS emit  PASS check  PASS negctl-pass  PASS negctl-rung  PASS negctl-absent
PASS sdk-typecheck  PASS gpu-fixture-loader  PASS negctl-loader-scale
PASS pub-surface  PASS no-allow  PASS named-tests  PASS emit-1m  PASS no-source-names
```

**On `no-source-names` and one file this slice does not own.** The row's first run was red on
`crates/graph-core/src/layout/force/session/tests/golden.rs:12-13`, a doc comment recording
how the 65 golden digests were regenerated (`Base commit: 8e8e93b`, `git archive …`). That
file last changed on 2026-09-30 (`5ae4210a`), six days before this job, and it is outside
this slice's path list, so it was red before the first commit here. The row on develop now
excludes that path explicitly, and the row passes with it excluded. **The exclusion is the
whole fix and nothing else changed**: every file this slice added or edited was already clean
and is still clean —

```
$ git grep -n -i -E '\bgit\b|commit|repositor|activitywatch' -- \
    crates/graph-core/src/layout/force crates/graph-cli/src/gpu_fixtures.rs \
    crates/graph-cli/src/gpu_fixtures crates/graph-sdk-js/src/gpu \
  | grep -v 'session/tests/golden.rs'
(no output)
```

The rewording of `golden.rs` is still owed by whoever owns it; it is a two-line change and it is
not this slice's to make.

## Conditions 1 to 6, each with what shows it

1. **One type and one method.** `git grep -h -E '^\s*pub (struct|enum|fn|type|const|trait) '`
   over `session/mesh_probe.rs` prints exactly two lines: `pub struct MeshProbe` and
   `pub fn mesh_probe`. `MeshPass` is `pub(in crate::layout::force)` on
   `particle_mesh::pass`; `Solved` is crate-internal in `particle_mesh/mesh.rs`;
   `PlacedFrame` is `#[cfg(test)]`. `force/mod.rs:34` re-exports `MeshProbe` beside the names
   already there and nothing else. Row `pub-surface`.
2. **No `#[allow]`, no `#[doc(hidden)]`.** Row `no-allow`, and `clippy` exits 0 with
   `-D warnings`. The one place a suppression would have been convenient — the `#[cfg(test)]`
   frame comparison helper — is a `#[cfg(test)]` item instead.
3. **The no-change property is a test.**
   `the_probe_leaves_the_next_tick_byte_identical` builds two identical mesh sessions, probes
   one, steps both by one and compares `xs`, `ys` and `tick_no` bit for bit.
   `a_delta_is_the_pass_and_not_the_tick` pins all five — `xs`, `ys`, `alpha`, `tick_no` and
   `params()` — which is what `fidelity.rs:51-53` already claimed. Row `named-tests`, and the
   two tests themselves.
4. **The hash gate does not move.** `hashgate --seeds 8` exits 0 with the same digests on all
   44 arms, and `negctl-degree` still exits 1. Rows `hashgate-8` and `negctl-degree`. Nothing in
   graph-core changed any arithmetic: the probe reads a solve the tick would have done anyway,
   and the one edit to `Mesh::solve` — dropping `self.frame` on the no-field path — moves no
   byte, because a solve that returns `false` has no reader.
5. **The README is normative-complete and the loader is version-strict.**
   `fixtures/gpu/README.md` gives all thirteen header offsets and all ten payload sections with
   every length derived from `n`, `m` and `P`, plus the shared scale table.
   `the_loader_refuses_a_version_it_does_not_know` feeds `gpu/fixture.ts` a major of 2 and
   requires a throw, and also pins that a newer *minor* is read rather than refused. Row
   `named-tests` and row `gpu-fixture-loader`.
6. **`SIZES` gains 1 000 000, and the base64 handoff is struck.** All eight files are emitted;
   `mesh-1m-start.gmfx` and `mesh-1m-settled.gmfx` are 105 592 232 B = 100.7 MiB each
   (`ls -l target/gpu-fixtures/`, confirmed by row `emit-1m`). The decided transport is
   `fetch` over the harness's own origin, stated in both the README and this document: at
   100.7 MiB a base64 handoff is a 134 MiB string through `import()`, which is not a
   transport. Row `emit-1m`.

## The two derivations the verdict found wrong, corrected

Both are now in `fixtures/gpu/README.md` and above.

**Condition 7 — the charge `maxAbs`.** The plan printed `√2 · 2⁻¹¹ · √1.25` beside its own
number; that formula is `7.7e-4`, not the `3e-4` the record states, off by 2.6×. The
derivation that produces the record's number is the quantum argument: four CIC weights, each
rounded with independent uniform error over one quantum, rms `2⁻¹¹/√12` apiece and four in
quadrature, giving **`2⁻¹¹/√3 = 2.8e-4`** of a unit per occupied cell. Carried through,
`90 · 2.8e-4 / 26.909² = 3.5e-5` ≈ the record's **`3.7e-5` units/tick**, unchanged. The `1e-4`
relative row says **1.5 orders of magnitude** under `3.6e-3`, not "one order" — the ratio is
36×.

**Condition 8 — the link ceiling.** The plan's `k · 2⁻²⁴` at `k = 4` gave `2.4e-7` and left
`5.6e-7` "for the one `sqrt` and one division", which is 3.0× below the ceiling it claimed to
cover. Two errors: an **`f32` ULP is 2⁻²³**, not 2⁻²⁴ (the latter is the unit roundoff), and
`k` is the *measured* maximum degree, not 4. Per term the cost is `(0.5 + 2 + 2.5) = 5` ULP, so
the ceiling is **`k_measured · 5 · 2⁻²³`**, with `k_measured` read from the fixture's own edge
columns.

## What this measurement does not establish

**The oracle here is a transcription check, not an algorithm check.** The comparison a GPU arm
will make is against columns that came out of the mesh's own solve, so a CPU-side error in the
twiddle table, the kernel spectrum, the `1/P²` prescale, the Green's function or the frame
ladder would be invisible to it: both arms would agree and both would be wrong. What covers
that is elsewhere and is not this slice — `perf-mb-fidelity` grades the mesh against the exact
all-pairs sum, and the 4-way hash gate pins the CPU mesh's bytes.

**One instrument, and it is partly self-referential.**
`every_column_is_the_meshes_own` compares the probe's charge column against `charge_deltas`,
which is the same instrument reached through a different path. It proves the two agree; it does
not prove either read the mesh. That is the cost-on-failure axis the verdict scored 3, and the
reviewer note stands: the independent check is `mb_fidelity`, run on these same fixtures.

**No adapter, no throughput, no f32 number.** G0 measured that an adapter exists
(`docs/measurements/gpu-adapter.md:30`) and nothing else. This slice measures no timing: the
emit at 1M is 100 ticks of a 1024-side mesh, which is a generator's cost and not a tick's.
`msPerTick` at 1M and `stressRatio` are G1c's, and the toggle stays off until they are
recorded.

**One edit to graph-core's own solve path, and it moves no byte.** `Mesh::solve` used to leave
`self.frame` set on the path where it returns `false`; it now clears it. A solve that returns
`false` has no reader — `charge::apply` returns before it reads a field — so the tick's bytes
are unchanged, and `hashgate --seeds 8` confirms it on all 44 arms. The edit is here because
`Mesh::solution`'s `None` has to mean *no field was solved* rather than *a frame was placed and
discarded*, which is the `None` the public signature promises. Without it the probe would hand
back a spectrum for a frame the convolution never ran on.

**The emitter's own schedule is `Serial`, one worker.** `mesh_probe` and `charge_deltas` share
`charge_deltas`'s `How`, which is what makes the two comparable bit for bit — the property
`every_column_is_the_meshes_own` rests on. It is also the slowest legal schedule, and the 1M
settle runs on it: 333 s for all eight files. That is a generator's cost, paid once per gate
run, and not a tick's.

## G1b — the charge pass, measured

The browser runs the particle mesh's charge pass — bounds, deposit, the FFT with the kernel, the
field read — and matches the CPU's own per-node velocity increments, the `.gmfx` fixtures G1a
landed. The gate is `docs/decisions/gpu-force-tier.md:74`: "rms bound, repeat equal,
broken-butterfly control red". The dispatch mirrors `mesh.rs:151-215` operation for operation:
bounds, deposit, then `Fft::forward` (`fft.rs:107-113`: rows over `cells`, columns over `side`)
and `Fft::inverse` (`fft.rs:119-126`: rows over `side` with the kernel multiply, columns over
`cells`), then the CIC read scaled by `charge·alpha`.

### The two arms

| arm | adapter | vendor/architecture | fallback |
|---|---|---|---|
| hardware | AMD RX 6600 (RADV) | amd/rdna-2 | false |
| software | SwiftShader | google/swiftshader | true |

### The measured ceilings

`bounds.ts` holds one table keyed by `(arm, n, state)`. Each row is the measured `rmsRel` and
`maxAbs` rounded **up** to two significant digits, so a later run of the same arm on the same
device sits at or under its own row. The arm is held to its own row.

| arm | n | state | rmsRel | maxAbs | rmsRef | depositedUnits | wallMs |
|---|---|---|---|---|---|---|---|
| hardware | 1000 | start | 1.4533e-07 | 7.62939e-05 | 99.3811 | 2097152000 | 13.4 |
| hardware | 1000 | settled | 3.1431e-07 | 3.43323e-05 | 21.4479 | 2097152000 | 13.9 |
| hardware | 10000 | start | 4.40041e-07 | 0.000152588 | 66.8159 | 1310720000 | 16.6 |
| hardware | 10000 | settled | 5.99767e-07 | 0.000106812 | 38.2461 | 1310720000 | 53.1 |
| hardware | 50000 | start | 2.31603e-06 | 0.000492096 | 45.0539 | 1638400000 | 23.7 |
| hardware | 50000 | settled | 2.25643e-06 | 0.000620037 | 53.8775 | 1638400000 | 21.7 |
| hardware | 1000000 | start | 7.48186e-05 | 0.00897522 | 21.3442 | 2048000000 | 237.2 |
| hardware | 1000000 | settled | 3.64338e-05 | 0.0235653 | 108.145 | 2048000000 | 196.7 |
| software | 1000 | start | 1.8763e-07 | 7.62939e-05 | 99.3811 | 2097152000 | 676.9 |
| software | 1000 | settled | 3.80306e-07 | 4.57764e-05 | 21.4479 | 2097152000 | 674.8 |
| software | 10000 | start | 4.5318e-07 | 0.000167847 | 66.8159 | 1310720000 | 678.0 |
| software | 10000 | settled | 6.02914e-07 | 0.000120163 | 38.2461 | 1310720000 | 656.2 |
| software | 50000 | start | 2.31441e-06 | 0.000495911 | 45.0539 | 1638400000 | 794.0 |
| software | 50000 | settled | 2.22571e-06 | 0.000598907 | 53.8775 | 1638400000 | 798.6 |

The 1M software rows were not run.

### The guards

Two guards, both derived, and a breach of either is a stop, not a re-tune.

- `rmsRel ≤ 1e-4` at every fixture. That is 1.5 orders of magnitude under the mesh's own best
  error against the exact all-pairs sum, `3.6e-3` rms.
- At the two 1M fixtures, the `maxAbs` guard. The old guard was
  `|charge| · (2⁻¹¹/√3) / h²` — `4.96e-5` settled, `3.50e-5` start — and the kernel breached it
  476× and 256×. Amendment 2 (`docs/decisions/gpu-g1.md`) re-derived it as
  `|charge·alpha| · (2⁻¹¹/√3) · ‖g‖₂ · 6`, with `‖g‖₂ = P · ‖spectrum‖₂` from the fixture's own
  spectrum section. The propagation factor is the kernel's 2-norm, not `1/h²` (about 100×
  smaller), and the peak factor `6` is `√(2·ln 2n)` = 5.39 at n = 1e6, rounded up. That gives
  `3.17e-2` settled (`‖g‖₂` 0.2085) and `2.61e-2` start (`‖g‖₂` 0.1712). The kernel sits under
  both, by 1.34× and 2.9×.

Caveat: these are one device's numbers on one driver stack. A driver update re-measures them; it
does not widen them. The per-arm rows are the decision: a software adapter's `f32` is the same
`f32`, but its reassociation and its transcendentals are not the hardware's, and the record's
claim is per-device repeatability, not cross-device equality.

### The controls

Each control must be caught by its own check (exit 3 and the named failure), not by a crash.

| control | fault | exit | named failure |
|---|---|---|---|
| negctl-butterfly | butterfly | 3 | rms |
| negctl-deposit | deposit | 3 | rms |
| negctl-repeat | repeat | 3 | repeat |
| negctl-weight | weight | 3 | deposit |
| negctl-bounds | bounds | 3 | bounds |
| negctl-no-device | (none — `GM_GPU_BREAK=1`) | 3 | refusal: software adapter |

## G1c — the link pass, measured

The link pass on the device (`crates/graph-sdk-js/src/gpu/link.ts`, kernel
`gpu/kernels/link.wgsl.ts`): one invocation per node sums its own share of every incident
simple edge, in ascending edge index, over a CSR the host builds once (`linkCsr`, the same
counting sort as `row_csr`). No atomics, no float scatter (D10). Graded against the fixture's
`delta_link` columns by `bounds.ts`'s `compare()`, held to `gpu/bounds-link.ts`. Measured on
2026-10-06 with `scripts/studio-probe.sh gpu-mesh <arm> target/gpu-fixtures --pass link --only
1k,10k,50k`, first with the ceiling table empty (guards only), then again with the rows below.

### The adapters

| arm | `vendor/architecture` | flag set that gave it | `isFallbackAdapter` (probe) |
|---|---|---|---|
| hardware | `amd/rdna-2` | set 1/4, `unsafe webgpu + vulkan` | false |
| software | `google/swiftshader` | set 1/4, `swiftshader webgpu + vulkan` | true |

### The measured rows

`rmsRel` and `maxAbs` as the guards-only run printed them; the ceiling is each rounded **up**
to two significant digits. Both runs passed every check, and both had `repeatEqual=True`.

| arm | `n` | state | `rmsRef` | `rmsRel` | `maxAbs` | ceiling `rmsRel` | ceiling `maxAbs` |
|---|---:|---:|---:|---:|---:|---:|---:|
| hardware | 1 000 | 0 | 17.4827 | 8.55892e-8 | 7.62939e-6 | 8.6e-8 | 7.7e-6 |
| hardware | 1 000 | 1 | 22.3892 | 1.00495e-7 | 1.14441e-5 | 1.1e-7 | 1.2e-5 |
| hardware | 10 000 | 0 | 73.2433 | 7.55265e-8 | 4.57764e-5 | 7.6e-8 | 4.6e-5 |
| hardware | 10 000 | 1 | 44.5554 | 9.81270e-8 | 2.28882e-5 | 9.9e-8 | 2.3e-5 |
| hardware | 50 000 | 0 | 172.786 | 7.48707e-8 | 1.22070e-4 | 7.5e-8 | 1.3e-4 |
| hardware | 50 000 | 1 | 67.6473 | 1.00948e-7 | 1.37329e-4 | 1.1e-7 | 1.4e-4 |
| software | 1 000 | 0 | 17.4827 | 8.13294e-8 | 7.62939e-6 | 8.2e-8 | 7.7e-6 |
| software | 1 000 | 1 | 22.3892 | 9.01399e-8 | 7.62939e-6 | 9.1e-8 | 7.7e-6 |
| software | 10 000 | 0 | 73.2433 | 7.27343e-8 | 4.57764e-5 | 7.3e-8 | 4.6e-5 |
| software | 10 000 | 1 | 44.5554 | 9.05203e-8 | 2.28882e-5 | 9.1e-8 | 2.3e-5 |
| software | 50 000 | 0 | 172.786 | 7.14735e-8 | 1.22070e-4 | 7.2e-8 | 1.3e-4 |
| software | 50 000 | 1 | 67.6473 | 9.43418e-8 | 1.37329e-4 | 9.5e-8 | 1.4e-4 |

The `maxAbs` values are single `f32` steps at the reference's magnitude (`7.62939e-6` is
`2⁻¹⁷`, `1.22070e-4` is `2⁻¹³`): one rounding of one component, not an accumulated drift. The
1M row is not here: the orchestrator measures it alone.

### The guards, with their arithmetic

`rmsRel ≤ k_measured · 5 · 2⁻²³` (condition 8): 5 ULP per term (a subtraction 0.5, `sqrt` 2,
`x/y` 2.5), an `f32` ULP of `2⁻²³` (`2⁻²⁴` is the unit roundoff), `k` terms per node, and
`k_measured = max degree + 1` read from each fixture's own `edge_lo`/`edge_hi` by
`measuredK`. Both states of one `n` share a topology, so they share `k`.

| `n` | max degree | `k_measured` | guard `k · 5 · 2⁻²³` | worst measured `rmsRel` | headroom |
|---:|---:|---:|---:|---:|---:|
| 1 000 | 39 | 40 | 2.384e-5 | 1.00495e-7 | 237× |
| 10 000 | 90 | 91 | 5.424e-5 | 9.81270e-8 | 553× |
| 50 000 | 113 | 114 | 6.795e-5 | 1.00948e-7 | 673× |

**No guard was breached.** The guard is a per-node worst case applied to an aggregate, so the
headroom is expected and is not a reason to tighten it: the measured ceiling is what holds the
arm to its own numbers, and the guard is what a re-measure may never exceed.

**No pair is coincident in `f32`.** The kernel gives a pair whose two `f32` positions are equal
on both axes no force, where the CPU's `jiggle` gives it one. A one-off host count of
`fround(x[hi]) - fround(x[lo])` and the same for `y` over all six fixtures found no edge with
either axis at zero, so the jiggle branch is never reached here; a fixture where it was would
fail `guard`, because one such edge moves two nodes by `O(60)`.

### The controls

| control | command | exit | line |
|---|---|---:|---|
| `link-bias` | `--pass link --only 1k --break link-bias`, hardware | **3** | `FAIL mesh-1k-start guard … rmsRel 0.8527938659570882 over k=40 · 5 · 2⁻²³ = 0.0000238… rms … max (… 105.15 over … 0.0000077)`; settled: `rmsRel 0.9323…`, `maxAbs 98.68` |

`link-bias` swaps each edge's two weights in the kernel — the higher end takes `-(1 - b)` and the
lower `b` — so every edge whose ends have unequal degrees moves its nodes by the wrong share:
an `O(1)` mismatch, `rmsRel ≈ 0.85` against a guard of `2.4e-5`. All three of `guard`, `rms`
and `max` name it.

### What this does not establish

The comparison is a transcription check, as charge's is (condition 6): the per-edge constants
(`distance`, `strength`, the bias) are computed on the host from the frozen parameters
(`params.rs:69-70`) and the fixture's own strengths, the way `edge_geometry` computes them, so
a CPU-side error in those would be shared by both arms. What the GPU arm is checked for is the
gather — the CSR, the row order, the per-edge force, the share and the sum.

## G1c — the collide pass, measured

The collide pass on the device (`crates/graph-sdk-js/src/gpu/collide.ts`, kernels
`gpu/kernels/collide-*.wgsl.ts`): a counting sort of the nodes into a hashed cell list one
diameter wide (`collide_hash`, `collide_scan`, `collide_scatter`), then a gather over each node's
nine neighbour cells (`collide_resolve`), one invocation per node. Graded against the fixture's
`delta_collide` columns by `bounds.ts`'s `compare()`, held to `gpu/bounds-collide.ts`. Measured on
2026-10-06 with `scripts/studio-probe.sh gpu-mesh <arm> target/gpu-fixtures --pass collide --only
1k,10k,50k`. All six fixtures pass on both arms; the ceilings below are written.

### The guard: mixed, absolute plus relative (Amendment 1)

The plan's guard, `rmsRel ≤ 1e-4`, failed on `mesh-1k-settled` on both arms (`rmsRel ≈ 5.4e-4`).
The failure is inherent `f32` noise, not a defect: a JS `f32` reference that mirrors the kernel op
for op reproduces the device's numbers (`rmsRel 5.379e-4`, `maxAbs 2.637e-5` against the device's
`5.406e-4` and `2.637e-5`). The settled 1k fixture's reference is tiny (`rmsRef = 0.0072`, against
`0.67` to `172` for the other five): at equilibrium the collide forces nearly cancel, so the net
delta is near zero while the individual pushes are `O(30)`. The `f32` rounding scales with the
pushes, not with their net, so a relative-only guard is a bound on a near-zero denominator.

The guard is re-derived (Amendment 1) as the Higham bound for the sum of `k_c` pushes:

`rmsAbs ≤ 1e-4 · rmsRef + k_c · 5 · 2⁻²³ · P`, relative form `1e-4 + k_c · 5 · 2⁻²³ · P / rmsRef`.

- `k_c` is the largest number of contacts of any one node (pairs closer than `reach`, the CPU's
  `d2` test), counted in `f64` on the host from the fixture's own positions, with the grid the
  device sorts with.
- `P` bounds one push: `(reach − dist) · 0.5` is at most `reach / 2 = collide_radius`
  (`collide.rs:254-256`). The `delta_collide` column carries no strength — `collide_pass` merges
  the push straight through `motion::merge` (`particle_mesh.rs:85-95`), and the only collide
  parameter is `collide_radius` — so `P = reach / 2 = 16`.
- `5 · 2⁻²³` is condition 8's per-term rounding (a product 0.5, `sqrt` 2, a division 2.5 ULP; an
  `f32` ULP is `2⁻²³`).

| fixture | `k_c` | `P` | `rmsRef` | guard |
|---|---:|---:|---:|---:|
| 1k-start | 8 | 16 | 1.72612 | 1.44e-4 |
| 1k-settled | 4 | 16 | 0.007189 | 5.41e-3 |
| 10k-start | 8 | 16 | 1.00834 | 1.76e-4 |
| 10k-settled | 9 | 16 | 2.6824 | 1.32e-4 |
| 50k-start | 8 | 16 | 0.67192 | 2.14e-4 |
| 50k-settled | 23 | 16 | 9.91163 | 1.22e-4 |

**Caveat: `k_c` is measured on the fixture's positions. A denser crowd raises it, so the guard is
only as good as the fixture's measured crowd, the same limit condition 8 states for link.**

### The adapters

| arm | `vendor/architecture` | flag set that gave it | `isFallbackAdapter` (probe) |
|---|---|---|---|
| hardware | `amd/rdna-2` | set 1/4, `unsafe webgpu + vulkan` | false |
| software | `google/swiftshader` | set 1/4, `swiftshader webgpu + vulkan` | true |

### The measured rows, with their ceilings

`rmsRel` and `maxAbs` as the run printed them; the ceiling is the measured value rounded **up** to
two significant digits, written in `bounds-collide.ts`. Every measured value sits under its ceiling
and its guard.

| arm | `n` | state | `rmsRef` | `rmsRel` | `maxAbs` | ceiling `rmsRel` | ceiling `maxAbs` | verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| hardware | 1 000 | 0 | 1.72612 | 4.0081e-06 | 3.2157e-05 | 4.1e-06 | 3.3e-05 | PASS |
| hardware | 1 000 | 1 | 0.007189 | 5.4055e-04 | 2.6373e-05 | 5.5e-04 | 2.7e-05 | PASS |
| hardware | 10 000 | 0 | 1.00834 | 2.1390e-05 | 1.2209e-04 | 2.2e-05 | 1.3e-04 | PASS |
| hardware | 10 000 | 1 | 2.6824 | 6.8592e-06 | 1.1373e-04 | 6.9e-06 | 1.2e-04 | PASS |
| hardware | 50 000 | 0 | 0.67192 | 8.0766e-05 | 3.8686e-04 | 8.1e-05 | 3.9e-04 | PASS |
| hardware | 50 000 | 1 | 9.91163 | 7.3358e-06 | 4.6638e-03 | 7.4e-06 | 4.7e-03 | PASS |
| software | 1 000 | 0 | 1.72612 | 4.0468e-06 | 3.1978e-05 | 4.1e-06 | 3.2e-05 | PASS |
| software | 1 000 | 1 | 0.007189 | 5.3788e-04 | 2.6374e-05 | 5.4e-04 | 2.7e-05 | PASS |
| software | 10 000 | 0 | 1.00834 | 2.1392e-05 | 1.2209e-04 | 2.2e-05 | 1.3e-04 | PASS |
| software | 10 000 | 1 | 2.6824 | 6.8621e-06 | 1.1444e-04 | 6.9e-06 | 1.2e-04 | PASS |
| software | 50 000 | 0 | 0.67192 | 8.0772e-05 | 3.8722e-04 | 8.1e-05 | 3.9e-04 | PASS |
| software | 50 000 | 1 | 9.91163 | 7.3364e-06 | 4.6634e-03 | 7.4e-06 | 4.7e-03 | PASS |

Both arms had `repeatEqual=True` and `exact.order=True` on every fixture.

### The window: the CPU's fixed 256, exact

The plan leaves the window's shape open (`:1340-1351`). This build uses the first shape: the
candidates are visited a window of `WINDOW = 256` at a time (`gather.rs:18`), every window, so no
contact is dropped and no truncation count is needed. The device keeps no copy of a window — the
sequence is read in place and the window is only its chunking. **Caveat: the window loses no contact.**
The contacts it can lose or gain against the CPU are the pairs whose `f32` distance rounds across
the diameter; the push of such a pair is near zero, so the loss is near zero too. Its cost is a
crowd's whole population per query: the measured fixtures read at most 55 candidates per node
(50k, settled), and a dense overlap that put thousands in nine buckets would run as long per
invocation as the CPU's own quadratic case.

### The controls

| control | command | exit | line |
|---|---|---:|---|
| `collide-order` | `--pass collide --only 1k --break collide-order`, hardware | **3** | `FAIL mesh-1k-start order … bucket 8 is not ascending at slot 1: node 981 then 926`; settled: `bucket 113 … node 861 then 242` |
| `collide-window` | `--pass collide --only 1k --break collide-window`, hardware | **3** | `FAIL mesh-1k-start guard … rmsRel 0.5787 over 1e-4 + k_c=8·5·2⁻²³·16/1.726 = 0.000144 … maxAbs 5.898`; settled: `rmsRel 0.6241 over 1e-4 + k_c=4·5·2⁻²³·16/0.00719 = 0.00541` |

`collide-order` ranks node `i` as `n - 1 - i`, so every member list comes out descending and the
`order` check names it — the check no `f32` bound can make (plan `:1333-1338`). `collide-window`
drops the last populated candidate of each 256-window, an `O(1)` error: `rmsRel ≈ 0.6` against the
new mixed guard (`5.4e-3` settled, `1.4e-4` start), caught by `guard` (condition 9). Both controls
are caught on both 1k fixtures, the window one by about `115×` and `4000×`.

### What this does not establish

The comparison is a transcription check, as charge's and link's are (condition 6): the grid (radius
16, cells 32 wide, the origin the finite positions' minimum) is derived on the host from the frozen
parameters, so a CPU-side error in it would be shared by both arms. What the GPU arm is checked for
is the hash, the scan, the scatter's order, the reads, the push and the sum. The old `1e-4` guard's
breach on `mesh-1k-settled` was not a defect in that transcription — the `f32` reference reproduces
it — it was the relative-only guard being too tight for a fixture whose reference is near zero; the
mixed guard (Amendment 1) absorbs it, and all six fixtures pass on both arms.
