# P4a-extend — a topology and a live force session grown by a batch, in place

The motor half of P4a (`docs/contract/delta.md`, accepted with conditions in
`docs/decisions/delta-abi.md`): `Topology::extend` appends one strict batch, and
`ForceSession::grow` takes the appended rows and edges into a running session. Both are
proven equal to the rebuild they replace. Brief: `prompts/jobs/perf-p4a-extend.md`.

## What was built

| Piece | Where | What it is |
|---|---|---|
| `AppendCsr` | `crates/graph-core/src/csr/append.rs` | One span (`start`, `len`, `cap`, all `u32`) per row over one `values` buffer. Built with zero slack through `Csr::from_pairs`. An append writes in place, pushes at the tail, or moves the row to the tail with `cap = max(4, 2·len)`. It compacts when dead slots outnumber live ones. `SAFE_LIVE` is the live count below which no append can overflow `u32`. |
| The topology on it | `index.rs`, `index/view.rs` | `out`, `inbound` and `hierarchy` are `AppendCsr`. The source set behind `group` is now the `sources` field, and `group_node(i)` files one node. |
| `Topology::extend` | `index/extend.rs` | First every refusal of `delta.md`: node id, edge id, endpoint, and a capacity check that counts every string as new. Only then, through `index_model`'s own `admit_node` and `admit_edge`: an empty row per node in each CSR, the node grouped, each edge filed in the rows `build_adjacency` gives it, and one degree per endpoint. |
| `ForceSession::grow` | `layout/force/session/grow.rs` | Takes rows `rows..` and raw edges `absorbed..`. New simple edges are deduplicated by `SimpleGraph::absorb`, which scans the smaller endpoint row. Then `edge_geometry` runs over every edge at a node whose degree moved. New rows go where carry's `beside_carried` puts them, or on `spiral_point`, at rest and free. `px`/`py` are zeroed and a particle-mesh session gets `Mesh::new(rows)`. |
| Shared, not copied | `barnes_hut/link.rs`, `barnes_hut/seed.rs`, `session/carry.rs` | `link::geometry` loops over `edge_geometry`. `golden_spiral` pushes `spiral_point(i)`. Carry's placement calls `beside_carried`. Each now has one spelling, used by both the rebuild and the grow. |

A new session field, `absorbed: u32`, records the raw edges the session has taken in. `seeded` sets
it, and every constructor goes through `seeded`.

## Gates

Each gate was run in the worktree, with `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3`, on the tree committed with this report:

| Gate | rc | Expected |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0 |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 0 |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 (`4-way equal on 8/8 seeds`, `PASS`) | 0 |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (`FAIL: 8 of 8 seeds diverge`) | 1 |
| `scripts/orch/gr cargo run -q -p graph-cli -- force-gate --seeds 4` | 0 (`force.session.positions: 4-way equal on 4/4 seeds`) | 0 |
| `~/goinfre/bench/p4a/xtree.sh <wt> > xtree-branch.out`, then `diff xtree-base.out xtree-branch.out` | 0 / 0 (12 lines, all equal) | 0 |
| `git grep -nE 'OnceLock\|OnceCell\|RefCell' crates/graph-core/src/index` | no output (rc 1) | no output |

`xtree.sh` takes the `graph-cli snapshot` sha256 of `force.barnes_hut`, `force.particle_mesh`,
`dag.sugiyama` and `circular.hierarchy` at 333, 4096 and 20000 nodes, seed 0. A frozen build kept
every byte.

## The named tests

| Test | File | What it proves |
|---|---|---|
| `extend_matches_index_model` | `index/extend/tests.rs` | 96 seeded random strict streams of 1 to 8 batches, with loops, parallel edges, every node and edge kind, and labels that name nodes not yet admitted. After each batch, two topologies are checked against `index_model` over every record so far: one extended from the first batch, one extended from `empty_model()`. They must give the same topology-stage `encode` bytes, the same `stats()` and the same node and edge indices. |
| `extend_refusal_leaves_topology_unchanged` | `index/extend/tests.rs` | One case per in-motor refusal: a node id in the graph, a node id twice in the batch, an edge id in the graph, an edge id twice in the batch, and an endpoint in neither. Each is refused with the right `ExtendError`. The bytes and the arena length are unchanged afterwards, and the next valid batch still matches the rebuild. |
| `grow_equals_carry` | `layout/force/session/tests/grow.rs` | Barnes-Hut and particle-mesh sessions, started hot. The stream is: a 12-node ring; a hub gaining 200 edges; parallel edges, one reversed with a different strength, and self-loops on an old and a new node; an empty batch; and a batch taking the node count from 213 across 256 to 300. Row 3 is pinned before the third batch. After each batch, the grown session is compared with `prev.carry(previous, topology)`, and again after 50 more ticks on both. The comparison covers `x`, `y`, `vx`, `vy`, `fx`, `fy`, the three link columns, `alpha`, `alpha_target` and `tick_no` bit for bit, plus the simple graph's `lo`, `hi` and `strength`. |

