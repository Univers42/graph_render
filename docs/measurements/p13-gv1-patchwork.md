# `layout.treemap.patchwork` against Graphviz 16.1.0's own `patchwork`

Measured 2026-10-01 on the p13-gv1-patchwork tree, 1000 gate seeds (`n = 2 + seed % 600`, one
connected preferential-attachment graph each, `graph_core::seeded_model` at `REFERENCE_DEGREE`),
against the docker-only oracle image `ge-graphviz-oracle` (Graphviz 16.1.0, pinned by sha256 in
`scripts/orch/fetch-refs.sh`).

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine patchwork \
  --seeds 1000 --out target/spectral-fixtures
docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/spectral-fixtures patchwork target/gv-patchwork
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine patchwork \
  --fixtures target/spectral-fixtures --graphviz target/gv-patchwork
```

## Oracle determinism

The oracle is run twice over the same fixtures and `cmp`-ed, then re-run at two other seeds. The
engine name is a parameter of one command, so the seed sweep is the same harness with
`GM_GV_START` set.

```sh
docker run … python3 harness/oracle-graphviz.py target/spectral-fixtures patchwork target/gw-r1
docker run … python3 harness/oracle-graphviz.py target/spectral-fixtures patchwork target/gw-r2
cmp target/gw-r1/graphviz-patchwork.jsonl target/gw-r2/graphviz-patchwork.jsonl
docker run -e GM_GV_START=7  … python3 harness/oracle-graphviz.py … patchwork target/gw-7
docker run -e GM_GV_START=99 … python3 harness/oracle-graphviz.py … patchwork target/gw-99
sha256sum target/gw-{r1,r2,7,99}/graphviz-patchwork.jsonl
```

| run | `-Gstart` | sha256 of `graphviz-patchwork.jsonl` |
|---|---|---|
| first | 1 | `c0e8becec9e4e30bde9a0c7ab98fe2e72b09169cbc69cf088aece4c0dd91fca2` |
| second | 1 | `c0e8becec9e4e30bde9a0c7ab98fe2e72b09169cbc69cf088aece4c0dd91fca2` |
| seed sweep | 7 | `c0e8becec9e4e30bde9a0c7ab98fe2e72b09169cbc69cf088aece4c0dd91fca2` |
| seed sweep | 99 | `c0e8becec9e4e30bde9a0c7ab98fe2e72b09169cbc69cf088aece4c0dd91fca2` |

`cmp` over the two runs is silent (exit 0). **`-Gstart` is INERT for this engine, measured over
all 1000 seeds and not assumed**: the four runs above are the same bytes. Each sweep ran under
`GM_GV_START` and each manifest carries the `start` it ran with (`1`, `1`, `7`, `99`), so the
seed really was varied rather than the pinned default read twice. That is the expected result —
`patchwork` draws no random numbers at all — and it means a gap from Graphviz is an algorithmic
difference or nothing, never seed drift. The metadata says so, and the port therefore reproduces
the closed form rather than a seeded initial placement.

The `cmp` is not vacuous. Perturbing one node coordinate by 1e-6 points and re-comparing:

```sh
cp -r target/gv-r1 target/gw-perturb   # then rewrite seed 0's nodes.n0[0] += 1e-6
cmp target/gv-r1/graphviz-patchwork.jsonl target/gw-perturb/graphviz-patchwork.jsonl
```

gives `differ: byte 9, line 1` (exit 1).

## The differential

The metric is the largest absolute node-coordinate difference **in points**, after both arms are
rescaled onto the same bounding box: per seed, Graphviz's own node-centre bounding box is the
target and both arms are mapped onto it with one uniform scale. The comparison is against
Graphviz's output, never against a second native run.

| layout | cases | worst max abs coordinate gap (points) | ceiling |
|---|---|---|---|
| `layout.treemap.patchwork` | 1000 | 6.613e-2 | 1e-1 |

**The ceiling is 1e-1 and the gap is the oracle's own printed resolution, not a disagreement.**
`-Tplain` writes five significant digits, so one printed digit at the largest gate drawing is
0.001 inch = 0.072 points. 1e-1 is the next power of ten above the worst gap (6.613e-2), measured
and not widened; the evidence that the gap *is* that quantum and nothing else:

| | value |
|---|---|
| worst gap over 1000 seeds | 6.613e-2 points, at seed 569 (n=571), whose node-centre box is 725.5 pt across |
| median gap | 5.054e-3 points |
| 95th percentile | 3.339e-2 points |
| seeds above 1e-2 | 61 of 1000 |
| seeds exact to the printed digit | 4 of 1000 |
| largest gate drawing, node centres | 744.2 points = 10.337 inches across, so one printed digit is 0.001 inch = 0.072 points |
| ratio, worst gap to that printed quantum | 0.919 |

The extent quoted in both rows is **Graphviz's own node-centre bounding box**, which is what the
metric rescales onto, and it is *not* the closed-form field `sqrt(1000 * n)` = 755.6 points at
n=571 (775.2 at the largest n = 601): a node centre sits half a tile inside the field's edge, so
the centre box is about 4 % narrower than the field. The printed-digit quantum is read off the
centre box, because that is the magnitude `-Tplain` actually prints.

The five worst seeds are all in the largest drawings and their gap-to-extent ratio is flat to
within a factor of 1.07 across the top of the sweep (8.51e-5 … 9.12e-5 at seeds 557, 560, 566,
587, 569), which is what a formatter's resolution looks like and what an algorithmic difference
does not: an algorithmic difference would show up as a handful of seeds at O(1) drawing widths,
not as 61 seeds clustered inside a quantum that moves with the size of the drawing.

The same claim measured directly, on isolated graphs rather than the gate's models: Graphviz's own
reported extent against the exact field `sqrt(1000 * n)`, over `n = 1..100`.

Measured by writing one isolated `graph { n0; … n(n-1); }` per `n` in `1..100`, running
`patchwork -Tplain -Gstart=1` over each, and comparing the `graph` line's reported extent with the
exact field `sqrt(1000 * n)` — the same oracle, the same engine, no Rust in the loop.

| | value |
|---|---|
| worst extent vs field | 3.618e-3 points, at n=67 (field 258.844, printed 258.847) |
| worst 5 | n=67 3.618e-3 · n=50 3.602e-3 · n=83 3.594e-3 · n=91 3.537e-3 · n=19 3.512e-3 |

So Graphviz's own drawing extent already sits up to 3.6e-3 points off the closed form — the
reference opens with a root square of `sqrt(total + 0.1)` and insets the fill rectangle by the
margin term, which is `-Tplain`'s fifth digit moving. Byte-exact agreement against the oracle's
*text* is therefore not reachable and is **not** claimed. What is claimed is agreement at the
resolution the oracle carries.

## The closed cases, compared node by node

A 1e-1 ceiling is weaker than the truth five small graphs carry, so those are checked against the
closed form rather than against the tolerance — ours in
`crates/graph-core/src/layout/graphviz/patchwork/tests.rs` (exact multiples of `L = sqrt(1000*n)`,
pinned in Rust), Graphviz's own printed lines quoted here from
`patchwork -Tplain -Gstart=1` over one DOT file per case (`target/closed/case-{1..5}.dot`), the
printed lines quoted verbatim:

| case | Graphviz's printed lines (inches) | our arm | gap |
|---|---|---|---|
| one node | `n0 0.21961 0.21961` | pinned there | 0 |
| two nodes | `n0 0.15529 0.31057`, `n1 0.46586 0.31057` | pinned there | 0 |
| 3-path | `n0 0.19019 0.50716`, `n1 0.57055 0.50716`, `n2 0.38037 0.12679` | pinned there | 7.2e-4 |
| 4-cycle | `n0 0.21961 0.65881`, `n1 0.65881 0.65881`, `n2 0.21961 0.21961`, `n3 0.65881 0.21961` | pinned there | 0 |
| 5-star | `n0 0.24553 0.78568`, `n1 0.73657 0.78568`, `n2 0.16369 0.29463`, `n3 0.49105 0.29463`, `n4 0.81842 0.29463` | pinned there | 6.3e-4 |

Every case agrees node by node to within the printed quantum, and the *tiling* is identical — the
n=4 case is a 2x2 grid in both arms, n=5 is a row of two over a row of three, n=3 is a row of two
over one spanning square. Which node gets which square is the statement the row-closing order
makes, and it is the same order in both arms.

The five closed cases are **not** compared byte-for-byte the way twopi's six are, and the
difference is stated rather than papered over: for twopi the exact offset from the closed answer's
frame to Graphviz's is a half-node outside the node-centre bounding box, which makes the printed
strings reproducible from the closed form. For patchwork the same offset holds but the *extent*
carries the `sqrt(total + 0.1)` guard described above, so the printed digit moves by up to 3.6e-3
and the strings are not reproducible from the closed form. The comparison is therefore at the
printed precision — the same quantum the ceiling rests on — rather than on the strings.

## Scale

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.treemap.patchwork \
  --n 220,10000,100000,1000000 --repeat 3
```

