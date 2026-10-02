# `SPIRAL_3D`: SciGraphs' conical 3D spiral, ported

The row that was mapped to the wrong layout. `SPIRAL_3D` was compared against
`layout.spiral` — graph-core's planar Archimedean spiral, a port of networkx's
`spiral_layout` at `resolution = 0.35` — and measured **0/1020 on both axes** with a
Procrustes disparity of 0.585. The picture said why: *grey is a 3D spiral, green one point at
the centre*.

SciGraphs has no 2D spiral. `apply_graph_layout` routes `SPIRAL_3D` to its own
`_spiral_layout_3d` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63`) through
`layouts/dispatcher.py:105-106`, and to nothing else. So no value of `resolution` could
have moved the row, and the conformance doc's original repair 10 was wrong on both counts.
`layout.spiral` is untouched by this job.

## What was measured, and on what

The same 24 fixtures the whole matrix uses (340 nodes, 1020 coordinates per row, 23 of the
24 compared), SciGraphs' own `apply_graph_layout` at `scale = 5.0`, in the pinned
`ge-python-oracle` image (numpy 2.3.3) with the `SciGraphs/` submodule at
`b7ccee691e368425ee3154681c43fd163e572edb`.

```sh
scripts/orch/gr cargo build --release -p graph-cli
scripts/scigraphs-conformance.sh                       # baseline, untouched tree: exit 0, PASS
scripts/scigraphs-conformance.sh                       # after the port: exit 1, SPIRAL_3D only
scripts/scigraphs-conformance.sh                       # after re-pinning the row: exit 0, PASS
scripts/scigraphs-conformance.sh --break               # negative control: exit 1, SPRING_3D
```

## Before and after

| | motor id | tier | f64 k/N | f32 k/N | max ULP | max gap | Proc. median | Proc. max | cause |
|---|---|---|---|---|---|---|---|---|---|
| before | `layout.spiral` | `shape` | 0/1020 | 0/1020 | 9.22e+18 | 6 | 0.585 | 0.815 | `algorithm` |
| after | `layout.basic3d.spiral` | `tolerance` | 120/1020 | **1020/1020** | 2.68e+08 | 2.35e-07 | 3.34e-16 | 5.59e-16 | `arithmetic` |

22 of 24 fixtures overlay; the two that do not (`gate-00` at 2 nodes, `gate-01` at 3) are
below the Procrustes fit's own noise floor. `layout.spiral`'s own conformance state is
untouched — it is still networkx's curve, still `resolution = 0.35`, still its own row's
business.

**`f32 1020/1020` is the strongest statement this tree can make**, and it is the same place
`SPHERE`, `HELIX` and `HIERARCHICAL_3D` sit. `f64 120/1020` is not a shortfall in the port:
the motor arm writes its `.f64` file as the `f32` widened, so that column answers "was the
reference's own value already `f32`-representable", which is a property of numpy's output,
not of this tree. That the two counts differ is the tier rule's own argument, restated here
by measurement.

## The four numpy primitives that had to be ported, and how each was pinned

`t` is the only value in this layout that is not closed form, and reproducing it means
reproducing four numpy behaviours that each differ from the obvious translation. **numpy
2.3.3's C source is not in `$GM_SCRATCH/refs`**, so each was pinned by *reproduction* — the
candidate formula run against numpy and compared over the IEEE-754 bits — rather than by
reading `compiled_base.c`. That is a stronger pin than a citation for the thing that actually
matters, and it is the only route available offline.

| primitive | what it does | how it was pinned | result |
|---|---|---|---|
| `linspace` | `start + i*step`, last element **overwritten** with `stop` | all 65 536 grid points, bit for bit | `step = 0x3ef0001000100010` (= `1/65535`); `grid[65535] = 1.0` exactly |
| `cumsum` | **sequential**: `length[i] = length[i-1] + step` | folded into the `t` sweep below | 65 535 additions in index order |
| `interp` | `slope = (fp[j+1]-fp[j])/(xp[j+1]-xp[j])`, then `slope*(x-xp[j]) + fp[j]` | 200 000 unrelated monotone points, and every node count 1..64 plus 77, 100, 101, 600, 601 | **0 ulp** of `np.interp` on all of them |
| `round` | half-to-**even** (`float.__round__`, `np.float64.__round__`) | `turns` pinned at n = 1..16, 77, 200, 256, 512 | floor at 2 for every `n <= 14` |

The `t` sweep is the load-bearing one: it recomputes the whole inversion with the recovered
formulas and compares the resulting `t` column against `np.interp`'s, bit for bit, for 68
node counts. **Zero mismatches.** The pinned `t` and `z` words in
`crates/graph-core/src/layout/basic_3d/spiral/tests/reference.rs` are therefore the
reference's, not a transcription of this port's own output.

`np.interp`'s `j == lenxp - 1` case returns `fp[j]` **without a slope**, which is why
`interp`'s top-end guard is `>=` rather than `>`. Both paths give `1.0` here, so the choice
is invisible in the numbers and load-bearing in the code.

## What is not reachable, and why the tier is `tolerance`

`x` and `y` are `libm`'s `sin`/`cos` (D1) against numpy's array loops — two implementations,
two answers. Measured **within 1 `f64` ulp** at n = 1, 2 and 7, and it vanishes entirely in
the `f32` narrowing, which is why the row reaches 1020/1020 rather than stopping short. The
`f64` disagreement is not what stops this row at `tolerance`; the fact that the motor is `f32`
end to end is. `SPHERE` and `HELIX` are in exactly the same position and carry the same tier.

One number is worth stating because it looks like a bug and is not: the head node's `y` is
`-2.4492935982947065e-15`, not zero. That is `5.0 * sin(4*pi)` — `sin` of the nearest `f64`
to `4*pi` is `-4.898587196589413e-16` on **both** sides (`np.sin` and libm agree exactly
here), and the radius multiplies it. Small, non-zero, and reproducible.

## The two gaps, closed

`G_NO_ITERATIONS` and `G_SNAPSHOT_SCALE` came off the row and `G_BASIC3D_SCALE` went on,
which is what its three siblings (`SPHERE`, `HELIX`, `CUBE`) already carry. Both dropped
consts cite `at:` paths this row no longer touches — `G_NO_ITERATIONS` names
`layout/spiral.rs:63`, the planar networkx spiral, and `G_SNAPSHOT_SCALE` names the same
`basic_3d.rs:43` constant that `G_BASIC3D_SCALE` names more specifically. **Both consts stay
in `gaps.rs`**: other rows still name them, and a const left behind once nothing names it is
a false record in the other direction.

`G_BASIC3D_SCALE`'s note gained `spiral` in its list of the placements it covers. It remains
true that `scale` is a const rather than a parameter — the reference takes one `scale` with
no default of its own and no SciGraphs caller passes anything else, so a struct would be a
knob with one setting.

## Commands and exit codes

```text
scripts/orch/gr cargo build --release -p graph-cli                 -> 0
scripts/scigraphs-conformance.sh    (untouched tree)               -> 0   PASS, 32/32 rows
scripts/scigraphs-conformance.sh    (after the port)               -> 1   SPIRAL_3D: FAIL — motor
                                                                      bytes are not the pinned
                                                                      ones; no other row moved
