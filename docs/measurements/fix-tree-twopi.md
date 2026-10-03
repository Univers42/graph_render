# fix-tree-twopi — L-01 and L-08 (`docs/reviews/review-layout-tree.md`)

Two MAJOR findings in `layout/radial/twopi`, both cost findings: the layout produced the
right drawing and spent the wrong amount of memory and time getting there. Neither changed a
coordinate — see "Output did not move" for the evidence that it did not.

| id | severity | verdict | test | file:line |
|---|---|---|---|---|
| L-01 | MAJOR | fixed | `an_edgeless_graph_allocates_its_columns_once_not_once_per_component` (RED on the old code) + `one_set_of_columns_per_component_would_blow_both_budgets` (control) | `twopi/scratch.rs:1` (new), `twopi/tree.rs:24`, `twopi/angles.rs:25`, `twopi/adjacency.rs:130` |
| L-08 | MAJOR | fixed | the 12 closed-form cases in `twopi/tests.rs` and the 1000-fixture oracle, which pin the neighbour order node by node | `twopi/adjacency.rs:42`, `twopi/adjacency.rs:82` |

## L-01 — `O(components x n)` scratch columns per component

### The claim on this tree

`twopi/tree.rs`'s `search` allocated `depth`, `parent` and `children` with
`vec![..; neighbours.rows()]` and `count_leaves` allocated `leaves` the same way, all three
inside `run`'s per-component loop; `angles::spans` and `angles::thetas` each allocated one
more `n`-length column per component. An edgeless graph has one component per node, so the
run spent `O(components x n)` — eight heap allocations *per node* — against the
`registry/radial.rs:48` declaration:

> `complexity: "O(n + m): two breadth-first searches (to the nearest leaf, then from the root),
> one parent walk per leaf, and one sweep per tree node over its own neighbours"`

### RED

