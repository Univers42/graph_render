# `.gmfx` — the particle mesh's own per-pass deltas, at one state

A `.gmfx` is what `graph-cli emit-gpu-fixtures` writes and what the GPU arm loads unchanged:
the mesh's collapsed adjacency, the frame it placed for one set of positions, the transform's
own twiddles, the kernel spectrum, and **what each of the mesh's three force passes adds to
the velocities from rest**.

One generator, one format, one loader (`crates/graph-sdk-js/src/gpu/fixture.ts`): the repo's
oracle rule. Nothing in this file is recomputed on the reading side — the twiddles are
`libm`'s, the spectrum is the kernel's own, and every delta column is a force pass run by
graph-core on a copy of the session.

**This file is normative.** Every offset and every section length is stated below, so a
reader needs nothing but `n`, `m` and `P`. `graph-cli emit-gpu-fixtures --check` is what
keeps it true: it re-emits and byte-compares, so a format change that does not reach this
table fails the gate.

## The two states, and what a state is

| state | word | positions |
|---|---|---|
| `start` | `0` | the engine's own golden-angle spiral, `r = 12·√(i+1)` |
| `settled` | `1` | those positions after **100** ticks of the mesh |

100, not the frozen `TICKS = 112`, because 100 is the number every other measurement in this
repo calls settled (`crates/graph-cli/src/mb_fidelity/measure.rs:19`).

**The model is the gate's own**: `stage::topology::seeded_model(0, n, REFERENCE_DEGREE)` —
one seed, so a fixture's identity is its node count. Underneath it is preferential attachment
plus 5 % `note_link` extras from `Mulberry32::new(0x05_1042)`.

**Caveat: `m ≈ 1.55n` is an assumption, not a fact.** The emitter prints the real `m` in every
header and in its own output; the sizes below are estimates built on the assumption, and the
last column is the number the emitter actually wrote.

**Caveat, and it travels with every delta column: the columns are from a copy at tick 0.**
`ForceSession::mesh_probe` runs each pass against a copy of the session whose velocities
start at zero and whose `alpha` is 1, and that copy is at tick 0 whatever the session's own
tick was (`session/fidelity.rs:26-29`). Collide reads `tick_no` for its coincidence jiggle, so
the **settled** fixture's collide column is the collide pass *at tick 0* — the state and the
pass are two different clocks. This is a property of the probe, not of the format; the header
carries no tick number and no velocities, so nothing downstream can mistake one for the other.

**The frame is recomputed from the positions every tick** (`particle_mesh/mesh.rs:119-125`), so
the start and settled files have different frames, different `h` and different kernel spectra.
That is why there are two files rather than one file with two columns.

## The 64-byte header

Little-endian throughout. Every integer is `u32` (D6): no `usize`, no `i32`, no `f64`-adjacent
width sneaks in. Every float is `f64` — the CPU's own, never narrowed, because the whole
comparison is against an `f32` arm and at 12 000 units `f32`'s spacing is 2⁻¹⁰
(`gpu-force-tier.md:91-93`), so a reference already narrowed could not show a 2⁻¹¹ difference.

| offset | size | type | field |
|---:|---:|---|---|
| 0 | 4 | bytes | magic `GMFX` (`0x58464d47` read little-endian) |
| 4 | 4 | `u32` | format major — **1** |
| 8 | 4 | `u32` | format minor — **0** |
| 12 | 4 | `u32` | node count `n` |
| 16 | 4 | `u32` | simple-edge count `m` |
| 20 | 4 | `u32` | mesh side `P` |
| 24 | 4 | `u32` | state: `0` start, `1` settled |
| 28 | 4 | `u32` | flags: bit 0 set exactly on the settled file |
| 32 | 4 | `u32` | rung `step`, the two's complement of the `i32` |
| 36 | 4 | `u32` | pad, always `0` |
| 40 | 8 | `f64` | cell size `h` |
| 48 | 8 | `f64` | origin `x` |
| 56 | 8 | `f64` | origin `y` |