Run twice on this host; `--repeat 3` medians, and the two runs differ by under 5 %:

| n | edges | run 1 | run 2 | per node (run 2) |
|---|---|---|---|---|
| 220 | 329 | 0.01 ms | 0.00 ms | — |
| 10 000 | 15 474 | 0.12 ms | 0.12 ms | 0.01 us |
| 100 000 | 154 978 | 1.96 ms | 1.88 ms | 0.02 us |
| 1 000 000 | 1 549 929 | 20.58 ms | 21.25 ms | 0.02 us |

A flat ~20 ns per node from 10 000 nodes up, and a decade-to-decade factor of ~10 (1.88 ms to
21.25 ms), which is the `O(n)` in `complexity` showing up in the timings. The layout reads no
edge at all, so there is no `m` term and the per-node cost is flat where a layout that read edges
would show one. The 220-node row is below the bench's own resolution and carries no claim.

1 000 000 is the largest size `bench` accepts and where it was run, **not** where it was found to
stop working, so `PATCHWORK_CEILING` is a measured lower bound.

## Hash gate

```sh
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
scripts/orch/gr -e GM_MUTATE_PATCHWORK_NODES=1 cargo run -q -p graph-cli -- hashgate --seeds 8
```

The gate passes with the stage registered (`layout.treemap.patchwork: 4-way equal on 8/8
seeds`, out of 41 stages). The negative control exits 1 and, read by stage name,
`DIVERGED layout.treemap.patchwork` is the only stage it names — the same shape as the five other
node controls, which re-draw one stage's own model and must move that stage alone.