Also added:

- `the_batch_load_counts_every_string_as_new`;
- `a_batch_that_could_overflow_a_count_is_refused_before_anything_is_counted_in`;
- `an_empty_batch_changes_nothing`;
- `grow_refuses_fewer_nodes_or_edges_and_changes_nothing`;
- `absorbing_raw_edges_one_by_one_builds_the_simple_graph`.

## Negative controls

Each was run as `scripts/orch/gr cargo test -p graph-core --lib -- <test>` and then reverted. The
reverted tree is the one gated above.

| Mutation | Failing test | Output |
|---|---|---|
| `grow`'s `place` numbers new rows from 1: `(1_u32..).zip(old_rows..rows)` | `layout::force::session::tests::grow::grow_equals_carry` | rc 101, `panicked at .../session/tests/grow.rs:84:9`. The `x` column differs from row 12 on, after batch 1. |
| The two `self.nodes.degree[..] += 1` lines in `extend.rs` `file_edge` deleted | `index::extend::tests::extend_matches_index_model` | rc 101, `panicked at .../index/extend/tests.rs:76:5`, the topology stage's bytes |

## Memory

The topology was measured with `cargo test --release -p graph-core --test memory -- --ignored
--nocapture topology_memory_per_node` (counting allocator, `build_synthetic_model(n)`). Its "3
CSRs" column is the sum of `byte_len` over `out`, `inbound` and `hierarchy`:

| n | m | 3 CSRs, `AppendCsr` (measured) | 3 CSRs, `Csr` (`p1-topology-memory.md`) | difference | 3·(8n − 4) |
|---|---|---|---|---|---|
| 1 000 | 1 541 | 48 328 | 24 340 | 23 988 | 23 988 |
| 10 000 | 15 474 | 483 792 | 243 804 | 239 988 | 239 988 |
| 100 000 | 154 978 | 4 839 824 | 2 439 836 | 2 399 988 | 2 399 988 |

The difference is exactly the extra span words. Per row and per CSR, `Csr` holds one 4 B offset
and `AppendCsr` holds a 12 B span (`start`, `len`, `cap`). Per value both hold 4 B, and a fresh
build has zero slack.

At 1M nodes this is **estimated**, by extrapolating the 100 000 row:

| Part | `Csr` | `AppendCsr` |
|---|---|---|
| Per row | 4 B | 12 B |
| Per value | 4 B | 4 B |
| 3 CSRs at 1M rows and about 1.55M edges | about 24.4 MB | about 48.4 MB |

A force session's `SimpleGraph::rows` is a fourth `AppendCsr`, adding 8 B per row over its `Csr`.

The other columns of the memory table moved between the p1 measurement and this tree for reasons
outside this slice, such as the edge columns. Only the CSR column is compared here.

After growth, slack is bounded by the compaction rule rather than zero. Dead slots never outnumber
live values, and a moved row has at most twice its length in room. `SAFE_LIVE` is
`(u32::MAX − 4) / 4` because a move can need `4·live + 4` slots.

## Caveat: dedup and bias cost on hubs

**Caveat:** `grow` is O(batch) only in the batch's records. Two of its costs grow with the degree of
the nodes the batch touches:

- `SimpleGraph::absorb` dedups a new pair by scanning the smaller endpoint's row. A batch of `b`
  edges between two hubs of degree `d` costs O(b·d).
- `relink` recomputes the link geometry of every edge at a node whose degree moved, because each
  edge's `bias` reads both endpoint degrees. One edge added to a hub of degree `d` recomputes `d`
  edges.

The values are exact: they are the ones a full `link::geometry` gives. Only the work is
degree-proportional. A star that keeps growing pays its hub's degree on every batch. P4c's bench is
the place to measure whether this matters at 1M.

## What it does not do

- No ABI, no SDK, no studio: no `gm_extend`/`gm_grow` export and no JS wrapper. Those are later P4
  slices.
- No timing claim. Nothing here is benchmarked; P4c measures the per-batch cost against the 30 ms
  target.
- `grow` does not detect a topology that is not an extension of the session's own. It trusts the
  caller, as the docs on `ForceSession::grow` say.
- `ExtendError` has no variant for `MAX_INGEST_BYTES`. That limit is on the wire buffer, and the
  motor only ever sees decoded records. It belongs to the ABI slice.
- The mesh is rebuilt whole (`Mesh::new(rows)`), as `carry` does. Keeping the FFT plan when the side
  is unchanged is P4c's call.
- The adjacency limit is conservative by 4x (`ADJACENCY_LIMIT` in `extend.rs`, with its own
  `Caveat:`).
