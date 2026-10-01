# The igraph family's differential ceilings

Measurement for `docs/decisions/layouts-igraph.md`'s ceilings, on the merged p12-igraph tree
(2026-10-01). Command:

```
graph-cli emit-igraph-fixtures --seeds 100
docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-igraph.py target/igraph-fixtures
graph-cli oracle-igraph
```

## The metric, and why not the one `layout.force.*` uses

`layout.force.barnes_hut` is held to **d3-force@3.0.0** by the *stress* metric: the Pearson
correlation between graph (BFS hop) distance and Euclidean distance over 32 max-min pivots,
with a margin of **-0.05** (`crates/graph-cli/src/stress/metric.rs`, gated by
`graph-cli stress --oracle d3`). `layout.forceatlas2` is held to networkx by a **coordinate**
gap, `1e-7` at 2 iterations (`oracle_python/fa2.rs`).

Neither shape transfers to these six, and the reason is in what each is a statement about:

- The d3 stress metric is a **margin against a reference layout that shares our starting
  positions and our tick count**. It asks "is our picture at least as faithful to graph
  distance as d3's, on this graph". igraph's six layouts take no such shared start: five are
  iterative with their own cooling and their own defaults, and `layout_lgl` takes no start at
  all. There is no d3-shaped shared starting condition to correlate against.
- A coordinate gap (`1e-7`, FA2) is a claim that two implementations of the *same* function
  agree to a tolerance. The igraph family cannot make it: igraph draws its own randomness for
  five of the six and works in single precision for DrL, so the arms are not the same
  computation twice. This is the same chaos that stops the FA2 coordinate differential
  gating (`prompts/RESUME.md` item 4), and the same reason the six carry a ratio rather than
  a gap.

What the family does use **is** the stress metric, on its own terms: normalised stress against
graph distance after the optimal uniform scale, per arm, and the verdict is
`max over seeds of ours / igraph`. Both arms are scored by the same formula the Barnes-Hut
metric is named after; the difference is that the comparison is a ratio against the reference
layout rather than a correlation margin, because igraph's own drawing is the only fixed point
available. `normalised_stress` in `harness/oracle-igraph.py` is the definition, and the floor
under igraph's stress is `1e-3`.

## The ceilings

Measured 2026-09-29 over 100 seeds and re-measured on this tree; each is the measured worst
rounded **up to the next power of ten**, which is what the other Python differentials do
(`oracle_python/fa2.rs`, `spectral.rs`, `closed_form.rs`).

| Layout | Metric | Measured worst | Ceiling | Status |
|---|---|---|---|---|
| `layout.force.fruchterman_reingold` | `ours/igraph` normalised stress | 1.30 | 10 | `implemented` |
| `layout.force.kamada_kawai` | same | 1.35 | 10 | `implemented` |
| `layout.force.drl` | same | 3.98 | 10 | `implemented` |
| `layout.force.lgl` | same | 2.13 | 10 | `implemented` |
| `layout.force.davidson_harel` | same | **51.91** | **100** | `implemented` |
| `layout.force.graphopt` | same | **15.39** | **100** | `implemented` |

### Davidson-Harel 51.91 and Graphopt 15.39 against a `2e0` ceiling

The brief carried a 2e0 ceiling. The tree has never held one: `oracle_python/igraph.rs`
declares `CEILING_TIGHT = 10.0` and `CEILING_LOOSE = 100.0`, and the same 51.9 / 15.4
measurement is recorded in `docs/decisions/layouts-igraph.md`. So this is not a ceiling
widened to pass a row; it is the branch's measured ceiling, re-measured here.

Two separate reasons, and they are not the same reason:

- **Graphopt 15.39.** igraph's stress *reference* is near-optimal on these small gate graphs
  (2 to 601 nodes, a fixed reference degree), so the denominator is small and the ratio is
  large. This is a property of the reference arm on this fixture family, not of our port: the
  ratio would shrink on a graph family where igraph's own drawing is not already
  stress-optimal, and the gate model is the only family measured here.
- **Davidson-Harel 51.91.** Annealing draws from different generators in the two arms, and
  DH's quality is a *local* optimum of an aesthetic energy. The worst seed is one where our
  draw settled in a worse local optimum than igraph's, which is the expected shape of two
  independent searches of the same energy — not a port defect, and not a 2x drift the ceiling
  is meant to catch.

Neither number is defensible as a 2e0 ceiling, and neither is defensible as a *tight* one:
the worst is a maximum over 100 seeds of a ratio whose denominator is floored at 1e-3, so
the tail is not characterised. 100 catches breakage (a layout that stops converging, a port
that reads the wrong parameter) and does not catch drift, which is the honest claim.

**Recorded limits.** The floor means a real gap smaller than 1e-3 under igraph's own stress is
invisible. Only pairs inside one component are scored, so a disconnected graph is judged on
its components alone. Stress is not what FR, DrL, LGL or Graphopt optimise — they balance
forces — so their ratio is a quality floor, not a disagreement measure: a correct force layout
can read worse than igraph's here, and an ugly stress-optimal one reads better. A ratio near
the ceiling needs a look at the picture before it is called a defect.

