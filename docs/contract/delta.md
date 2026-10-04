# Growing a live graph — the delta contract (P4)

Status: **accepted** with conditions, 2026-10-03 (`docs/decisions/delta-abi.md`). Being implemented
in slices P4a–P4c.

An AI process emits nodes and edges while the force layout runs. Each emission is a **batch**. The
motor appends the batch to the graph and to the live session in O(batch), and the next tick moves the
grown graph. The layout continues; it never restarts.

## Why append, not rebuild

`docs/measurements/perf-p4-carry.md` measured the rebuild path at 1M nodes with a 10 000-node batch,
against the plan's 30 ms per batch (`prompts/perf-plan.md:118`):

| step | 1M median |
|---|---|
| `index_model` over every record | 1.3–2.7 s |
| `ForceSession::carry` | 320–341 ms |
| `ForceSession::from_frozen` alone | 180–195 ms |

Every row is O(n + m) on growing data, so no rebuild can reach 30 ms at 1M. Appending is O(batch),
amortised. `carry` stays: it is the reference the append path must equal, and the path for any change
v1 does not cover.

## The batch

A batch is the ingest JSON v1 document (`docs/contract/wasm-abi.md`, "Ingest — PROVISIONAL", C13),
read by the same `read_records`. There is no new schema and no codegen change.

```json
{"version":1,
 "nodes":[{"id":"n9","kind":"note","database_id":null,"source":"ai","label":"n9","group":null,
           "weight":1,"version":0,"has_note":false,"icon":null}],
 "edges":[{"id":"e9","source":"n1","target":"n9","kind":"link","label":"","record_id":null,
           "strength":1,"directed":false}]}
```

What a batch accepts, beyond C13's shape rules:

- An edge endpoint may name a node already in the graph or a node of the same batch.
- An empty batch (`"nodes":[]`, `"edges":[]`) is accepted and changes nothing.

What refuses the whole batch, with nothing changed:

- A node id already in the graph or repeated in the batch.
- An edge id already in the graph or repeated in the batch.
- An endpoint that names neither a node in the graph nor a node in the batch.
- A batch whose strings could overflow the string arena or whose counts could overflow `u32`. The
  check is conservative: it counts every string in the batch as new.
- A batch longer than `MAX_INGEST_BYTES`.

These are C12's rules, applied to the cumulative graph. A graph grown by valid batches is therefore
the graph `index_model` builds from all the records in order, with nothing dropped.

## The motor: two operations

### `Topology::extend(&mut self, nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<(), ExtendError>`

- Validates everything first, then mutates. On `Err`, `self` is unchanged.
- Appends to the arena, the id sets and the node and edge columns in O(batch).
- Updates the degree column, `group`, `by_database` and the note count incrementally and exactly.
  `group` keeps its source-to-group set between calls.
- The out, inbound and hierarchy CSRs stay eager. They are append CSRs (below): `extend` appends
  each new edge to its rows in O(1) amortised. No interior mutability: no `OnceLock`, `OnceCell` or
  `RefCell` under `crates/graph-core/src/index`.

Equivalence: after any sequence of valid batches, every public accessor of the extended topology
returns what `index_model(all records)` returns: counts, `stats`, `node_index`, `edge_index`, `node`,
`edge`, `parent`, `child`, `incident`, `by_database`, the columns and the three CSRs, row by row.
Strings are compared after resolution through `strings().get`. Raw `Interned` values, the arena's
slot order and a CSR's storage layout are not equal and are not promised. The topology stage's
encoding (`stage/topology.rs`) is equal, which is what the stage hash sees.

### `ForceSession::grow(&mut self, topology: &Topology) -> Result<(), SessionError>`

- The session absorbs nodes `[rows .. node_count)` and raw edges `[absorbed .. edge_count)`. It
  records the raw edge count it has absorbed.
- Refused with `SessionError::ColumnLength { column: "grow", .. }` when `topology` has fewer nodes or
  fewer edges than the session has absorbed, and with `SessionError::Capacity` when the new edges
  could overflow the simple graph's `u32` adjacency (`AppendCsr::SAFE_LIVE`). Nothing changes on
  refusal.
- Precondition: `topology` is the topology the session was built over, extended. The motor cannot
  check this in O(batch). Breaking it is memory-safe but gives a meaningless layout. The wasm layer
  enforces it by graph id.

