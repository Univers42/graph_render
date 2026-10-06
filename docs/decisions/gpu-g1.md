# GPU force tier G1 — the public-surface verdict on G1a and G1d

Status: G1a PROCEED-WITH-CONDITIONS, G1d PROCEED-WITH-CONDITIONS, 2026-10-06

Ruled on `docs/superpowers/plans/2026-10-06-gpu-g1.md` as written, before code, under
`.claude/rules/devil/risk.md` (public surface; format change; wide blast). No code was written
and no build was run for this record. Every citation below was re-read on `gpu-g1-verdict` at
`7888af5b`.

## Rulings

| # | Question | Ruling | Evidence (path:line or command) |
|---|---|---|---|
| 1 | Are all seven new `pub` items needed? | **No — condition 1.** Six of the seven collapse into one type and one method. `MeshPass` exists only to be an argument to one method; `MeshGraph` and `MeshSolution` exist only as return types of the three methods; `MeshFrame` exists only as a field of `MeshSolution`. Required form: `pub struct MeshProbe` + `pub fn mesh_probe(&self) -> MeshProbe`, `MeshFrame`'s seven fields inlined, `MeshPass` deleted from the public surface and kept `pub(in crate::layout::force)`. **Cargo feature: rejected** — graph-core has no `[features]` today, and the gate row `cargo build -p graph-core --target wasm32-unknown-unknown` builds *without* the feature, so the probe would never be compile-checked for wasm32: the coverage hole lands exactly on the new code. It also adds a second build configuration to the one crate whose promise is a single byte-identical build. **`#[doc(hidden)]`: rejected** — zero occurrences in graph-core today (`git grep -c 'doc(hidden)' -- crates/` prints nothing); the first one in the crate, introduced to hide a fixture emitter, is a precedent every later surface can hide behind. It reduces documentation, not surface. | `crates/graph-core/src/layout/mod.rs:18` (`pub mod force`); `crates/graph-core/src/layout/force/mod.rs:34` (the re-export line); `crates/graph-core/src/layout/force/barnes_hut/sim.rs:46` (`graph` is `pub(in crate::layout::force)`); `crates/graph-core/src/layout/force/session.rs:220-236` (the only public readers today: `xs`, `ys`, `params`, `alpha`); `crates/graph-core/src/layout/force/particle_mesh/frame.rs:34` (`Frame` is `pub(super)`); `crates/graph-core/Cargo.toml` (no `[features]`); plan `:264-298` (the crate-internal items, which stay crate-internal) |
| 2 | Can the probe change a later tick, a hash or a session's state? | **Structurally no; condition 3 makes it a test.** The guaranteeing line is `fidelity.rs:84`, `fn charge_probe(&self, theta: f64) -> Sim`, combined with `sim.rs:89` `from_parts`, which takes `graph` and `(x, y)` **by value** and allocates every velocity column at zero — the probe is handed clones (`fidelity.rs:86-88`), and `Mesh::new(rows)` is fresh, so `self.mesh` is never reached. Both new methods take `&self`. But the plan's own test is weaker than the property `charge_deltas` already documents: `a_delta_is_the_pass_and_not_the_tick` pins `xs`, `ys`, `alpha` and **not** `tick_no`, the pins, or `params()`, while `fidelity.rs:51-53` claims all five. | `crates/graph-core/src/layout/force/session/fidelity.rs:84-92`; `crates/graph-core/src/layout/force/barnes_hut/sim.rs:89-119`; `crates/graph-core/src/layout/force/session/fidelity.rs:51-53`; plan `:583-589` |
| 3 | Must `.gmfx` be specified under `docs/contract/` or `graph-contract`? | **`fixtures/gpu/README.md` is enough — condition 5.** Not a product format: no external consumer, no wire, no snapshot. It has exactly one property that makes it contract-shaped — two independent implementations, a Rust writer and a TS reader, that must agree byte for byte — and that is discharged by `--check`, by the header's major/minor, and by a README whose table is normative-complete (every offset, every section, how every length is derived). A `graph-contract` module would put a fixture format in the crate the motor ships. **Sizes: acceptable.** 2 × 0.34 MiB = 0.68 MiB committed, against a largest existing fixture of ~108 KiB (`fixtures/scale`) — under a megabyte, and `--check` is meaningless without them committed. The fingerprint cost is one-time and the plan already names it. | `crates/graph-cli/src/fingerprint.rs:54-73` (`fixtures` in `FINGERPRINTED`); `du -sh fixtures/*` (scale 108K is the largest); plan `:404-424`; python3: `16·m+16·n+16·P+16·P²+48·n` at n=1000, P=128, m=1550 → 353 064 B = 0.34 MiB, matching the plan's ≈353 000 |
| 4 | Which GPU defects survive an oracle that loads the CPU's twiddles and spectrum? | **Acceptable for a per-device tier — condition 6.** The comparison is a **transcription check, not an algorithm check**. Caught: every GPU-side defect — deposit cell and weight indexing, the FFT's butterfly, radix, bit-reversal and stage-`half` twiddle indexing, a double-applied `1/P²`, the CIC read's stencil and fixed order `(at, at+1, at+P, at+P+1)`, the `charge·alpha` scale, the f64→f32 narrowing, workgroup indexing, atomics, and the transposed `a[kx·P+ky]` read. Invisible: a CPU-side error in the twiddle table, the kernel spectrum, the `1/P²` prescale, the Green's function, or the frame ladder — the arms would agree and both be wrong. That is covered elsewhere and named here so the record does not overstate the gate: `perf-mb-fidelity` grades the mesh against the exact sum and the hash gate pins the CPU mesh's bytes. The record's gate wording (`gpu-force-tier.md:39-42`) says "agrees with the CPU mesh"; it does not say "and the CPU mesh is right", and the measurement doc must now say so. | `crates/graph-core/src/layout/force/particle_mesh/fft.rs:79-104` (twiddles, the `k.max(1).ilog2()` quirk); `crates/graph-core/src/layout/force/particle_mesh/kernel.rs:64-91` (`sample`, `green`, the `1/P²`); `crates/graph-core/src/layout/force/particle_mesh/frame.rs:50-89` (the rung ladder); `crates/graph-core/src/layout/force/particle_mesh/mesh.rs:184-195` (the read's fixed order); `docs/decisions/gpu-force-tier.md:53-55`, `:39-42` |
| 5 | Are the G1b/G1c bounds soundly derived, and does each have a control that fails a wrong kernel? | **No — conditions 7, 8, 9.** Three findings. (a) The charge `maxAbs` derivation prints a formula that does not produce its own number: `√2 · 2⁻¹¹ · √1.25 = 7.7e-4`, not the stated `3e-4` — off by 2.6×. The record's `3e-4` is right and the formula beside it is wrong; four weights each rounded with uniform error over one quantum, rms `2⁻¹¹/√12` apiece, four in quadrature, gives `2⁻¹¹/√3 = 2.8e-4`. Carried through, `90 · 3e-4 / 724 = 3.7e-5` stands; the plan's own formula would give `9.6e-5`. Also `1e-4` is **36×** under `3.6e-3`, which is 1.5 orders of magnitude, not "one order" (plan `:1032`). (b) The link ceiling is ~3× **below** the worst case it claims to cover. `k·2⁻²⁴` at k=4 is `2.4e-7`, leaving `5.6e-7` "for the one `sqrt` and one division" — but WGSL `sqrt` is 2 ULP and `x/y` is 2.5 ULP, and an f32 ULP is `2⁻²³`, not `2⁻²⁴`. Per term that is `(0.5+2+2.5)·2⁻²³ = 6e-7`, and over k=4 terms `2.4e-6`: 3.0× the `8e-7` ceiling. And k is the *max* degree of a preferential-attachment graph, not 4 — the plan concedes this (`:1292-1297`) but still prints 8e-7 as "the derived ceiling". (c) **Collide has no control that turns a bound red.** `negctl-collide-order` is admitted in the plan to pass every f32 bound, because reordering a sum differs by rounding and not by O(1) (`:1336-1338`). So Review Focus 2 ("a broken stage must be attributable to a stage") and constraint 11 ("every gate row has a negative control that must fail") are both unmet for one of the three passes. `negctl-butterfly` and `negctl-order` do cover charge; `negctl-gravity`/`negctl-decay`/`negctl-tick-order` cover the tick but not the link or collide kernels. | python3: `√2·2⁻¹¹·√1.25 = 7.72e-4`; `2⁻¹¹/√3 = 2.82e-4`; `3.6e-3/1e-4 = 36`; `4·(0.5+2+5/2)·2⁻²³ = 2.38e-6` vs `8e-7`; `gpu-force-tier.md:88-90`, `:98-101`; plan `:1032`, `:1036-1039`, `:1288`, `:1336-1338`; `crates/graph-core/src/layout/force/particle_mesh/collide/gather.rs:18` (`WINDOW = 256`); `crates/graph-core/src/layout/force/particle_mesh/collide.rs:255-256` |
| 6 | G1d's `ForceEngine` value, the studio toggle, the fallbacks. | **Rule the shape now; condition 10 gates the default on G1c's measurement.** The shape is already decided by `gpu-force-tier.md:34-35, 64-66`: an SDK-only third value, never auto-substituted, never through the wasm ABI. The one-line union widening at `types.ts:197` is additive and `force-create.ts` refuses it with a typed error, which is the right refusal. What is *not* decided is the default and the fallback's cost. Rule: the toggle ships **off**, and no measurement is needed for that. G1c must record `msPerTick` at 1M and `stressRatio` before the toggle may be offered on — and if the stress ratio is outside 2.0 that is a stop (`gpu-force-tier.md:91-94`), so gating the default on it is not a formality. | `crates/graph-sdk-js/src/types.ts:197`; `crates/graph-sdk-js/src/force-create.ts:18-31`; `docs/decisions/gpu-force-tier.md:34-35`, `:62-66`, `:13` |

## The failure nobody mentioned

`SIZES` is `[1_000, 10_000, 50_000]` (plan `:705`) and the size table has no 1M row — but the
charge ceiling is *derived at 1M* (`:1038`, "at 1M with `charge·alpha ≤ 90` and `h = 26.909`"),
the frame table's 1M row is offered as "the cross-check that both are right" (`:347-349`), and
G1c's `tick-1m` row reads `--n 1000000` out of `target/gpu-fixtures`. No row emits a 1M fixture,
so `tick-1m` reads a file that does not exist. The size is the reason: at n=1M, P=1024 the file
is **105 593 672 B = 100.7 MiB**, and the plan's own base64 handoff into the page (`:1183-1185`)
turns that into a **134 MiB** string through Python and `import()`. A derived ceiling for a
fixture the emitter does not produce, computed against a state no gate reaches, is not a bound.
The frame ladder is also unverified where it is most fragile: `place` walks `step` upward until
`fit` succeeds (`frame.rs:50-67`) with no iteration cap, so a degenerate span is an unbounded
loop inside the emitter.

## Scores

| axis | score | why |
|---|---:|---|
| blast radius | 3 | G1a touches seven `pub` items in the crate every consumer links and commits two fingerprinted goldens; G1d touches `types.ts`, `force-create.ts`, `force.ts`, and three studio files. Additive throughout, and the 4-way hash gate is the tripwire for the motor. Not 4: nothing here changes a layout's bytes, and the tier is explicitly outside `LAYOUTS` and the hash gate. |
| reversibility | 4 | One commit deletes either slice. The only irreversible edge is the fingerprint move from committing `fixtures/gpu/*.gmfx` — one-time, planned, and it costs a gate re-run, not a migration. |
| cost on failure | **3** | **the worst axis.** Not because a red row is expensive — it is cheap — but because of the failure mode the plan does not name: a fixture whose CPU columns and whose GPU arm are wrong *in the same way* gates green. `charge_deltas` compares the probe against itself (plan `:571-580`), so it cannot catch a probe that misreads the mesh. The tier's whole oracle rests on `every_column_is_the_meshes_own`, one test, against one instrument. |
| confidence | 3 | Most citations verified exactly (`fidelity.rs:84`, `sim.rs:89`, `frame.rs:34`, `mod.rs:18`, `fingerprint.rs:54-73`, `mesh.rs:43`, `fft.rs:67`, `kernel.rs:32`, `link.rs:90`, `collide.rs:261`, `motion.rs:150-152`). But two derivations did not survive arithmetic (rulings 4 and 5), one fixture size is 2.6× off its own formula, and the 1M case is specified in three places and emitted in none. |

**Worst axis: cost on failure, 3.** A silently-green gate is the one outcome the house cannot
detect later, and the plan's defence against it is a single self-referential test.

## Conditions

Each is checkable by the named command or test. Conditions 1–6 bind G1a, 7–9 bind G1b/G1c and
are stated here because the bounds were part of this ruling, 10–12 bind G1d.

1. **The public surface is one type and one method.** `git grep -n 'pub ' -- crates/graph-core/src/layout/force/session/mesh_probe.rs` names exactly `MeshProbe` and `mesh_probe`, nothing else. `MeshFrame`'s seven fields are inlined into `MeshProbe`; `MeshPass`, `MeshGraph` and `MeshSolution` do not exist as public types. `force/mod.rs:34` re-exports `MeshProbe` beside the names already there and nothing more. The pass enumeration stays `pub(in crate::layout::force)` on `particle_mesh::pass`.
2. **No `#[allow]` and no `#[doc(hidden)]`** gets the probe through. `cargo clippy --workspace --all-targets -- -D warnings` exits 0 with constraint 8's reason line on anything it needs (plan `:75`).
3. **The no-change property is a test, not an argument.** A new test `the_probe_leaves_the_next_tick_byte_identical` builds two identical mesh sessions, calls `mesh_probe()` on one, `step(1)` on both, and asserts the two position columns and `tick_no` are bit-equal. `a_delta_is_the_pass_and_not_the_tick` is extended to pin `tick_no`, one pin and `params()` as well, matching what `fidelity.rs:51-53` already claims.
4. **The hash gate does not move.** `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` exits 0 with the same digests, and row `no-code` in `scripts/orch/rows/docs.rows` exits 0. A moved hash is a finding, not a re-baseline (plan `:868-871`).
5. **The README is normative-complete and the loader is version-strict.** `fixtures/gpu/README.md` carries every offset and section of the 64-byte header and the payload in wire order, and states how each length is derived from `n`, `m` and `P` alone. A new test `the_loader_refuses_a_version_it_does_not_know` feeds `gpu/fixture.ts` a header with `format major = 2` and requires a throw.
6. **`SIZES` gains `1_000_000`, or every 1M claim in the plan is struck.** `emit-gpu-fixtures --out target/gpu-fixtures` writes a 1M pair, `ls -l target/gpu-fixtures/mesh-1m-*.gmfx` shows two files, and the fixture crosses into the page by `fetch` over the harness's own origin — **not** base64, which at 100.7 MiB is a 134 MiB string. If 1M is dropped instead, then `tick-1m` (`:1373`), `ms_per_tick_at_1m` (`:1362`) and the 1M row of the `maxAbs` derivation (`:1033`) are struck from the plan and `docs/measurements/gpu-g1.md` says the tier has no 1M fixture.
7. **The charge `maxAbs` derivation is corrected before it is written down.** The README and `docs/measurements/gpu-g1.md` print `2⁻¹¹/√3 = 2.8e-4`, not `√2 · 2⁻¹¹ · √1.25`; the `3.7e-5` figure is unchanged. The `1e-4` row says "1.5 orders of magnitude under 3.6e-3", not "one".
8. **The link ceiling is `k_measured · 5 · 2⁻²³`, not `8e-7`.** `bounds.ts` reads the maximum degree from the fixture's own edge columns, `k = 4` is not used, and the comment states that an f32 ULP is `2⁻²³` while `2⁻²⁴` is the unit roundoff.
9. **Collide gets a control that is O(1) wrong.** A new `negctl-collide-window` makes the resolve drop one candidate from the populated window (or resolve against `2·collide_radius`), and row `negctl-collide-window|nonzero|…` in `scripts/orch/rows/gpu-g1c.rows` must exit non-zero. `negctl-collide-order` stays — it proves the order is compared, which no bound can — but on its own it leaves the collide ceiling with no failing control, against constraint 11.
10. **The toggle ships off.** `the_toggle_off_is_the_default` on a fresh settings object, and `the_toggle_is_disabled_with_a_reason` when `navigator.gpu` is absent. Neither waits on G1c.
11. **G1c's numbers gate the default, not the code.** `docs/measurements/gpu-g1.md` records `msPerTick` at 1M against the 100 ms browser budget (`gpu-force-tier.md:13`) and `stressRatio` against the factor of 2.0, before the toggle is offered on a fresh settings object. A stress ratio outside 2.0 is a stop, and the report must say whether the cause is the f32 spacing floor or the collide window.
12. **Both fallbacks are tests, and the readback's cost is stated.** `no_adapter_falls_back_to_the_cpu_mesh_and_says_so`, `a_lost_device_resumes_from_the_last_readback`, and row `negctl-gpu-no-adapter|nonzero|…` all present; `force.ts`'s doc on the held `Float32Array` names the 8 MB at 1M and says `release()` drops it.

## What a reviewer should re-check first

The cost-on-failure axis. `every_column_is_the_meshes_own` compares `mesh_pass_deltas(Charge)`
against `charge_deltas` — the same instrument, reached through the new probe. It proves the two
agree. It does not prove either read the mesh. The independent check already in the repo is
`mb_fidelity`, which grades `charge_deltas` against the exact all-pairs sum; run it on the same
fixtures and let its number, not the probe's self-agreement, be what says the column is the
mesh's.

## Amendment 1 — the collide guard (orchestrator, 2026-10-06; an independent review is owed)

**Finding.** The plan's collide guard, `rmsRel ≤ 1e-4`, fails on `mesh-1k-settled` on both arms
(`rmsRel` 5.41e-4 hardware, 5.38e-4 software), while the other five fixtures pass. The kernel is
not the cause: a JS `f32` reference that follows the kernel op for op gives `rmsRel` 5.379e-4 and
`maxAbs` 2.637e-5, and the device gives 5.406e-4 and 2.637e-5 (`docs/measurements/gpu-g1.md`, "G1c
— the collide pass", branch `gpu-g1c-collide`).

**Why the guard is wrong.** The guard is relative to the net delta. At equilibrium the pushes on a
node nearly cancel: that fixture's `rmsRef` is 0.0072, against 0.67 to 172 on the other five. The
`f32` rounding error, however, scales with the pushes themselves, not with their net. The bound for
a rounded sum is `|fl(Σxᵢ) − Σxᵢ| ≤ γₖ·Σ|xᵢ|` (Higham), and that bound is relative to `Σ|xᵢ|`,
not to `|Σxᵢ|`.

**Ruling.** The collide guard becomes mixed, absolute plus relative, in the form of condition 8:

`rmsAbs ≤ 1e-4 · rmsRef + k_c · 5 · 2⁻²³ · P`

As a relative guard for `compare()`, this is `1e-4 + k_c · 5 · 2⁻²³ · P / rmsRef`.

- `k_c` is the largest number of contacts of any one node: pairs closer than `reach`, the CPU's
  `d2` test. It is counted in f64 on the host from the fixture's own start positions.
- `P` bounds one push. The push is `(reach − dist)·0.5` (`collide.rs:254-256`), so its magnitude is
  at most `reach / 2 = collide_radius`. `P` also carries any strength factor that the
  `delta_collide` column includes.
- `5 · 2⁻²³` is condition 8's per-term rounding (`0.5 + 2 + 2.5` ULP for the product, the `sqrt`
  and the division), with an f32 ULP of `2⁻²³`.
- Caveat: `k_c` is measured on the fixture's positions. A denser crowd raises it, so the guard is
  only as good as the fixture's measured crowd, the same limit condition 8 states for link.

**Condition 9 still binds.** `collide-window` must still fail under the new guard on both 1k
fixtures. On `mesh-1k-settled` its `rmsAbs` is about 0.62 · 0.0072 = 4.5e-3. At `k_c ≤ 55` and
`P = 16`, the new term is at most 5.2e-4, so the control stays red by about 8.6×.

## Amendment 2 — the 1M charge `maxAbs` guard (orchestrator, 2026-10-06; an independent review is owed)

**Finding.** G1b stopped at the plan's 1M stop. Its `maxAbs` was 2.36e-2 on `mesh-1m-settled`
against a guard of 4.96e-5, and 8.98e-3 on `mesh-1m-start` against 3.50e-5. Its `rmsRel` stayed
under 1e-4 on both (3.64e-5 and 7.48e-5).

**The experiment.** The analysis job `g1b-floor` ran a numpy transcription of the charge pass
against the fixtures' f64 `delta_charge`.

- **(a) f64 throughout** reproduces every fixture to `maxAbs` ~1e-13, so the transcription is
  right.
- **(b) the GPU's f32 arithmetic** (the 2⁻¹¹ fixed-point deposit, a radix-2 FFT in complex64 and
  an f32 read) gives `rmsRel` 3.65e-5 / 7.49e-5 and `maxAbs` 2.97e-2 / 1.01e-2 at 1M.
  - The GPU matches it to three significant figures in `rmsRel`.
  - The GPU's `maxAbs` is 21% and 11% lower: the spread of a single maximum.
