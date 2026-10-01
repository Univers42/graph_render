# layout.packing.osage — Graphviz `osage`, and where the agreement ends

`osage` packs a flat graph into a near-square array of node boxes, filled row by row in the
order the nodes were declared. It is closed form: no iteration, no force model, no seed.
The port is `crates/graph-core/src/layout/graphviz/osage.rs`, the metadata is
`crates/graph-core/src/registry/graphviz_osage.rs`, and the differential is
`graph-cli oracle-graphviz --engine osage` over `harness/oracle-graphviz.py --differential`.

**The headline: the two arms agree, to the resolution the oracle is able to print.** The
worst gap over the 1000 gate seeds is **6.31e-2 points** at seed 574, and `-Tplain` writes
five significant digits, so one printed digit at that drawing's size is 0.001 inch = **0.072
points**: the worst gap is 0.88 of a single digit, the median is 3.99e-2, and **no seed's gap
reaches one cell stride** (58 points), which an algorithmic difference would.

**That was not always true, and the change is the point of this revision.** The arms used to
agree on only the 18 of 1000 seeds where every node box tied (n ≤ 10, gap ≤ 3.6e-3) and to
differ by 122 to 1785 points on the other 982, where they were two different drawings. Both
named causes were in the *fixture*, not in the arithmetic, and both are now removed at the
fixture: every node carries an explicit box that the DOT pins, and no two boxes tie on the
key `arrayRects` sorts by. Before: worst gap 1.785e3 at ceiling 1e4. After: worst gap
6.31e-2 at ceiling **1e-1** — five orders of magnitude tighter, and **no ceiling was widened
to get there**.