Equivalence: after `grow`, the session is **byte-identical** to `self.carry(prev, topology)`, where
`prev` is the topology before the batch. That covers every column, the pins, `alpha`, `alpha_target`,
`tick_no`, the simple graph, the link geometry and the mesh, for both engines. Every later tick is
byte-identical too. The rules `grow` reproduces from `session/carry.rs`:

- Carried nodes keep `x`, `y`, `vx`, `vy`, `fx` and `fy`.
- A new node with neighbours among the carried nodes starts at the mean of their positions, plus
  `1.0·(cos, sin)(fresh·2.399963229728653)`. `fresh` counts the batch's new nodes from 0, in row order.
- A new node with no carried neighbour starts at `golden_spiral`'s point for its row.
- New nodes start at rest, with no pin.
- `px` and `py` are zeroed for every row, as `Sim::from_parts` zeroes them.
- The particle mesh is equal to `Mesh::new(rows)`, its collide grid included: `grow` rebuilds
  `Grid::new(rows)`, because `carry` starts `grid.order` at the identity and the first deposit walks
  it. P4a rebuilds the whole mesh with `Mesh::new(rows)`. Keeping the FFT plan, the kernel and the
  spectrum when the mesh's side does not change is P4c's (`prompts/jobs/perf-p4a-extend.md`).

`grow` and `carry` share one placement function and one per-edge geometry function, so the two
paths cannot drift.

Costs, all amortised per batch of `b` nodes and `k` edges:

| part | cost |
|---|---|
| node columns | O(b) |
| simple-edge dedup | O(k · min degree of the pair) |
| simple-graph rows (append CSR with slack) | O(k) amortised |
| link distance and strength | O(new simple edges) |
| link bias | O(Σ degree of every node whose degree changed) |
| `px`, `py` zeroed | O(n) memset |
| collide `Grid::new(rows)` | O(n) |
| mesh, rebuilt every batch in P4a | `Mesh::new`, O(n + P² log P) |
| mesh, side unchanged (P4c) | O(n) memset of the per-node slots |
| a column at capacity | O(n) copy, once per doubling of that column |

The O(n) rows are memsets and sequential writes over 8 MB columns at 1M. They set the floor of a
batch's cost; P4c measures it. Reallocation makes the per-batch maximum much larger than the median, so
the bench reports the median, p95 and max, and the 30 ms target gates on the median.

Caveat: the dedup scan and the bias update are proportional to degree. A batch of edges between two
hubs of degree 100 000 scans 100 000 slots per edge, and a hub that gains one edge recomputes the
bias of all its edges. Both are exact, only slow on hubs. The escape hatch is a pair index, which
costs about 32 bytes per simple edge.

The simple graph's rows and the topology's three CSRs become one primitive, an append CSR. Each row
stays contiguous and in ascending edge order.
A row with no free slot moves to the tail with double the capacity. The rows are compacted when the
dead slots outnumber the live ones. A graph that never grows has zero slack and the row order `Csr`
gives today, but every row pays a 12 B span against `Csr`'s 4 B offset, grown or not
(`docs/decisions/delta-abi.md`, "Memory"). Every reader of `SimpleGraph::rows` keeps `row()` and `rows()`
unchanged.

## The wasm ABI: two exports

| export | signature | notes |
|---|---|---|
| `gm_graph_extend` | `(graph: u32, ptr: u32, len: u32) -> u32` | `(ptr, len)` is a live `gm_alloc` buffer holding one batch (C5). It is copied, never freed (C7). Returns `1` when appended, `0` on refusal: `InvalidHandle`, `BuildSourceInvalid` (the buffer is not a live allocation, as `gm_build`), `IngestInvalid` or `IngestTooLarge`. On success it clears the handle's snapshot and geometry, so the next read is `NoGeometryYet` until a `gm_run`, and every column address read from that handle before is invalid. |
| `gm_force_session_grow` | `(session: u32, graph: u32) -> u32` | Absorbs what `graph` gained since the session last saw it. Returns `1`, or `0` on refusal: `InvalidSession` (as every `gm_force_session_*` call), `InvalidHandle`, and `SessionRefused` when `graph` is not the graph the session was created over or has fewer nodes or edges than it absorbed. Every column address read from the session before is invalid: its columns may have moved. |