The header ends at 64. `cells` and `reach` are the payload's first two words, not header
fields, so every header offset is a multiple of 4 and the three `f64`s are 8-byte aligned.

**A reader must refuse a `format major` it does not know.** A reader that accepted a future
major would read a longer header as this one's fields and produce silently wrong numbers. A
newer *minor* is read, because the minor only ever appends sections after the ones below.
Pinned by `the_loader_refuses_a_version_it_does_not_know`.

## The payload, in wire order

| # | section | type | count | offset from 64 |
|---:|---|---|---:|---:|
| 1 | `cells`, `reach` | `u32` | 2 | 0 |
| 2 | `edge_lo` | `u32` | `m` | 8 |
| 3 | `edge_hi` | `u32` | `m` | 8 + 4`m` |
| 4 | `edge_strength` | `f64` | `m` | 8 + 8`m` |
| 5 | `pos_x`, `pos_y` | `f64` | `n` each | 8 + 16`m` |
| 6 | `twiddle_re`, `twiddle_im` | `f64` | `P` each | 8 + 16`m` + 16`n` |
| 7 | `spectrum_re`, `spectrum_im` | `f64` | `P·P` each | … + 16`P` |
| 8 | `delta_link_x`, `delta_link_y` | `f64` | `n` each | … + 16`P`² |
| 9 | `delta_charge_x`, `delta_charge_y` | `f64` | `n` each | … + 16`n` |
| 10 | `delta_collide_x`, `delta_collide_y` | `f64` | `n` each | … + 16`n` |

So the whole file is

```
64 + 8 + 16m + 16n + 16P + 16P² + 48n  bytes
```

and a reader needs nothing but `n`, `m` and `P` to place every section. Node order is the
dense index throughout. `P = side_for(n) = ceil(√n).next_power_of_two().clamp(128, 1024)`
(`particle_mesh/mesh.rs:31-34`): **128** at 1k and 10k, **256** at 50k, **1024** at 1M.

**`twiddle_*` is `Plan::forward` whole.** Stage `half` reads the contiguous run
`twiddle[half..2·half]` (`fft.rs:130`), so a GPU stage `half` reads `[half + j]` — the table is
uploaded as-is and never rebuilt from `sin`/`cos`, which WGSL has only to 2⁻¹¹ absolute.

**`spectrum_*` is `Kernel::spectrum`, pre-scaled by `1/P²`.** The arm must **not** divide by
`P²` again (`particle_mesh/kernel.rs:66-76`).

**`edge_lo`/`edge_hi`/`edge_strength` are graph-core's own collapse** (`layout/force/mod.rs:64-69`):
undirected, deduplicated, self-loop-free, and among the raw edges that collapse onto one
unordered pair the **first by ascending index** keeps its strength. `lo < hi` always. A
TypeScript re-implementation of that rule would be a second implementation of a graph-core
convention, which is the one thing this format exists to prevent.

**No density column, no velocities, no `alpha`, no tick number.** The GPU computes the deposit
itself; shipping the CPU's would test nothing. The deltas are from rest at `alpha = 1`, and
those scalars are the arm's business, not a column it could get wrong.

## The deposit's scale, derived and not stored

The fixed-point scale the deposit needs is the largest power of two that keeps `n` unit
charges in one cell under `i32::MAX`:

```
scale_for(n) = 2^(31 - ceil(log2 n))        n = 1_000_000 gives 2^11, so the quantum is 2^-11
scale_for(0) = nothing                     ceil(log2 0) has no value
```

Integer arithmetic, no `libm`, in `emit.rs` and in `gpu/fixture.ts`. `n = 1` gives `2^30` and
is in the shared test table because that is the one case where a signed `<<` is a single bit
from correct rather than visibly wrong — **the TypeScript side uses `2 **` and never `<<`**.

## The bounds the next slice is held to

Both are the decision record's, restated here with the arithmetic that produced them, because
the first GPU run either confirms them or names the kernel that broke.