`crates/graph-core/src/layout/radial/twopi/tests/alloc.rs` — a counting `#[global_allocator]`
around `run`, per thread, armed only for the duration of the measurement (the pattern
`crates/graph-core/tests/tick_alloc.rs` uses, moved into the lib's test binary so it can
count the layout's own buffers). Not a timing assert: an allocation count.

```
$ scripts/orch/gr cargo test -q -p graph-core --lib layout::radial::twopi
allocations on an edgeless graph: n=1024 -> 8220, n=4096 -> 32800
thread '...an_edgeless_graph_allocates_its_columns_once_not_once_per_component' panicked at
  crates/graph-core/src/layout/radial/twopi/tests/alloc.rs:153:5:
Heap { calls: 32800, peak: 344076 } at n=4096 exceeds the 256-allocation budget; a per-component
  column is back
test result: FAILED. 12 passed; 1 failed; 1 ignored; 0 measured; 1232 filtered out
```

8220 at n=1024 and 32800 at n=4096: eight per node, exactly the six columns plus the
component vector and its per-component `Vec`. The test also asserts that four times the nodes
must not cost more than 32 extra allocations, which is what separates `O(components x n)`
from geometric growth — a count that merely stayed under a budget would not carry the claim.

### GREEN

* `twopi/scratch.rs` (new, 71 lines) holds the seven columns, allocated once for the whole
  layout. `reset` clears **only the component's own entries**, and its doc says why that is
  sound rather than merely convenient: every write a pass makes is to a node it reached from
  this component's root, and every read is to a neighbour of a node it is walking, and a
  neighbour of a component node is in the same component because `for_each` yields out- and
  in-edges alike. Clearing `n` slots per component would have been the quadratic again.
* `twopi/adjacency.rs`: `components` returns a flat `Components` (one node list plus one
  `u32` per component) instead of `Vec<Vec<u32>>`, because a vector per component is one
  allocation per component — the same defect, half the size.
* `twopi/tree.rs` and `twopi/angles.rs` write into the `Scratch` instead of into vectors of
  their own.

```
$ scripts/orch/gr cargo test -q -p graph-core --lib layout::radial -- --nocapture
allocations on an edgeless graph: n=1024 -> 48, n=4096 -> 54
allocations for one column set per component: n=1024 -> 7168, n=4096 -> 28672
test result: ok. 14 passed; 0 failed; 1 ignored; 0 measured; 1232 filtered out
```

The second line is the **negative control**, compiled in the way `layout/force/barnes_hut`
does its split-sum control rather than through an environment knob: it builds the layout's
own seven columns once per component, through the same probe, and must be over both budgets
(28672 > 256, and +21504 > the 32 slack). If either budget were loosened to the point where
the two shapes were indistinguishable, that test turns red.

### Before / after — `twopi/tests/measure.rs`, an **edgeless** graph

`cargo test --release -p graph-core --lib layout::radial::twopi::tests::measure -- --ignored
--nocapture`, printing time, peak live heap and allocation count. These are the sizes the
job named. Printed, never asserted: the assertion is the allocation count above, which is
the same fact with no clock in it.

| n | before | after | before allocs | after allocs | before peak | after peak |
|---:|---:|---:|---:|---:|---:|---:|
| 2 000 | 4.60 ms | **1.22 ms** | 16 030 | **51** | 169 164 B | **146 392 B** |
| 16 000 | 116.01 ms | **9.08 ms** | 128 036 | **60** | 1 353 228 B | **1 171 080 B** |
| 128 000 | 10 439.15 ms | **67.46 ms** | 1 024 042 | **69** | 10 825 740 B | **9 368 584 B** |

The before column reproduces the review's measurement — `116.01 ms` at 16 000 against the
review's `115.14 ms`. Sixty-four times the nodes cost 2 269 times the time before and 55
times after, which is the quadratic and the linear.

**Note on `crates/graph-core/tests/memory.rs`.** The job named a `bench`/`memory.rs` row and
that file is outside this job's paths (another fix job owns `tests/`), so the row above lives
in `twopi/tests/measure.rs` instead and carries time, peak heap *and* the allocation count —
one more column than a memory row would have. Nothing was added to `memory.rs`.

### The declared complexity holds after the fix

`registry/radial.rs:48-49`, verbatim, still true:

```
complexity: "O(n + m): two breadth-first searches (to the nearest leaf, then from the root), \
one parent walk per leaf, and one sweep per tree node over its own neighbours"
```

Each clause now has a matching allocation: the two breadth-first searches queue in
`center::steps_to_leaf`'s one `Vec`, `tree::search` reuses `Scratch::order`, the parent walk
per leaf writes `Scratch::leaves` and allocates nothing, and the sweeps read
`Scratch::span`/`theta`. The one-sweep-per-node clause is what was false before on a
disconnected graph and is what the table above measures. The registry file is outside this
job's paths and is unchanged.

Cross-check on the registered path (`seeded_model(1, n, 9)`, a connected random graph where
the defect is nearly invisible, so this row is a no-regression witness rather than a win):

```
$ scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.twopi \
    --n 2000,16000,128000 --past-ceiling --repeat 3
n=2000    layout.twopi   0.36 ms  stress-1 0.4334  (edges=3075)
n=16000   layout.twopi   2.95 ms  stress-1 0.4335  (edges=24793)
n=128000  layout.twopi  42.65 ms  stress-1 0.4359  (edges=198365)
```

## L-08 — the neighbour order rebuilt as two sorted pair vectors

### The claim on this tree

`twopi/adjacency.rs`'s `Neighbours::of` collected `edges.source × edges.target` and
`edges.target × edges.source` into two `Vec<(u32, u32)>` — 16 bytes an edge live at once at
`RADIAL_CEILING`'s 1 000 000 nodes and 1 549 929 edges — and `sort_unstable`d each, an
`O(m log m)` pass per layout run, duplicating a row mapping `index.rs:175-185` already builds
in `O(n + m)`.

### GREEN

The fix is two stable counting sorts over edge **indices** — by the other endpoint's id, then
by the row key, least significant first — which is `O(n + m)` and keeps two `m`-long index
buffers rather than two pair vectors. `Csr::from_pairs` keeps arrival order within a row, so
a row arrives grouped and ascending by the other endpoint's id, which is exactly the order
the old lexicographic pair sort produced.

**The order could not simply be read off a topology CSR, and that is the one thing this fix
does not take literally.** `Topology::out`/`inbound` (`index.rs:177-178`) file each row's
edges by *edge index*, and an edge index is not the other endpoint's id — a row read out of
one would be a different drawing, which the module doc calls out at `twopi.rs:31-36`. So the
counting sort is still required, and `twopi/adjacency.rs:82` records that as the reason
rather than leaving the reviewer to find it. No `Ponytail:` line is added: the neighbour
order is exact, not a heuristic, so there is nothing it gets wrong.

**Why this cannot move a pixel.** Parallel edges tie on *both* sort keys, so their pairs are
equal and the row they produce is byte-for-byte the same whatever order they arrive in — the
comparator's secondary key (`e_seq`) is unobservable, exactly as before. Nothing in the
layout's arithmetic changed: `search`, `count_leaves`, `spans`, `thetas` and the polar
conversion in `angles::place` compute the same expressions in the same order, and the only
new arithmetic anywhere is `u32` counting in the sort.

### Evidence

The 12 closed-form cases in `twopi/tests.rs` pin the neighbour order node by node (the
3-path's sibling order is a direct read of "out-edges before in-edges"; the 6-branch's
`4*PI/3` split is the reference's `n4`-before-`n0` order) and all pass unchanged. So do:

```
$ scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
  layout.twopi: 4-way equal on 8/8 seeds
  ...
  4-way equal on 8/8 seeds
PASS                                                      # exit 0

$ docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
      python3 harness/oracle-twopi.py target/twopi-fixtures target/gv-twopi
twopi: 1000 seeds, worst 7.104e-02 points; closed 6 exact: True

$ scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine twopi
  layout.twopi: 1000 cases, worst 7.104e-2, ceiling 1e-1: ok
  closed cases: 6 compared byte for byte: ok
PASS                                                      # exit 0
```

`7.104e-2` is the number `registry/radial.rs:40` already records, and the negative control
still fails:

```
$ scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge                               # exit 1
```

## Output did not move

* `hashgate --seeds 8` exit 0, `layout.twopi` 4-way equal on 8/8 seeds; the
  `GM_MUTATE_REFERENCE_DEGREE=9` control exit 1.
* `oracle-graphviz --engine twopi` (the command `oracle-twopi` is now an alias of) exit 0:
  1000 cases, worst 7.104e-2, closed 6 exact — the recorded numbers.
* `scripts/scigraphs-conformance.sh`: `GRAPHVIZ_TWOPI: ok — 340 f64, 340 f32 of 1020
  coordinates, median 2.040e-10 <= 1.000e-9`, and the row's pinned **motor** digest matched,
  which is 1020 coordinates compared byte for byte against a committed hash.
* `determinism-probe` exit 0: native and wasm32 agree bit for bit on every function.

## Commands

| command | exit |
|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 |
| `scripts/orch/gr cargo clippy -q --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test -q -p graph-core --lib layout::radial -- --nocapture` | 0 |
| `scripts/orch/gr cargo test --release -p graph-core --lib layout::radial::twopi::tests::measure -- --ignored --nocapture` | 0 |
| `scripts/orch/gr cargo build -q -p graph-core --target wasm32-unknown-unknown` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 |
| `scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine twopi` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.twopi --n 2000,16000,128000 --past-ceiling --repeat 3` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- determinism-probe` | 0 |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 101 (see below) |

### Two pre-existing failures, neither in this job's paths

1. **`layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them`**
   fails because `target/probe/rank1000.txt` is absent — a generated oracle artifact this
   host has not written. Environmental, in `layout/graphviz/dot`, untouched here.
2. **`scripts/scigraphs-conformance.sh` exits 1** on `YIFAN_HU` and `GRAPHVIZ_SFDP`, both
   with `FAIL — reference bytes are not the pinned ones (sha 78ccfd5e…)`. The message is
   about the **reference** half's digest, re-hashed from disk by
   `crates/graph-cli/src/oracle_python/conformance/verdict.rs:210-217`, and both names are
   Graphviz engines SciGraphs reaches through `scigraphs_utils` — `sfdp`, whose own start is
   not seeded by `-Gstart`, which `docs/measurements/scigraphs-conformance.md:37,184` and the
   matrix's own `GRAPHVIZ_FDP` row both record. Neither row reads twopi. Every other row is
   `ok`, including `GRAPHVIZ_TWOPI`.