- The session records its graph id at `gm_force_session_create`. Graph ids are never reissued (C6), so
  a recorded id cannot alias a later graph. A released graph refuses with `InvalidHandle`.
- No new error code. Codes 20 to 23 are being claimed by other branches, so reusing the existing ones
  avoids a renumbering.
- `gm_abi_version` stays 2. Adding an export changes no signature, no code's meaning and no
  document version (`crates/graph-wasm/src/lib.rs:132-136`, `wasm-abi.md:31`). The two exports add
  invalidation events to C7, but a host that never calls them sees no change. P4b writes that rule
  into `wasm-abi.md`'s version line and C7. It also rewrites the comments that promise a fixed
  topology or never-resized session columns: `graph-wasm/src/handle.rs:24-28`,
  `graph-wasm/src/session.rs:198-205` and `barnes_hut/link.rs:30-31`. The SDK adds both names to
  `EXPORT_NAMES` (`crates/graph-sdk-js/src/wasm.ts:60-71`), so a module built before them is refused
  by name at load.
- These are the plan's `gm_force_session_apply`, split in two. Extending is a graph operation and
  growing is a session operation. A graph can carry several sessions, and the snapshot path
  (`gm_run`, then `gm_snapshot_bytes`) needs the extended graph without a session.

## The SDK

- `Motor.extend(handle, { nodes, edges })` encodes the batch as JSON v1, calls `gm_graph_extend`,
  then `views.bump()` (`crates/graph-sdk-js/src/index.ts:192` shows the pattern), so every column
  view taken from that handle before is stale.
- `ForceSession.grow(handle)` calls `gm_force_session_grow`, then sets `#views` to null, as `tick`
  already does (`crates/graph-sdk-js/src/force.ts:123`).
- Both throw the existing typed error on refusal.

## The studio

- `applyDeltas(batch)` is a host action registered in `src/actions/registry.ts`. It sends one
  protocol message to the motor worker.
- The worker queues batches. Before the next tick it applies every queued batch: one `extend` per
  batch, then one `grow`. Then it reheats to `max(alpha, 0.3)` through the existing
  `gm_force_session_reheat`. The reheat is the studio's policy, not the motor's.
- Position frames keep the frame rate. A structure snapshot is rebuilt at most twice a second, or
  when the queue drains: `layout.random`, then `toBytes`, then `describe`. Until it arrives, the main
  thread draws the first `nodeCount` entries of each frame (`subarray`), so the renderer needs no
  append path.
- The worker logs `(tick_no, batch)` for each grow, so a live session can be replayed.
- Pick and hover ignore rows ≥ the structure snapshot's `nodeCount`.
- Caveat: between two structure snapshots, new nodes and edges move in the motor but are not drawn.
  The lag is at most one cadence period, 500 ms.

## Replay and gate

- `graph-cli emit-stream-fixtures` writes `fixtures/stream-*.jsonl`: one batch per line, the first
  line being the initial graph.
- The `force-gate` stream stage replays a fixture natively twice and in wasm twice. It ticks a fixed
  count between batches and hashes the session's columns after each batch. All four hashes must be
  equal per batch. A native-only check also compares each batch's session against the rebuild plus
  `carry` reference.
- Negative control: `GM_MUTATE_DROP_DELTA=<k>` drops batch `k` in one arm, and the gate must go red.
  The knob is added once, in `crates/graph-cli/tests/common/mod.rs`.

## Done when (the plan's exit, restated)

- At 1M nodes with 10 000-node batches, `extend` plus `grow` takes ≤ 30 ms per batch, natively and in
  wasm, as the median of 3 alternated rounds. The p95 and the max are reported beside it, with the
  host load. A miss is recorded, and the row stays `implemented`.
- The structure snapshot cost at 1M is measured and reported.
- The `force-gate` stream stage is green and its negative control is red.

## What v1 does not do

- **Removals and attribute edits.** They go through rebuild plus `carry`, which is correct and
  O(n + m). A later slice can make them incremental.
- **Lenient batches.** A batch is strict. A host that wants first-wins dedup filters its batch first.
- **Renderer append.** The renderer still takes whole snapshots. Drawing new elements between
  structure snapshots needs an append path in `packages/graph-render`, which a peer owns.
- **Ingest contract batches.** Only the provisional v1 shape is read. `gm_build_contract`'s format
  has no extend path.