scripts/scigraphs-conformance.sh    (after re-pinning)             -> 0   PASS
scripts/scigraphs-conformance.sh --break                          -> 1   --break caught: SPRING_3D
scripts/orch/gr cargo run -q --release -p graph-cli -- capabilities --check   -> 0
scripts/orch/gr cargo run -q --release -p graph-cli -- codegen --check         -> 0
scripts/orch/gr cargo test -p graph-core --lib                              -> 0   1059 passed
```

`capabilities --check` reports 70 rows (36 layouts plus the rest of the ledger) and **no
problem names `layout.basic3d.spiral`**; the 36 problems it does print are the pre-existing
"gated, but no `<record>`: run the gate" lines, which need the orchestrator's gate run.
`codegen --check` reports all four generated files up to date: the four outputs are the
snapshot-header schema and its `.d.ts`, and the two `docs/contract/` schemas, and **none of
them is layout-specific**, so a new `layout.basic3d.*` id produces no codegen diff. That was
predicted before the change and confirmed by it.

## Ponytail (what this layout is bad at)

Two, both recorded in the ledger's `ponytail` field with the failing input and the direction:

- **The turn count is floored at 2 for every `n <= 14`.** `sqrt(n/(0.75*pi))` is below 2
  there and `max(2, ...)` takes over, so the first fourteen node counts all draw a two-turn
  spiral and the "gap between successive turns close to the spacing along the curve" the
  reference's own docstring claims is not what happens at the sizes a reader tries first.
  Wrong-but-plausible and silent: still a spiral, still exactly on the reference's cone.
- **512 KiB and 65 535 sequential additions per call, for an `O(n)` output.** Reference-faithful
  and not necessary — a closed-form conic arc-length integral would be faster, would stop
  agreeing with SciGraphs' own grid, and is therefore a different layout id rather than a
  faster build of this one. Failing input: any caller running this at `n = 2` in a loop.

No seed is owed and none is published: the function draws no random number at all.

## Files

- `crates/graph-core/src/layout/basic_3d/spiral.rs`, `spiral/tests.rs`,
  `spiral/tests/{reference,structure}.rs` — the port and its tests
- `crates/graph-core/src/layout/basic_3d.rs` — the `spiral` wrapper, the module, the doc
- `crates/graph-core/src/registry/three_d/{,basic,spiral3d,graph}.rs` — the `SPIRAL_3D`
  `Metadata`, and the split that keeps every file under 300 lines
- `crates/graph-core/src/registry.rs` — the appended `Capability`, `LAYOUTS: 35 -> 36`
- `crates/graph-cli/src/oracle_python/conformance/{rows,gaps}.rs`, `baseline/table/networkx.rs`
- `crates/graph-cli/src/capabilities/registry/unproven.rs`, `capabilities/tests/registry/ids.rs`
- `crates/graph-cli/src/snapshot_cmd/tests.rs` — the id list, which enumerates every
  registered layout
- `docs/measurements/scigraphs-conformance.md` — matrix row 14 and repair 10