# P4-carry — a live force session that survives a graph change

`ForceSession::carry(&self, from: &Topology, to: &Topology) -> Result<ForceSession, SessionError>`
in `crates/graph-core/src/layout/force/session/carry.rs`. This is the motor half of the plan's
P4 (`prompts/perf-plan.md:104`): the graph grows while an AI process emits nodes and edges, and
the layout continues instead of restarting. The delta format, the wasm ABI, the SDK and the studio
host are later slices and are not here.

## Why a carry and not a restart

A layout under a live process is not a function of the current graph — it is the whole trajectory
of the process so far. Re-seeding on every emission puts the picture back on the golden spiral
(`barnes_hut/seed.rs:11`) each time, which is exactly the flicker this exists to avoid. So a carry
moves the state across and keeps it.

## The API

```rust
impl ForceSession {
    pub fn carry(&self, from: &Topology, to: &Topology) -> Result<ForceSession, SessionError>;
}
```

`self` is untouched and both sessions may be stepped. `from` is the topology `self` was built over;
`to` is the new one. The result carries:

- every node of `to` whose id is a row of `from`: its exact `x`, `y`, `vx`, `vy` and **both pins**,
  at `to`'s row for that id;
- `alpha`, `alpha_target`, `tick_no` and the parameters, unchanged;
- every node of `to` that `from` did not have, placed by the rule below at rest.

Refused with `SessionError::ColumnLength { column: "from", .. }` when `from.node_count()` is not the
session's row count. Nothing is built when it is refused. Carrying onto a **smaller** topology is
not refused: dropping nodes is a shrink, and the survivors keep their bytes.

### Only ids cross

A dense row means nothing in a topology the session was not built over, so `to`'s rows are mapped
back to `from`'s by id through `Topology::node_index` and nothing else. `only_ids_cross_a_carry`
is the negative control: the same carry onto the same ids with the rows in the opposite order moves
every survivor to the row its **id** is at, at its own bytes.

### Loop order

Every walk is ascending and every reduction is over a fixed sequence: rows ascend, and a row's
neighbours are its `simple_graph` row, which `mod.rs:77` files in edge-index order. No hash order
reaches the output.

## The seeding rule, and why

| A node of `to` | Where it starts |
|---|---|
| its id is a row of `from` | exactly where it was: `x`, `y`, `vx`, `vy`, `fx`, `fy` |
| new, with a carried neighbour | the mean of its carried neighbours plus a phyllotaxis offset |
| new, with none | the golden-spiral point of its row in `to` |

The third row needs no code: `Sim::new` has already seeded every row of `to` on that spiral, and
that function of the row index is the same one. Writing it again would be a second spelling of it.

A new node starts **at rest** — `Sim::new` leaves every velocity at zero and a carry writes only
the columns of carried rows. Inventing a velocity for a node that has never moved would make the
result depend on the graph change rather than on the run.

The mean is read from the **old** columns by `from`'s row, never from the new ones: a carried
neighbour further along `to` has not been copied yet when an earlier new row asks for it, so its
position is only trustworthy where the run already had it.

### The offset

`OFFSET_RADIUS = 1.0` layout unit, at `seed.rs`'s own `GOLDEN_ANGLE`, indexed by the row's index
among the new rows in ascending order. One line of reason: the seed spiral's first point is 12 units
out and `link_distance` is 60, so one unit lands a new node *inside* the cluster it belongs to
rather than displacing it, and two leaves of one hub — consecutive new indices, 137.5° apart — land
`2·sin(φ/2) = 1.86` units apart, so they never coincide. That non-coincidence is the only thing the
offset has to guarantee.

## Complexity

One pass over `to`'s rows plus one over the edges of its new rows: `O(n + m)`, no `O(n²)`, no
per-node allocation, one buffer sized once at the row count.

Getting there took one measured change. The first version asked "is this row of `to` a row of
`from`?" through `from.node_index(...)` at every question — and `carried_mean` asks once per
incident edge, so a batch cost an `O(m)` hash of an id string on top of the `O(n)`. Building the
whole map once (`Placement::map`, ascending, before anything is placed) turned those `2m + n` hashes
into `n`. At n = 1M, batch 10 000, that took the carry from ~965 ms to ~400 ms on the same host.
Reading the id out of the node column rather than through `Topology::node` — which builds a
ten-field `NodeView` to hand back one field — took it from ~650 ms to ~320 ms.

## Gates

Run in this worktree; exit codes are real.

| Gate | Exit |
|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 (PASS, 4-way equal on 8/8) |

`cargo test --workspace`: 1106 passed / 0 failed / 6 ignored in graph-core's lib, 134 / 0 / 1 in
graph-wasm, 117 / 0 in graph-contract, and every integration binary green.

## Timing

`graph-cli tick --grow <BATCH>` (`crates/graph-cli/src/bench/tick.rs:105`): builds the gate model at
`--n`, indexes the topology **without** the last `--batch` nodes, warms a frozen session with 3
ticks, then times, separately and as the median of 3 repetitions, (a) building the new topology with
`index_model` and (b) the `carry`. The old topology, the session and the warm ticks are outside the
timer. `loadavg` is printed on the row.

Release, batch 10 000, `/proc/loadavg` as printed:

| n | batch | m new | index ms | carry median ms | carry min ms | carry max ms | carried | new | load start | load end |
|---|---|---|---|---|---|---|---|---|---|---|
| 100000 | 10000 | 154978 | 75.33 | 18.24 | 17.34 | 18.37 | 90000 | 100000 | 14.21 14.79 13.95 | 14.21 14.79 13.95 |
| 100000 | 10000 | 154978 | 72.29 | 18.92 | 17.10 | 19.18 | 90000 | 100000 | 14.21 14.79 13.95 | 14.21 14.79 13.95 |
| 100000 | 10000 | 154978 | 66.22 | 17.84 | 17.49 | 18.19 | 90000 | 100000 | 14.21 14.79 13.95 | 14.21 14.79 13.95 |
| 1000000 | 10000 | 1549929 | 2685.47 | 669.88 | 569.11 | 798.43 | 990000 | 1000000 | 13.79 14.69 13.92 | 14.99 14.90 14.01 |
| 1000000 | 10000 | 1549929 | 2610.34 | 645.79 | 397.72 | 721.26 | 990000 | 1000000 | 20.56 16.19 14.45 | 22.71 17.11 14.81 |
| 1000000 | 10000 | 1549929 | 2303.38 | 665.16 | 476.48 | 683.14 | 990000 | 1000000 | 22.63 17.46 14.97 | 20.33 17.26 14.96 |
| 100000 | 10000 | 154978 | 64.94 | 17.07 | 17.05 | 17.86 | 90000 | 100000 | 17.61 17.59 20.23 | 17.61 17.59 20.23 |
| 1000000 | 10000 | 1549929 | 1310.14 | 320.07 | 315.18 | 354.54 | 990000 | 1000000 | 17.61 17.59 20.23 | 16.59 17.37 20.11 |
| 100000 | 10000 | 154978 | 78.57 | 18.10 | 17.91 | 35.35 | 90000 | 100000 | 19.93 19.79 19.37 | 19.93 19.79 19.37 |
| 1000000 | 10000 | 1549929 | 1764.75 | 341.28 | 328.02 | 568.81 | 990000 | 1000000 | 19.93 19.79 19.37 | 18.32 19.44 19.27 |

Rows per size, so the spread is visible rather than summarised away. The last two rows are later runs
of the final binary; the 1M rows above them predate one last micro-optimisation (reading the id from
the node column rather than through `Topology::node`), which is why those figures fall from ~650 ms to
~320 ms. **~320–341 ms is the number to read**; the ~650 ms rows are recorded because they were real
runs of real code, not because they describe what is in the tree.

**Caveat: the host was loaded throughout.** Load average sat at 14–23 on 20 cores for every run,
reaching 19.93 by the last one, and never fell below ~13.9. Wall clock on a loaded host is inflated,
and the 1M rows show it: `carry min` moved between 315 ms and 569 ms across repetitions of the *same*
work, and the `carry max` on the 100k row swings to 35 ms on one repetition out of three. That is the
machine, not the algorithm. Treat these as an upper bound on a busy host, not as this code's cost on
an idle one. Nothing here speaks for wasm32.

## The plan's budget: missed

`prompts/perf-plan.md:118` — "1M final size at 10k nodes per second ingested, with a **rebuild of
≤ 30 ms per batch at 1M** (measured)".

**Missed, by more than an order of magnitude.** The carry at 1M with a 10 000-node batch measures
**~320–341 ms** median on the final binary (~650 ms before the last id-read optimisation), against a
30 ms budget. The rebuild of the topology itself — the part the budget's "rebuild" names — is
~1.3–2.7 s, which is ~45–90× the budget on its own, before any carry is timed.

Two things are true at once, and the second is the more useful one:

1. **The budget is not met**, and the `index_model` half misses it worst. Re-indexing a 1.5M-edge
   model from records is seconds of work, not tens of milliseconds, so a design that re-indexes the
   whole topology per batch cannot reach 30 ms at 1M no matter how fast the carry is. The carry's
   own `O(n + m)` is the cheap half by comparison.
2. **The plan already anticipated this.** `perf-plan.md:118` says: *"If it misses, an append-only
   CSR with periodic compaction becomes a sub-slice"*, and `perf-plan.md:38` says the same about an
   append-only topology. That is the recommended next step, and this measurement is the evidence
   for it: the fix is in the topology's representation, not in the carry.

Note also that this is the *floor* the carry could reach here, not a claim about a future one.
`ForceSession::from_frozen` over the new topology — a bare session build with no mapping at all —
measured ~180–195 ms at 1M on this host, because `simple_graph` deduplicates 1.5M edges and
`link::geometry` recomputes per-link geometry for all of them. So even with an append-only topology
and no rebuild at all, a carry that constructs a fresh `Sim` cannot get near 30 ms at 1M; reusing
the existing graph where the edges are unchanged would be the next lever after the CSR.

## What this does not do

- **No delta format, no `apply`.** `perf-plan.md:107` specifies
  `{addNodes, addEdges, removeNodes, removeEdges, attrs}` and `ForceSession::apply(delta)`. This is
  the id-mapping half, reachable only with two already-built `Topology` values in hand.
- **No wasm ABI.** No `gm_force_session_apply`; nothing here crosses a boundary yet.
- **No SDK, no studio host, no replay fixture, no `stream` hashgate arm.** The delta trace and its
  negative control (drop one delta, the gate goes red) are later slices.
- **The new-topology build is untimed in the plan's sense and unmeasured for `from`.** The table
  times `index_model` for `to`; building `from` is setup and outside the timer.
- **Not measured on wasm32, and not measured on an idle host.**
- **No append-only CSR, no compaction.** Per `perf-plan.md:118` that is the sub-slice this
  measurement argues for.
- **`tick_no` and `alpha` are carried, not re-derived.** A carry is not a restart by construction —
  `a_grown_topology_keeps_every_old_node_byte_for_byte` asserts it — but a caller that wants a
  reheated run still has to call [`ForceSession::reheat`]; this does not do it for them.
- **Pins do not protect a new node.** A new node is placed free; it does not inherit a pin from a
  neighbour.