**Charge, absolute.** The deposit quantum at 1M is 2⁻¹¹ of a unit charge. Four CIC weights
each rounded with independent uniform error over one quantum give an rms of `2⁻¹¹/√12` apiece,
and four in quadrature give **`2⁻¹¹/√3 = 2.8e-4`** of a unit per occupied cell. The field that
produces is `(charge·alpha)·2.8e-4 / h²`, and at 1M with `charge·alpha ≤ 90` and `h = 26.909`
that is **`3.7e-5` units per tick**. (This is the figure the record's own `3·10⁻⁴` gives; the
formula beside it in the plan, `√2 · 2⁻¹¹ · √1.25`, does not produce the record's number and is
not the derivation.)

**Charge, relative.** `1e-4` rms, which is **1.5 orders of magnitude** under the mesh's own best
error against the exact all-pairs sum, `3.6e-3` rms (`perf-mb-fidelity.md`).

**Link.** The ceiling is **`k_measured · 5 · 2⁻²³`**, where `k_measured` is the maximum degree
read from the fixture's own `edge_lo`/`edge_hi` columns — not a constant. Per term: one
subtraction (0.5 ULP), WGSL's `sqrt` (2 ULP) and one `x/y` (2.5 ULP), and an **`f32` ULP is
2⁻²³** while 2⁻²⁴ is the unit roundoff — so 5 ULP per term, over `k` terms. The plan's `8e-7`
assumed `k = 4`, which is neither the measured maximum degree nor a bound on it.

**A `maxAbs` ceiling below 2⁻¹¹ units at 1M is unmeasurable, not tighter.** Positions are `f32`
and the seed spiral reaches 12 000 units, where `f32`'s spacing is 2⁻¹⁰, so a step under 2⁻¹¹ is
simply lost. Any number under that floor is reported as a zero, not as a pass.

## Sizes — all estimated, with the real count beside it

`64 + 8 + 16m + 16n + 16P + 16P² + 48n`, with `m ≈ 1.55n`:

| file | `P` | bytes (estimated) | MiB (estimated) | bytes (what the emitter wrote) |
|---|---:|---:|---:|---:|
| `mesh-1k-start.gmfx` | 128 | 353 000 | 0.34 | **352 776** |
| `mesh-1k-settled.gmfx` | 128 | 353 000 | 0.34 | **352 776** |
| `mesh-10k-start.gmfx` | 128 | 1 152 000 | 1.10 | **1 151 608** |
| `mesh-10k-settled.gmfx` | 128 | 1 152 000 | 1.10 | **1 151 608** |
| `mesh-50k-start.gmfx` | 256 | 5 493 000 | 5.24 | **5 492 136** |
| `mesh-50k-settled.gmfx` | 256 | 5 493 000 | 5.24 | **5 492 136** |
| `mesh-1m-start.gmfx` | 1024 | 105 594 000 | 100.7 | **105 592 232** |
| `mesh-1m-settled.gmfx` | 1024 | 105 594 000 | 100.7 | **105 592 232** |

`m` is 1532 at 1k, 15459 at 10k, 77462 at 50k and 1 549 910 at 1M — so the `m ≈ 1.55n`
assumption holds to within a percent at every size.

## What is committed, and how the 1M files travel

**Only the 1k pair is committed.** `fixtures/` is inside the fingerprint list
(`crates/graph-cli/src/fingerprint.rs:54-73`), and 100.7 MiB × 2 of derived binary in there
would be two orders of magnitude larger than anything already committed
(`fixtures/scale/n220.json`, 100 KiB) and would re-fingerprint the tree on every
regeneration. The other six files are written into `target/gpu-fixtures/` by the gate row and
are derived data.

**The 1M fixture crosses into the page by `fetch` over the harness's own origin, never as
base64.** At 100.7 MiB a base64 handoff is a 134 MiB string through `import()`, which is not a
transport. The next slice's harness serves the file and the loader reads the bytes; there is
no inlining step and there is no second copy of the fixture in a JS bundle.
