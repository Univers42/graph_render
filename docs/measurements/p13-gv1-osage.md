# layout.packing.osage — Graphviz `osage`, and where the agreement ends

`osage` packs a flat graph into a near-square array of node boxes, filled row by row in the
order the nodes were declared. It is closed form: no iteration, no force model, no seed.
The port is `crates/graph-core/src/layout/graphviz/osage.rs`, the metadata is
`crates/graph-core/src/registry/graphviz_osage.rs`, and the differential is
`graph-cli oracle-graphviz --engine osage` over `harness/oracle-graphviz.py --differential`.

**The headline: the two arms do not agree, and this file says so with the numbers rather
than behind a ceiling.** They agree to within the oracle's own printed quantum — 3.6e-3
points — on the 18 of 1000 seeds where every node box ties, and they differ by 122 to
1785 points on the other 982, where they are two different drawings. Two named causes,
both measured, neither an iteration and neither a tolerance. The row is
`Status::Implemented` and never `gated`.

## The oracle is deterministic

Run twice over the same 1000 fixtures and `cmp`-ed, in the shape the ADR's determinism
evidence uses:

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine osage --seeds 1000
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/osage-fixtures osage target/gv-osage-a --fixtures=osage.jsonl
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/osage-fixtures osage target/gv-osage-b --fixtures=osage.jsonl
cmp target/gv-osage-a/graphviz-osage.jsonl target/gv-osage-b/graphviz-osage.jsonl
```

`cmp` is silent: **byte-identical across two runs**, 1000 seeds, exit 0.

## `-Gstart` is inert for this engine

The same fixtures at three seeds, byte-compared:

| `-Gstart` | result |
|---|---|
| 1 | the reference run |
| 7 | identical to start=1, `cmp` silent |
| 99 | identical to start=1, `cmp` silent |

```sh
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/osage-fixtures osage target/gv-osage-s7 --fixtures=osage.jsonl --start=7
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/osage-fixtures osage target/gv-osage-s99 --fixtures=osage.jsonl --start=99
cmp target/gv-osage/graphviz-osage.jsonl target/gv-osage-s7/graphviz-osage.jsonl
cmp target/gv-osage/graphviz-osage.jsonl target/gv-osage-s99/graphviz-osage.jsonl
```

So none of the disagreement below is seed drift. `osage` reads `start` nowhere: the engine
is `layout()` in `lib/osage/osageinit.c` plus `arrayRects` in `lib/pack/pack.c`, and
neither takes a random seed.

### The `cmp` is not vacuous

One node coordinate of one seed, moved by 1e-6 points — four orders of magnitude below
anything the layout computes:

```sh
python3 - <<'EOF'
import json
p = 'target/gv-osage-a/graphviz-osage.jsonl'
rows = [json.loads(l) for l in open(p)]
rows[94]['nodes']['n0'][0] += 1e-6
open('target/gv-osage-perturbed/graphviz-osage.jsonl', 'w').write(
    '\n'.join(json.dumps(r) for r in rows) + '\n')