## The rows are `implemented`, and may not be `gated`

All six rows are `Status::Implemented`
(`crates/graph-cli/src/capabilities/registry/unproven.rs`, `force_record`'s igraph arm). That
is what the evidence supports today, and `problems()` would refuse `gated` for three separate
reasons even if a row claimed it:

1. `capabilities::verdict::Evidence::oracle_record` has **no `oracle-igraph` arm** — its match
   carries `oracle-diff`, `roundtrip`, `oracle-layouts`, `stress`, `oracle-fa2`,
   `oracle-spectral` and `wasm-transport`. An igraph row naming `oracle-igraph` finds no
   record and no field to read, so `verdict::oracle_diff` returns
   `Err("no oracle-igraph record: run the gate")`.
2. **No negative control reaches these six stages.** `Knob::ALL` (27 arms) has none that
   perturbs drl, lgl, davidson_harel, graphopt, kamada_kawai or fruchterman_reingold, so
   `verdict::hash_4way`'s `red_control` requirement ("a negative control that ran on this
   tree, went red, and diverged on that stage") cannot be met for any of them.
3. The igraph differential defaults to **100 seeds** (`oracle_python/cli.rs`) against
   `verdict::MIN_SEEDS = 1000`.

Promoting any of the six to `gated` needs all three: an `Evidence::igraph` field and its
`oracle_record` arm, one `GM_MUTATE_*` control per stage, and the differential run to 1000
seeds. That is a separate piece of work, not a status flip, and it is why this document
records the ceilings without claiming the rows are gated.

## The case count after the fix

`emit-igraph-fixtures` writes an `ours.<key>` column only for an id `registry::find`
resolves, so an unregistered layout scores zero cases and `judge` fails it. The measurement
above was re-run on the merged tree, where all six resolve, and the Python arm reports the
case count directly:

```
{"fruchterman_reingold": {"cases": 100, "worst": 1.298, "reference_worst": 1.029},
 "kamada_kawai":         {"cases": 100, "worst": 1.348, "reference_worst": 1.029},
 "drl":                  {"cases": 100, "worst": 3.977, "reference_worst": 4.960},
 "lgl":                  {"cases": 100, "worst": 2.131, "reference_worst": 1.237},
 "davidson_harel":       {"cases": 100, "worst": 51.910, "reference_worst": 4.830},
 "graphopt":             {"cases": 100, "worst": 15.389, "reference_worst": 1.386}}
```

**100 cases for all six**, DRL and LGL included. `graph-cli oracle-igraph` on this tree
prints the same six rows against their ceilings and exits 0.

**There was no fixture/case-name mismatch to fix.** The keys and the ids are byte-identical
between the writer and the reader on both sides of the merge — `harness/oracle-igraph.py`'s
`REFERENCES` keys and `crates/graph-cli/src/oracle_python/igraph.rs`'s `IGRAPH.ceilings`
keys are the same six strings, and both take the dotted stage id from `<Layout>::ID`. The
zero cases came from the one code path that can produce them, `igraph.rs:64`'s
`registry::find(id).is_some()`: with the registry in conflict, the igraph entries were not
resolvable, no `ours` column was written, and the Python arm's pre-seeded row kept
`cases == 0`. Resolving `crates/graph-core/src/registry.rs` — all fifteen develop entries
and all six igraph ones, `LAYOUTS: [Capability; 21]` — is the fix, and the count above is
the proof. Had a key genuinely differed, the Python side could not have shown 100 for the
other four: `layouts[key]` at line 89 is a dict lookup on a dict pre-seeded from the same
`REFERENCES` table, so an unknown key raises `KeyError` and kills the whole run rather than
scoring two layouts at zero.

## The hash-gate entry and its negative control

Each of the six is a stage of the gate without an edit to `hashgate/stages.rs`, because
`stages()` is registry-driven: registering the layout registers its stage. The 4-way gate
hashes all six on every seed (measured, `--seeds 1` on this tree: `4-way equal on 1/1` for
each), which is what makes the native and wasm32 arms agree bit for bit on them.

Each also has a control of its own, `GM_MUTATE_FORCE_{LAYOUT}_NODES`, in
`hashgate::knobs::IGRAPH_LAYOUT_STAGES`. The knob list is one list, in
`Knob::ALL` and in `crates/graph-cli/tests/common/mod.rs::KNOBS` (33 entries), and each of
the six turns the gate red **on its own stage and no other** — measured, `hashgate
--seeds 1` with the variable set exits 1 for all six, and `tests/cli_igraph.rs` asserts
every other stage by id (37 of them: 32 non-layout plus the five sibling igraph layouts)
stays `4-way equal`.

## Reproducing

`emit-igraph-fixtures` writes one line per seed with the gate model and, for every id
`registry::find` resolves, an `ours.<key>` column. A layout with no column has zero cases and
`judge` fails it: NOT-RUN, never green. The image is built from the digest-checked sdists:

```
docker build --build-context nx="$GM_SCRATCH/refs/networkx-3.6" \
  --build-context ig="$GM_SCRATCH/refs/igraph-0.11.9" \
  -f docker/python-oracle.Dockerfile -t ge-python-oracle .
```