## The negative controls of the differential itself

`oracle-graphviz` refuses rather than reading a partial comparison as a pass:

| what is wrong | refusal |
|---|---|
| no recorded run for the engine (`--engine circo`) | exit 2, `circo: no Graphviz differential; this build has patchwork` |
| a fixtures directory with no manifest | exit 2, naming the path |
| the oracle's manifest sha256 does not match the file beside it | `the oracle's output is not the run its manifest records` |
| the fixtures and the tree are not the same tree | `the fixtures and the tree are not the same tree: re-emit` |
| the two arms hold different numbers of seeds | `1 arm(s) ran past the other: re-run the harness` |

## Reproducing the distribution tables

`oracle-graphviz --engine patchwork` prints the case count, the worst gap, the ceiling and how many
seeds were exact — the first row of this file. The distribution behind those four numbers (the
median, the 95th percentile, the per-seed gap-to-extent ratio, and the closed-case gaps) is a
read of the same two arm files, `target/spectral-fixtures/patchwork.jsonl` and
`target/gw-r1/graphviz-patchwork.jsonl`, applying the rescale and gap the Rust `check`
applies (`target/gapdist.mjs`, and `target/closedgap.mjs` for the five closed cases). It was
computed with a throwaway script under `target/` rather than a committed one,
because the ceiling is a single number the checked command prints and the distribution is
corroboration for it, not a gate; `target/` is unversioned, so nothing here is left pointing at a
file a reader cannot open.

The oracle's own closed-form extent sweep (3.618e-3 points) needs no Rust at all: it is
`patchwork -Tplain -Gstart=1` over `n = 1..100` isolated DOT files written to `target/iso/`,
comparing each `graph` line's extent with `sqrt(1000 * n)` (`target/iso-extent.txt`).