EOF
cmp target/gv-osage-a/graphviz-osage.jsonl target/gv-osage-perturbed/graphviz-osage.jsonl
```

`cmp` exits 1: **differ: byte 149645, line 95**. This is the `osage-oracle-perturb-negctl`
row in `scripts/orch/rows/p13-gv1-osage.rows`, run there over 20 seeds so it is cheap.

## The differential

The metric is the largest absolute node-coordinate difference in points, after both arms
are rescaled onto one bounding box — imported from `harness/oracle-twopi.py`, not copied,
so both Graphviz differentials are measured by the same definition.

| layout | cases | worst max abs coordinate gap (points) | ceiling |
|---|---|---|---|
| `layout.packing.osage` | 1000 | 1.785e+3 | 1e+4 |

The ceiling is the next power of ten above the worst measured gap, which is the rule; it
was **not** chosen to make the row pass, and it does not make the row mean anything it does
not already say. Worst gap 1785.2 points at seed 584 (n = 586).

### The distribution is the finding

| seeds | n | gap (points) | what it is |
|---|---|---|---|
| 18 of 1000 | 2 … 10 | 0 … 3.6e-3 | every node box ties — the oracle's printed quantum, not an algorithmic difference |
| 982 of 1000 | 11 … 601 | 122 … 1785 | two different drawings |
| median over the sweep | — | 967.2 | — |

There is no middle ground and no tail: the port is right or it is a different picture, and
which side of the line a seed falls on is decided entirely by `n`.

## The closed cases, compared byte for byte

Six graphs whose layout has an analytically determined answer — one node, two nodes, a
3-path, a 4-cycle, a 5-star and a 6-branch — are compared on the **strings `-Tplain` itself
prints**, not on parsed floats, at the plain format's own five significant digits. Graphviz's
arm is in `harness/oracle-graphviz.py`'s `OSAGE_CLOSED`, ours node by node in
`crates/graph-core/src/layout/graphviz/osage/tests.rs`.

| case | nodes | byte-exact |
|---|---|---|
| one-node | 1 | yes |
| two-nodes | 2 | yes |
| three-path | 3 | yes |
| four-cycle | 4 | yes |
| five-star | 5 | yes |
| six-branch | 6 | yes |

All six match. **All six are below eleven nodes**, which is the whole of why: that is the
region where the port claims exactness, and it is exact there. The harness exits non-zero if
any closed case disagrees, and `graph-cli oracle-graphviz --engine osage` fails the run if
`closed_exact` is false — so this is a gate, not a note.

The port keeps the reference's own translation onto the origin (`osageinit.c:198-220`), so
no offset is applied on either side: `n0` of a one-node graph is `(27, 18)` points =
`0.375 0.25` inch on both sides, which is exactly what `-Tplain` prints.

## The two named causes

### 1. Graphviz sizes a node's box from its rendered label

`osageinit.c:124` packs `{.UR = {.x = ND_xsize(n), .y = ND_ysize(n)}}`, and
`gv_nodesize` returns the `nodesize` minimum only while the label fits inside it. Measured
on the oracle, one node per graph, label length against `-Tplain`'s reported width:

| label | box width (points) |
|---|---|
| `n`, `nn` | 54.000 (the 0.75 inch minimum) |
| `nnn` | 57.942 |
| `nnnn` | 70.358 |
| `nnnnn` | 82.774 |

so the gate's own fixtures — labelled `n0` … `n{n-1}` — have 54 point boxes below `n10` and
**57.942** point boxes from `n10` on. `arrayRects` then takes each column's cell width from
the widest box in it (`pack.c:665`), so from n = 11 the cell grid is no longer uniform.
The 0.75 inch default is the *minimum*, not the answer, and the amount above it is a font
metric of Graphviz's own text layout.

The port takes every box to be the default 54 x 36, which is exact exactly while the labels
fit — that is, below eleven nodes.

### 2. `arrayRects` sorts the boxes with a `qsort` that is not stable

`pack.c:658` orders the rectangles by `width + height` before filling the grid, and
comparing boxes of equal size returns 0, so which node lands in which cell among tied boxes
is glibc's choice. Measured on n = 14 (`nc = 4`, `nr = 4`), the reference's own rows:

| row (top to bottom) | nodes |
|---|---|
| 0 | `n10 n11 n12 n13` — the four 57.942-wide boxes, sorted first |
| 1 | `n0 n1 n2 n3` |
| 2 | `n4 n5 n6 n7` |
| 3 | `n8 n9` |

Below eleven nodes every box ties *and* they are all the same size, so the sort cannot move
the geometry — only the names — and the names are in declaration order at every size
measured. From n = 11 the wide boxes sort ahead of the narrow ones and whole rows move.

### Why neither is worth closing here

Both would have to be reproduced, not tolerated. (1) needs Graphviz's text metrics and a
font engine; graph-core has no I/O, no font and no label layout, and this stage emits
`Point` geometry with no box at all, so there is nothing on the motor side to size a box
from. (2) needs glibc's introsort, which is a C library implementation detail, not an
algorithm. Reimplementing either would buy a number on this gate and cost a layout nobody
would draw that way.

## Scale

`scale_ceiling` is 1 000 000, **measured** — `--release`, `--repeat 3` medians on one host:

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.packing.osage \
    --n 220,10000,100000,1000000 --past-ceiling --repeat 3
```