The row's `Status` is still `Implemented`. That is a reader this job's paths do not reach,
and the reason is measured rather than asserted: see
[## Why the row is not `gated`](#why-the-row-is-not-gated) below.

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
| `layout.packing.osage` | 1000 | 6.309e-2 | 1e-1 |

The ceiling is the next power of ten above the worst measured gap, which is the rule; it
was **not** chosen to make the row pass. Worst gap **0.0631 points at seed 574 (n = 576)**.
The previous figure for this row was 1.785e+3 at ceiling 1e+4, so the ceiling came down by
five orders of magnitude on a measurement rather than by a decision.

### The distribution, and why the ceiling is what it is

| seeds | n | gap (points) | what it is |
|---|---|---|---|
| min over the sweep | 601 | 0 | a node coordinate that landed on a printed digit |
| median over the sweep | — | 3.990e-2 | — |
| p90 over the sweep | — | 5.583e-2 | — |
| max over the sweep | 576 | 6.309e-2 | 0.88 of one printed digit |
| above 1e-2 | 714 of 1000 | — | — |

The gaps are **not** a tail with a body: the worst is 0.88 of one printed digit and nothing
approaches one cell stride. Two facts make the ceiling a claim about the oracle's formatter
rather than about the layout, and both are measured rather than argued:

- **the gaps track the drawing's extent.** The five worst seeds all draw 18.94 inches across,
  where `%.5g` puts the last digit at 0.001 inch = 0.072 points; the ratio of each seed's gap
  to *its own* printed digit is at most **0.883**, over the whole sweep.
- **no gap is a whole cell.** One cell stride is 58 points. An algorithmic difference — a
  node in the wrong cell, a column one box too wide — costs at least that. The maximum over
  1000 seeds is 0.0631, so no seed is in the wrong cell and no column is the wrong width.

The oracle side of that second point is the `negctl-osage-sizes` row: perturbing **one** node's
pinned width moves the drawing by 58.0 points, a full cell stride, which is what a real
disagreement looks like on this metric. The honest gap is three orders of magnitude smaller.

### The fixture sizes, and why there is one generator

Every node's box is generated **once**, in `crates/graph-core/src/layout/graphviz/osage/sizes.rs`,
and read twice: by `osage::run_sized`, which packs it, and by the fixture's `box` column,
which `harness/oracle-graphviz.py` turns into the DOT. The two arms cannot disagree about a
box without the differential going red, and `negctl-osage-sizes` proves it does.

| node `i` | width (points) | height (points) | `width + height` |
|---|---|---|---|
| `i % 8 == r`, `i / 8 == s` | `54 + r·9/256` | `36 + s·18/8` | `90 + i·9/256` |

Two properties carry the whole change, and both are pinned by tests rather than trusted:

- **`width + height` rises strictly with the node index.** `arrayRects` sorts on that key with
  a `qsort` that is not stable, so a tie hands the cell order to glibc. With this table there
  is no tie to hand over, the descending sort is a **total order**, and the port's `sort_by`
  is exact instead of a guess. The side effect is that the sort is a genuine permutation — it
  is the exact reverse of the declaration order — so the drawing differs from the uniform grid
  at *every* node count, not only past the eleventh.
- **every size is a whole number of `1/2048` inch.** Graphviz reads `width` in inches and
  converts back to points (`INCH2PS`), so a size that does not survive that round trip would
  make the two arms size the same box differently. On this grid `points / 72 · 72` is the same
  `f64`, verified over the whole 601-node range in `osage/tests.rs`.

The DOT the harness writes is `fixedsize=true, label="", margin=0, width=…, height=…` per node.
`fixedsize=true` is the load-bearing attribute: it makes `shapes.c:2116-2125` take
`bb = (width, height)` verbatim instead of `fmax`-ing it against the label. `label=""` leaves
the label nothing to demand and `margin=0` leaves the default 0.11 inch of padding nothing to
add. Measured: with all three, `-Tplain` reports back exactly the box that was asked for, for
every node count 1 … 601.

### The per-node size input the layout needed, and where it is

The registered stage had **no** per-node size input: `run` took only a `Topology`, because
every rectangle was Graphviz's default `nodesize` and the whole layout was a function of the
node count. That collapsed only because every box tied. The narrow input the layout actually
needs is one box per node in dense index order, and it is
`layout::graphviz::osage::run_sized(&Topology, &Boxes)` —
`crates/graph-core/src/layout/graphviz/osage.rs`, packed by
`crates/graph-core/src/layout/graphviz/osage/array.rs`, which is `arrayRects`
(`lib/pack/pack.c:604-714`) over differing boxes with none of its five steps collapsed.

The two callers are deliberately different and both are honest:

| caller | entry point | what it packs |
|---|---|---|
| the ledger row (`registry/graphviz_osage.rs`, the six closed cases) | `run` | the default `nodesize`, a pure function of the node count, unchanged |
| the differential (`oracle_python/osage.rs`) | `run_sized` over `Boxes::table(n)` | the fixture's explicit boxes |

`run` was **not** changed. It still emits the uniform 58 × 40 grid and still matches Graphviz
byte for byte on the six closed cases, and `a_uniform_table_gives_the_registereds_own_answer`
pins that the two paths agree node for node over every count 1 … 600 — so the sized path
cannot drift from the path the ledger runs while the closed cases keep passing on a stale
answer.

One honest cost: **the sized path is not a gather, so D10 does not describe it.** Under `run`,
node `i`'s cell depends only on `i` and the node count. Under `run_sized` it depends on which
column and row the *sort* put `i` in, and a column's width is a maximum over every box in it —
so one coordinate is a function of all the sizes. It is still deterministic (the table makes
the sort a total order, and every pass is in one fixed order, with no reduction over a
`HashMap`), and `grid.rs` still is a gather, but the property has to be stated per path rather
than for the module.

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

## The two named causes, and what closed each one

### 1. Graphviz sizes a node's box from its rendered label — closed by pinning it

`osageinit.c:124` packs `{.UR = {.x = ND_xsize(n), .y = ND_ysize(n)}}`, and
`gv_nodesize` returns the `nodesize` minimum only while the label fits inside it. Measured
on the oracle, one node per graph, label length against `-Tplain`'s reported width:

| label | box width (points) |
|---|---|
| `n`, `nn` | 54.000 (the 0.75 inch minimum) |
| `nnn` | 57.942 |
| `nnnn` | 70.358 |
| `nnnnn` | 82.774 |

so the *default* fixtures — labelled `n0` … `n{n-1}` — have 54 point boxes below `n10` and
**57.942** point boxes from `n10` on. `arrayRects` then takes each column's cell width from
the widest box in it (`pack.c:665`), so from n = 11 the cell grid is no longer uniform.
The 0.75 inch default is the *minimum*, not the answer, and the amount above it is a font
metric of Graphviz's own text layout.

The port took every box to be the default 54 × 36, which is exact exactly while the labels
fit — that is, below eleven nodes.

**Closed.** The fixture now gives every node an explicit box and the DOT pins it, so the
amount above the minimum is zero and the font metric never enters:
`fixedsize=true, label="", margin=0, width=…, height=…`. Measured: `-Tplain` reports back
exactly the box asked for, at every node count from 1 to 601. The width Graphviz *would*
have computed from the label is still a font metric, and still is not something graph-core
should compute — the fix is to stop asking, which is what a pinned size is.

### 2. `arrayRects` sorts the boxes with a `qsort` that is not stable — closed by removing the tie

`pack.c:658` orders the rectangles by `width + height` before filling the grid, and
comparing boxes of equal size returns 0, so which node lands in which cell among tied boxes
is glibc's choice. Measured on n = 14 (`nc = 4`, `nr = 4`) with the *default* labels, the
reference's own rows:

| row (top to bottom) | nodes |
|---|---|
| 0 | `n10 n11 n12 n13` — the four 57.942-wide boxes, sorted first |
| 1 | `n0 n1 n2 n3` |
| 2 | `n4 n5 n6 n7` |
| 3 | `n8 n9` |

Below eleven nodes every box ties *and* they are all the same size, so the sort cannot move
the geometry — only the names — and the names are in declaration order at every size
measured. From n = 11 the wide boxes sort ahead of the narrow ones and whole rows move.

**Closed.** The fixture table makes `width + height` strictly increasing in the node index,
so there is no tie for glibc to resolve: the descending sort is a total order and the port's
`sort_by` reproduces it exactly. This was the right fix rather than reimplementing glibc's
introsort — the tie was an artefact of *letting Graphviz choose the box size*, and pinning the
size removes the question instead of answering it. `tests.rs` pins the no-tie invariant over
the whole 601-node range, so the property cannot rot into a regression silently.

### What is still a limitation

Both fixes are in the fixture, so they say something narrower than "this engine now matches
Graphviz for every graph". What is held:

- **the sweep** — the 1000 gate fixtures, with the boxes pinned, agree to 6.31e-2 points, and
  the negctl shows what a real disagreement costs on the same metric (58.0 points).
- **the six closed cases** — byte-exact, against the *default* `nodesize`, which is the
  registered path.
- **the uniform table** — `run_sized` over `54 × 36` boxes equals `run` node for node over
  every count 1 … 600.

What is **not** held, and cannot be from here:

- A label-sized graph (the *default* fixtures, ids of three or more characters at n ≥ 11).
  There the reference's boxes are 57.942 points wide and graph-core would need Graphviz's text
  metrics to know it. The registered `run` is documented as taking the default `nodesize`, and
  the differential deliberately runs the sized path instead of pretending otherwise.
- A DOT graph with `subgraph cluster_*`, or with `pack` / `packmode` / `nodesize` attributes.
  The reference packs subclusters recursively and reads all three; this port has neither.
- glibc's introsort itself. The tie is gone on this fixture, so the question does not arise;
  a graph that *did* tie would still be glibc's to resolve, and `run_sized` breaks such a tie
  by ascending node index (D2), which was declaration order at every size measured.

## Why the row is not `gated`

`Status::Gated` needs both a 4-way hash verdict and the row's own oracle verdict backed by a
recorded run (`crates/graph-cli/src/capabilities.rs:201-209`), and
`verdict::Evidence::oracle_record` (`crates/graph-cli/src/capabilities/verdict.rs:63-74`)
resolves only a **fixed list** of record names — `oracle-diff`, `roundtrip`,
`oracle-layouts`, `stress`, `oracle-fa2`, `oracle-spectral` and the transport row. It has no
arm for `oracle-osage`, nor for `oracle-twopi`, `oracle-circo` or `oracle-patchwork`.

Measured, not assumed: setting `Status::Gated` on this row and running the check gives

```
layout.packing.osage: gated, but no hashgate record: run the gate
layout.packing.osage: gated, but no oracle-osage record: run the gate
```

The first line is the orchestrator's gate to produce. The second is the reader gap, and it is
in `capabilities/verdict.rs` — outside this job's paths, and shared with the parallel Graphviz
engine jobs, so five agents editing one `match` is a merge collision on a shared file rather
than a feature. So the row stays `implemented`, which states the truth: registered, hashed,
differentially measured against a real recorded run, and not yet an oracle-backed gate.
The fix is one `Evidence` arm per Graphviz differential, and it belongs with whoever owns that
reader — not smuggled in to make one row look stronger than the four beside it.

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
| **the pinned node sizes are load-bearing** | `negctl-osage-sizes` — one node's width in the fixture's `box` column moved by `9/18432` inch, differential re-run | nonzero | **1** (worst gap **58.0 points** against ceiling 1e-1) |
| a knob the gate already had still turns it red | `GM_MUTATE_TWOPI_NODES=1 … hashgate --seeds 8` | nonzero | 1 |
| a shared knob still turns it red | `GM_MUTATE_REFERENCE_DEGREE=9 … hashgate --seeds 8` | nonzero | 1 |

The second row is what keeps the first honest. An earlier draft of the control compared a
20-seed output against the 1000-seed one; it exited non-zero with the perturbation deleted,
because the seed counts differ — a control that cannot fail. Both sides now come from the
same fixture set, and the unperturbed pair is asserted identical, so the non-zero exit can
only come from the 1e-6 point move.

**The fourth row is the control this revision needed**, and it is what makes the whole
differential mean something. The change pins every node's box in the fixture and reads the
same boxes in the layout, so the failure mode is that the two stop agreeing — the harness
drawing Graphviz's graph at the default `nodesize`, or at a table of its own. Nothing else in
the gate would notice: the check would still pass the hash, still compare real oracle output,
and report a large gap that reads like a layout disagreement. So the control moves **one**
node's pinned width by `9/18432` inch — `1/2048` point, far below anything the layout
computes, and enough to change `width + height` and therefore the sort order — and the
measured result is a **58.0 point** gap: one full cell stride, and what a real disagreement
costs on this metric. That is 920× the honest worst gap of 6.31e-2.

The perturbation is in the **fixture**, not in the harness, on purpose: the drift being
tested for is the two arms reading different tables, and a harness-side knob would have the
harness testing itself.

**There is no `negctl-osage-nodes` row, on purpose.** The per-stage knob for a Graphviz
engine re-draws that stage's own model (`hashgate/knob/twopi.rs` is the precedent), and
`hashgate/knob.rs` is shared with the five parallel Graphviz engine jobs — five agents
editing one enum is a merge collision on a gate file. The three osage controls above sit
where they can be added without touching it, and they are the ones that matter here.

### A limitation of the check itself, found while writing control 1, and then used

`verdict()` compares the fixture manifest's `sha256` against the *result's* recorded
`sha256`, not against the fixture file on disk, so editing `osage.jsonl` after the harness
has run is not caught, and a perturbation to the fixture file is invisible to the check as a
*staleness* failure. That is a pre-existing property of the shared three-step driver
(`oracle_python.rs:143`), not something this job changed, and it is recorded here rather than
fixed because `oracle_python.rs` is outside this job's paths.

It is also why the two fixture-side controls are shaped the way they are. `negctl-osage-sizes`
*deliberately* edits `osage.jsonl` and then re-runs the harness, so the harness reads the
edited fixtures, records a real worst gap, and the check fails on the **ceiling** rather than
on a staleness it cannot see. Control 1 perturbs the *recorded oracle output* and re-compares
with `cmp`, because that is a different question: whether the comparison looks at the oracle's
own bytes at all.

## Ponytail (this differential)

- **The ceiling is the oracle's printed resolution, and it is measured to be so.** 1e-1 points
  is the next power of ten above 6.31e-2. The worst gap is 0.88 of one printed digit, the
  gaps track each drawing's extent, and nothing reaches one cell stride — while
  `negctl-osage-sizes` shows a real disagreement on the same metric at 58.0 points. A tighter
  ceiling would be a claim about `-Tplain`'s formatter, not about the layout.
- **`-Tplain` prints five significant digits** (`lib/common/output.c:129-141`), so even where
  the arms agree exactly, 6.31e-2 points at these drawing sizes is the oracle's own quantum
  rather than our arithmetic. The closed cases are compared on the printed strings for exactly
  this reason.
- **The fix is in the fixture, not a tolerance.** An earlier revision of this file argued the
  two causes were "not worth closing here". They were worth closing: both were artefacts of
  letting Graphviz pick the box size, and pinning it removes the font metric and the tie
  together. The 58-point control is what a disagreement looks like, and it is not what the
  sweep produces any more.
- **The registered path still takes the default `nodesize`, and that is a stated limit rather
  than a quiet one.** `run` is a pure function of the node count; `run_sized` is the general
  `arrayRects` and is what the differential runs. Both are correct for what they pack, and
  `a_uniform_table_gives_the_registereds_own_answer` keeps them from drifting apart.
- **The harness imports the metric from `harness/oracle-twopi.py` by path**, because that
  filename is not importable by name and a copy of the metric would be a second definition
  of the number the ceiling is measured against. `sys.dont_write_bytecode` keeps the
  fingerprinted `harness/` tree free of the `__pycache__` a path import writes.
- **`harness/oracle-graphviz.py` gained `--start=`, `--fixtures=` and `--differential`**, all
  optional; the three-argument call the twopi and circo determinism evidence used is
  unchanged, and `harness/oracle-twopi.py` is untouched and still works.
- **The size-pinned DOT writer lives inside `harness/oracle-graphviz.py`, not `gv_plain.py`.**
  `gv_plain.write_dot` is shared by twopi, circo and patchwork and is left exactly as it was;
  the pinned writer is chosen by whether the *fixture* carries a `box` column, so an engine
  gains or loses the pinning by changing what its emit writes rather than by a name checked
  in the harness. The other three engines' fixture files are byte-identical before and after
  this change (sha256 `44b1a461…`, `66393b41…`, `f1be79c6…`).
- **The row stays `implemented`, and the reason is now on the hash side.** The ledger
  resolves `oracle-osage` by name like any other record, so `oracle_diff` reads this
  measurement back; what a `gated` status would still need is a negative control that went
  red on the `layout.packing.osage` stage, and none exists — the honest run hashes the stage
  4-way on every seed while every control that does go red leaves it equal. Promoting the
  row therefore needs a per-stage knob in `hashgate/knobs`, not a verdict edit.
