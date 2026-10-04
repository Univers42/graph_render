# p12-3d-oracles: `layout.random.3d` and the 3-D arms of the two oracles

Port of `p12-t4a` (`971318dc`) onto develop (`bccdf383`) under Option A
(`docs/decisions/3d-ids.md`): develop's 3-D ids win, `layout.random.3d` is the one id
t4a had that develop did not, and everything else was dropped as a duplicate. This
file records what the port measured on this tree, the two ceilings that moved and why,
the one kernel fix it needed, and what was deliberately left behind.

Both differentials ran over the 1000 gate seeds. Nothing below is re-derived from t4a's
notes; every figure is this tree's.

## `oracle-closed-form`

```
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-closed-form-fixtures --seeds 1000
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle \
    python3 harness/oracle-closed-form.py target/closed-form-fixtures /w/SciGraphs/core
scripts/orch/gr cargo run -q --release -p graph-cli -- oracle-closed-form
```

| id | worst | ceiling | source |
|---|---|---|---|
| `layout.circular.ring` | 2.654e-7 | 1e-6 | networkx `circular_layout` (pre-existing, unchanged) |
| `layout.spiral` | 2.980e-8 | 1e-7 | networkx `spiral_layout` (pre-existing, unchanged) |
| `layout.bipartite` | 2.980e-8 | 1e-7 | networkx `bipartite_layout` (pre-existing, unchanged) |
| `layout.random.3d` | **3.030e-1** | 5e-1 | DISTRIBUTION metric, not coordinates — see below |
| `layout.basic3d.spiral` | **2.384e-7** | 1e-6 | SciGraphs `_spiral_layout_3d(n, 5.0)` (`basic.py:36`) |
| `layout.bipartite_3d` | **1.192e-7** | 1e-6 | SciGraphs `_bipartite_layout_3d(G, 5.0)` (`hierarchical.py:213`) |

The first three rows are `docs/measurements/closed-form-oracle.md`'s, re-run here and
unchanged: the ceilings are declared at `oracle_python/closed_form.rs:45` and `:62`, `:81`, and the
three 3-D rows at `:34-36`.

## `oracle-spectral`

```
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-spectral-fixtures --seeds 1000
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle \
    python3 harness/oracle-spectral.py /w/SciGraphs/core target/spectral-fixtures
scripts/orch/gr cargo run -q --release -p graph-cli -- oracle-spectral
```

| id | cases | degenerate | worst | ceiling |
|---|---|---|---|---|
| `layout.spectral` | 984 | 14 | 7.178e-6 | 1e-5 (pre-existing, unchanged) |
| `layout.mds.pivot` | 996 | 2 | 3.691e-8 | 1e-7 (pre-existing, unchanged) |
| `layout.spectral3d` | 982 | 16 | **1.030e-5** | **1e-4** |
| `layout.mds.pivot3d` | 990 | 8 | **4.838e-8** | **1e-7** |

The `degenerate` column is the harness's own: seeds whose eigenspace is degenerate inside
the reference's span are counted, not compared (`harness/oracle-spectral.py:23`, `:276`).
`layout.spectral3d`'s ceiling is the only one that moved, and it is `CEILING_SPECTRAL_3D`
(`oracle_python/spectral.rs:46`) — one power of ten above the 2-D arm's 1e-5. `layout.mds.pivot3d`
keeps 1e-7 because it measured as its 2-D sibling (`spectral.rs:35`).

## Why the two 3-D closed-form rows take 1e-6 and not the 1e-7 their 2-D siblings take

One reason, and it is measurable. `layout.basic3d.spiral` and `layout.bipartite_3d` draw at
the 3-D dispatcher's `scale = 5.0` (`SciGraphs/core/scigraphs_core/mesh/layouts/dispatcher.py:14`),
so their coordinates run to about +/-5, where the 2-D arms run to +/-1 — and the snapshot's
`f32` floor is **relative**, not absolute. 2.384e-7 is 2^-22 and 1.192e-7 is 2^-23: the `f32`
spacing at that magnitude and nothing else. The 2-D arms measure 2.980e-8 at +/-1, which is
the same figure scaled down by the same five.