- **(c) f64 everything except the quantised deposit** alone gives 2.85e-2 / 9.71e-3. The deposit
  quantum is the dominant error, as the guard assumed.
- **(d) the f32 transform alone** gives 1.18e-4 / 5.76e-5.

So no correct f32 implementation meets the old guard. Even a perfect transform over the
quantised deposit misses it by 277× to 575×. The kernel is not defective.

The script was kept on branch `g1b-floor` under `target/floor/` (untracked); its re-run command
is in the job's report.

**Why the guard is wrong.** Its deposit term, `2⁻¹¹/√3`, is right: the rms of four CIC weights,
each rounded over one quantum. Two steps after it are wrong.

1. **The propagation factor.** The field is the convolution of the density error with the
   sampled kernel `g`, so the field error's rms is `δ_rms · ‖g‖₂`, not `δ_rms / h²`. The kernel
   `G = −r/l(r)` is a 1/r law. The 2-norm comes from the fixture's own spectrum:
   `‖g‖₂ = P·‖spectrum‖₂`, which is 0.2085 on the settled fixture and 0.1712 on the start one.
   `1/h²` is about 100× smaller than that.
2. **An rms read as a maximum.** The guard has no peak factor over the 2n read components.

**Ruling.** At the two 1M fixtures the guard becomes:

`maxAbs ≤ |charge·alpha| · (2⁻¹¹/√3) · ‖g‖₂ · 6`, with `‖g‖₂ = P · ‖spectrum‖₂`

- The factor 6 is `√(2·ln 2n)` = 5.39 at n = 1e6, rounded up.
- `‖spectrum‖₂` is computed from the fixture's spectrum section, as condition 8 reads `k` from
  the fixture's own edges.
- Values: 3.17e-2 settled, 2.61e-2 start. The GPU sits under both, by 1.34× and 2.9×.
- Below 1M there is still no `maxAbs` guard (`prompts/jobs/gpu-g1b.md:86-88`). `rmsRel ≤ 1e-4`
  is unchanged everywhere.
- Caveat: this is a statistical bound, not a strict one. It models the deposit error as white
  noise and its peak as Gaussian. The fixed-point weights sum to exactly one per node, which makes
  the true error field slightly smaller than the model. The f32 transcription sits 7% under the
  settled value. Another seed's 1M maximum could cross it, and that is a stop, not a re-tune.