| nodes | edges | median |
|---|---|---|
| 220 | 329 | 0.00 ms |
| 10 000 | 15 474 | 0.03 ms |
| 100 000 | 154 978 | 1.00 ms |
| 1 000 000 | 1 549 929 | 8.94 ms |

A flat ~9 ns per node, and the `O(n)` in `complexity` is what the timings show: one `sqrt`
for the grid and one ordered pass over the node count. The edges are never read, which is
why `m` does not appear at all. 1 000 000 is the largest size `bench` accepts and where it
was run, not where it was found to stop working — a measured lower bound.

## Negative controls

| control | command | expected | measured |
|---|---|---|---|
| oracle output is read to the last digit | `osage-oracle-perturb-negctl` — two runs of the same 20 fixtures, one coordinate moved 1e-6 points, then `cmp` | nonzero | 1 |
| …and that control is not vacuous either | the same two runs with the perturbation **deleted** | 0 | 0 |
| the check cannot pass on nothing | `graph-cli oracle-graphviz --engine osage --dir target/osage-fixtures-absent` | nonzero | 2 |
| a knob the gate already had still turns it red | `GM_MUTATE_TWOPI_NODES=1 … hashgate --seeds 8` | nonzero | 1 |
| a shared knob still turns it red | `GM_MUTATE_REFERENCE_DEGREE=9 … hashgate --seeds 8` | nonzero | 1 |

The second row is what keeps the first honest. An earlier draft of the control compared a
20-seed output against the 1000-seed one; it exited non-zero with the perturbation deleted,
because the seed counts differ — a control that cannot fail. Both sides now come from the
same fixture set, and the unperturbed pair is asserted identical, so the non-zero exit can
only come from the 1e-6 point move.

**There is no `negctl-osage-nodes` row, on purpose.** The per-stage knob for a Graphviz
engine re-draws that stage's own model (`hashgate/knob/twopi.rs` is the precedent), and
`hashgate/knob.rs` is shared with the five parallel Graphviz engine jobs — five agents
editing one enum is a merge collision on a gate file. The two osage controls above sit
where they can be added without touching it, and they are the ones that matter here: this
engine disagrees with its oracle by construction, so what has to be shown is that the
comparison sees the oracle's own output and cannot pass vacuously.

### A limitation of the check itself, found while writing the control

`verdict()` compares the fixture manifest's `sha256` against the *result's* recorded
`sha256`, not against the fixture file on disk, so editing `osage.jsonl` after the harness
has run is not caught. That is a pre-existing property of the shared three-step driver
(`oracle_python.rs:143`), not something this job changed, and it is recorded here rather
than fixed because `oracle_python.rs` is outside this job's paths and the fix belongs to
whoever touches that driver next. It is why control 1 perturbs the *recorded oracle output*
and re-compares, rather than perturbing the fixtures and expecting the check to notice.

## Ponytail (this differential)

- **The ceiling is a record of a disagreement, not a tolerance we believe.** 1e+4 points is
  the next power of ten above 1785. The row it gates is `implemented`, and the numbers that
  matter are the 18 exact seeds and the six byte-exact closed cases, not the 982.
- **`-Tplain` prints five significant digits**, so even where the arms agree, 3.6e-3 points
  at these drawing sizes is the oracle's own quantum rather than our arithmetic. The closed
  cases are compared on the printed strings for exactly this reason.
- **The harness imports the metric from `harness/oracle-twopi.py` by path**, because that
  filename is not importable by name and a copy of the metric would be a second definition
  of the number the ceiling is measured against. `sys.dont_write_bytecode` keeps the
  fingerprinted `harness/` tree free of the `__pycache__` a path import writes.
- **`harness/oracle-graphviz.py` gained `--start=`, `--fixtures=` and `--differential`**, all
  optional; the three-argument call the twopi and circo determinism evidence used is
  unchanged, and `harness/oracle-twopi.py` is untouched and still works.