The cross-check is that **1e-6 is the ceiling `oracle-basic-3d` already carried** for these same
two ids over these same 1000 seeds: `crates/graph-core/src/registry/three_d/spiral3d.rs:19`
records worst 2.384e-7 against a 1e-6 ceiling. Two independent harness paths, one figure, so
the number belongs to the layout rather than to this arm's oracle. The shared constant is
`CEILING_3D_COORDS` (`oracle_python/closed_form.rs:62`).

**t4a's numbers for these two rows do not reproduce here, and the reason is a scale, not a
regression.** t4a recorded 2.9802e-8 and 2.9798e-8 — the 2-D magnitudes. Its
`layout.spiral.3d` was compared at `scale = 1.0`; this tree's `layout.basic3d.spiral` draws at
5.0, which is what the reference actually does (`basic.py:36`, `dispatcher.py:14`). The figures
differ by the scale factor and both are at the `f32` floor for the magnitude in play. Recording
this rather than quietly dropping it, because a reader holding t4a's numbers would otherwise
read the two tables as a contradiction.

## `layout.random.3d`'s ceiling is 0.5, and it is not a coordinate tolerance

Our stream is `Mulberry32` at a fixed seed (`layout/random.rs:47`, `SEED = 0x00_5EED`); the
reference's `_random_layout` (`basic.py:5-9`) is `np.random.RandomState(get_layout_seed()).rand(n, 3) * scale`.
Two different generators at two different seeds, so **no coordinate can ever match for any
seed**. The arm therefore compares the DISTRIBUTION: per axis, the error in the sample mean
(target 1/2) and in the sample variance (target 1/12), worst of the six numbers wins
(`harness/oracle-closed-form.py:131`, `:151-152`). That is what `random_3d_gap` scores and what
`CEILING_3D_RANDOM = 0.5` (`oracle_python/closed_form.rs:81`) bounds.

**The floor is the smallest graph in the sweep, not our arithmetic.** The measured worst
0.30300 happens at `n = 2`, where the expected `|sample mean - 1/2|` for two uniform draws is
0.204 — the same order, and 0.5 is the furthest a two-point sample can reach. The figure falls
as `1/sqrt(n)` (0.0384 over the cases with `n >= 100`), which is the signature of sampling
noise rather than of a skewed stream. Hence 0.5: one significant figure above the measured
worst, so the row still bites on a stream that drifts.

`layout::random::run_seeded` is the **other** arm and does compare coordinates — off the ported
MT19937 at the layout seed 981798123 (`layout/random.rs:98-100`). It is deliberately a separate
entry point and not this id, so this id's registered snapshot does not move when either changes.
That is stated on the arm itself (`layout/random.rs:56-66`) and on the metadata row
(`registry/three_d/random3d.rs:42-64`).

## The one kernel fix, and it is the whole of the spectral internals work

Develop's `spectral_stage::spectral_3d` **refused** on the gate model.
`emit-spectral-fixtures --seeds 1000` exited 2 at seed 255 with "parameter topology: no component
passed the eigensolver's residual and orthonormality gate" — the refusal constant at
`crates/graph-core/src/layout/spectral_stage.rs:12`.

Cause: `start_block` in `crates/graph-core/src/linalg/lobpcg/ops.rs` drew its Weyl irrationals
from a fixed `[f64; 3]` read with `.take(block - 1)`. The reference's block is
`k = min(dims + 2, n_c - 1)`, so `dims = 3` asks for `block = 5` while the 2-D arm asks for 4 —
and a three-element table over `block = 5` leaves column 4 **entirely zero**. A start column of
all zeros is one LOBPCG cannot move off, so the solve returns the Laplacian's trivial
eigenvector (eigenvalue 0.0) and the residual gate refuses.

Why seed 255 was the first failure and not an arbitrary one: only components above
`DENSE_EIG_LIMIT = 256` (`layout/spectral.rs:65`) reach the iterative path at all, and seed
255's component is 257 nodes — the first gate seed that reaches it. Every seed below it took
the dense branch and passed.

