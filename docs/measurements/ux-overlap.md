# Node overlap removal (`post.separate.grid`) — quality, cost, and the Graphviz ceiling

Measured 2026-10-03 on branch `ux-overlap` at `1126257`. Host dlesieur42: 20 cores, i5-13600KF,
Linux 6.17, `ge-rust` image, rustc 1.98.1, `--release`. Every number below is from
`graph-cli overlap`, which prints the pass's own time and the invariant check's verdict in one
line so they cannot disagree about what was measured.

## 1. The input, and why it is deliberately the hard one

`--nodes N` is the hairball generator at seed 1, laid out by `layout.grid`, re-declared as
`Circle` nodes of radius **1.0** — a layout emits `Point` centres, which have no extent, and the
pass is a documented no-op on one.

`layout.grid` pitches nodes exactly 1 unit apart, so radius 1.0 makes every node overlap its four
orthogonal *and* four diagonal neighbours: 3 811 overlapping pairs at `n = 1 000`, a drawing four
times too crowded to pack. That is the worst case for a sweep, and the point of measuring it:
a radius below 0.5 produces a lattice with no overlaps at all and the pass has nothing to do.

Ponytail (timing): one run per row, no warm-up and no repeat, on a host that was not idle, so
`pass ms` and `wall` are single samples. `wall` is the whole command; `pass ms` is the pass alone.

## 2. Quality and cost at 1 000 / 10 000 / 100 000

`graph-cli overlap --nodes N --layout layout.grid --radius 1.0 [--no-scan]`, default
`max_iterations = 512`.

| nodes | overlapping before | still overlapping | mean displacement | ÷ mean NN distance | stress before → after | pass ms | wall |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 000 | 3 811 | **0** | 14.786 | 14.79 | 0.0576 → 0.0315 | 33.0 | 0.03 s |
| 2 000 | 7 733 | **0** | 20.884 | 20.88 | 0.0473 → 0.0248 | 95.1 | 0.10 s |
| 10 000 | scan refused | **16 294** | 35.618 | 35.62 | 0.0289 → 0.0170 | 734.3 | 0.75 s |
| 100 000 | scan refused | **383 572** | 46.114 | 46.11 | 0.0092 → 0.0081 | 10 487.3 | 10.69 s |

- `overlapping before` is the exhaustive `O(n^2)` scan and `still overlapping` is the same scan
  afterwards; both are exact and the command exits non-zero when the second is not 0. Above 2 000
  nodes the scan is refused (`BRUTE_FORCE_CEILING`) because it would cost more than the pass it
  checks, and the pass's own grid count stands in — the two bracket one quantity and agree at
  every size where both run, which is what makes the large-`n` column worth reading.
- **÷ mean NN distance** normalises by the drawing's own mean nearest-neighbour distance, so the
  displacement is "how far a node moved, in units of the local spacing". It is 1.000 by
  construction here because the lattice pitch *is* 1.0; the column is what makes the number
  comparable to §4, where the spacing is not 1.
- **Stress below 1 is normal** for a dense drawing: it is hops over layout length, and a hairball
  layout draws most edges far shorter than one hop. It moves the right way — 0.0576 → 0.0315 is
  edge lengths growing 1.8×, which is what pushing overlapping nodes apart has to do. The cost
  side is visible too: separating a 1 000-node drawing compresses the drawing's faithfulness,
  and at 100 000 nodes the ratio barely moves (0.0092 → 0.0081) because the pass leaves most of
  the drawing alone.

## 3. The iteration cap is what binds above 2 000 nodes

`SeparateParams::max_iterations` is 512, sized from a lattice of at most 2 000 nodes (the hardest
measured case needs 448). Above that the default does not converge inside the cap and the residue
is reported rather than hidden: at 10 000 it is 16 294 pairs, 0.3 % of the 5 042 600 it separated;
at 100 000, 383 572 of 51 057 153. Both are `Bundled::unbundled`, i.e. the pass's own count over
the same 3 × 3 neighbourhood the scan uses.

The escape hatch is `--max-iterations`, and it is a parameter the pass validates rather than a
silent quality dial: `0` is refused by name (`StageError::Param`, no new variant).

## 4. The quality differential against Graphviz

