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
   `mesh-1m-start.gmfx` and `mesh-1m-settled.gmfx` are 105 592 232 B = 100.7 MiB each. The
   decided transport is `fetch` over the harness's own origin, stated in both the README and
   this document: at 100.7 MiB a base64 handoff is a 134 MiB string through `import()`, which
   is not a transport. Row `emit-1m`.

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