Fix: `alpha_for(col)` **generates** the phase (`frac(col * PHI)`) past column 3 instead of
tabulating it (`ops.rs:59-72`), keeping the three historical values verbatim for columns 1..=3
so every 2-D start block stays bit-identical and no 2-D snapshot moves. Generating also removes
the failure mode itself — there is no fixed table length left to forget to raise. After the
fix seed 255 solves and the differential passes. Two unit tests assert the properties directly,
in `crates/graph-core/src/linalg/lobpcg/tests.rs`: `every_start_column_is_filled_at_every_block_width`
(`:60`) and `the_start_block_of_every_2d_run_is_unchanged` (`:84`).

The gate row that holds this in place is `spectral3d-above-dense-limit` in
`scripts/orch/rows/p12-3d.rows`, with `spectral2d-at-the-same-seed` as its 2-D control.

## What was deliberately not ported

- **`layout/spectral/space.rs`** (plus its tests) and **`layout/spectral/tests/dims_3d.rs`**.
  Develop already has the `dims` threading — `spectral::run_3d` (`layout/spectral.rs:196`),
  `pivot_mds::run_3d` (`layout/pivot_mds.rs:180`) and `spectral::DIMS_3D`
  (`layout/spectral.rs:55`). t4a's `space.rs` is the same refactor against a tree that had no
  3-D arms at all; porting it would be a second implementation of what develop has.
- **`crates/graph-core/examples/diag_{block,lobpcg,seed}.rs`.** Diagnostic examples for work this
  tree does not have. The two unit tests named above replace them and assert the property
  directly instead of printing it.
- **t4a's `registry/spectral.rs` entries and `*_3D_CEILING` aliases, `registry/closed_form.rs`'s
  `SPIRAL_3D`/`BIPARTITE_3D` rows, `registry/tests.rs`, and `layout/spiral/spiral_3d.rs`** — all
  duplicates of develop's ids, superseded under Option A (`docs/decisions/3d-ids.md` §Decision,
  the t4a → develop id map). One ceiling disagreement resolved in develop's favour there: t4a's
  `CLOSED_FORM_3D_CEILING` loses to `BASIC_3D_CEILING` for the same spiral/bipartite content.
- **The `p12-t4a` count literals** (snapshots, ledger, hashgate report). They were regenerated
  from this tree's own test output and never hand-merged: `LAYOUTS` 47 → 48, roundtrip
  `snapshots` 240 → 245, `NO_PER_STAGE_CONTROL` 33 → 34, plus `layout.random.3d` into
  `crates/graph-cli/src/snapshot_cmd/tests/names.rs:109` and the hashgate report fixture
  (`crates/graph-cli/src/hashgate/tests/report.rs:139`).
- **`harness/oracle-igraph.py`.** That is `sg-igraph-clean`'s half of the Python oracle work, not
  this job's.

## The gate rows, the registration, and the scale ceiling

The rows file is **`scripts/orch/rows/p12-3d.rows`**, run with
`scripts/orch/gate.sh target/gate-p12-3d-own scripts/orch/rows/p12-3d.rows`. It opens with
`scripts/orch/rows/quick.rows` verbatim, then adds the 3-D wire/marker rows, the hashgate rows
with their per-stage controls, both three-arm differential chains in order, and the
fail-closed ingest controls.

`layout.random.3d` is **`LAYOUTS` index 47**, append-only: `LAYOUTS` is
`[Capability; 48]` (`registry/layouts.rs:49`) and the new entry is last
(`registry/layouts.rs:296`), because the wasm module maps a layout by index and a reordering
would move every snapshot after it.

`layout.random.3d` takes `BASIC_3D_CEILING` (`registry/three_d/random3d.rs:68`, the constant at
`registry/three_d.rs:125`). That figure is **inherited** for this id, and its own `ponytail`
field says so (`random3d.rs:70-95`): it is the seventh row under that constant and the only
one with neither a `bench` timing at 1 000 000 nodes nor a memory arm. The constant's own
comment records that six of the seven have one or both (`registry/three_d.rs:85-86`).

## Reproducing

Both chains are in `scripts/orch/rows/p12-3d.rows` and are already written out at the top of
this file. The SciGraphs core directory is passed **explicitly** to
`harness/oracle-closed-form.py` rather than left to the harness default: the two 3-D references
live in that package, and a path that resolved elsewhere would surface as an `ImportError`
naming neither the module nor the path.