**A ceiling, not a contest.** Graphviz's PRISM stress-majorises on a Delaunay triangulation;
this is a uniform-grid Jacobi sweep. They will never agree coordinate for coordinate
(`docs/decisions/node-overlap.md` §8), so what is compared is quality: overlapping pairs cleared,
and displacement measured in units of the drawing's own spacing.

Engine: `ge-graphviz-oracle-gts`, pinned positions (`pos="x,y!"`) in a box at 0.35 packing
fraction, node radius 0.3. Ours: `layout.grid`, radius 0.55 and 1.0. Both sides count overlaps
with the same exhaustive `O(n²)` predicate — pairs closer than the sum of the radii — and the
Graphviz side ships a control (`-Goverlap=true`, "leave the drawing alone") that must leave the
input's overlaps untouched or the row is void. It does: 100 → 100 at `n = 150`, 723 → 723 at
`n = 1 000`.

| | input overlapping pairs | pairs/node | mean NN | PRISM: pairs after | PRISM displacement | in × mean NN |
|---|---:|---:|---:|---:|---:|---:|
| Graphviz, n = 150 | 100 | 0.67 | 0.487 | **0** | 15.177 | 31.2 |
| ours, n = 150, r = 0.55 | 275 | 1.83 | 1.000 | **0** | 0.421 | **0.42** |
| ours, n = 150, r = 1.00 | 528 | 3.52 | 1.000 | **0** | 5.531 | **5.53** |
| Graphviz, n = 1 000 | 723 | 0.72 | 0.449 | **0** | 39.998 | 89.1 |
| ours, n = 1 000, r = 0.55 | 1 936 | 1.94 | 1.000 | **0** | 1.062 | **1.06** |
| ours, n = 1 000, r = 1.00 | 3 811 | 3.81 | 1.000 | **0** | 14.786 | **14.79** |

Both engines reach zero overlapping pairs. PRISM reaches it by moving nodes 31–89× the local
spacing; this pass reaches it by moving them 0.4–15×. **That gap is a difference in what the two
algorithms are for**, not a defect in either: PRISM preserves relative distances, so on a drawing
that is four times too crowded it expands the whole picture, while a Jacobi sweep pushes each
overlapping pair apart and leaves the rest of the drawing where the layout put it. If a caller
wants Graphviz's behaviour — a uniformly expanded drawing rather than a locally repaired one —
this pass is not that algorithm, and §4 of the decision doc is where that was decided.

Other Graphviz modes on the same input, for scale: `voronoi` moves 1.12 (n = 150) and 7.52
(n = 1 000), `scale` moves 47.57 and 722.71. All three modes clear the overlap; none of them is
the one being compared against.

## 5. What this does not say

- **The two inputs are not the same object.** Ours is a lattice, theirs a random cloud, and ours
  is 2.7–5.3× more crowded in pairs per node. A row-for-row contest would need a lattice Graphviz
  lays out, which `-Goverlap` does not do — it adjusts an existing drawing.
- **Only n = 1 000 and 2 000 have an exact invariant here.** Past the brute-force ceiling the
  residual is the pass's own grid count, which the scan agreeing at small `n` supports but does
  not prove.
- **Timings are one run on a loaded host**, and the pass is `--release` on x86_64; wasm32 is
  bit-identical in *output* (hashgate, D1) but not measured for time.
- **`radius = 1.0` is a choice, not a property.** It is four times denser than a packable drawing,
  which is what makes the lattice the binding case; a caller who separates a normal drawing
  spends far less displacement for the same invariant.

## Reproducing

    scripts/orch/gr cargo build --release -p graph-cli
    ./target/release/graph-cli overlap --nodes 1000 --layout layout.grid --radius 1.0
    ./target/release/graph-cli overlap --nodes 10000 --layout layout.grid --radius 1.0 --no-scan

    docker run --rm --user 0:0 -v $PWD:/w -w /w -e GM_SCRATCH=/w/target \
        ge-graphviz-oracle-gts python3 harness/gv_overlap.py --nodes 150 --seed 7 \
        --out target/gv-overlap-150.json          # exits 0

    docker run --rm --user 0:0 -v $PWD:/w -w /w -e GM_SCRATCH=/w/target \
        ge-graphviz-oracle python3 harness/gv_overlap.py --nodes 150 --seed 7 \
        --out target/gv-overlap-control-150.json  # exits 1: the negative control