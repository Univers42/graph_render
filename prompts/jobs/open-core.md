# Job open-core (agent build; the 1M-node open path, graph-core StringArena and index_model)

Why: opening 1 000 000 nodes in the studio takes 14.3 s. Two id passes walk the same strings:
`graph_wasm::ingest::ids::check_ids` (934 ms) and `graph_core::index::index_model` (1988 ms). The
hottest single row of the worker is the probe inside `StringArena::find`.

Facts (profile `deploy/perf/open.py 1000000 webgl2` on the perf-p5 tree, SwiftShader, load ~10;
worker 14.77 s sampled; re-run the probe for your own baseline):
- Self: `indexmap::inner::Core<Interned,()>::get_index_of_raw` (from `StringArena::find`) 1700 ms;
  `hashbrown::RawTable::reserve_rehash` (from `indexmap::inner::get_hash<Interned>`) 271 ms; two more
  `IndexMap<Interned,..>` rows 298 ms and 215 ms.
- Inclusive: `StringArena::find` 726 ms, `StringArena::intern` 517 ms.
- `crates/graph-core/src/arena.rs`: `StringArena { text: String, spans: Vec<(u32,u32)>, lookup:
  IndexMap<Interned,(),FixedState> }`, `#[derive(Default)]` only, so every arena grows from empty and
  rehashes ~20 times on the way to 1M strings.
- `crates/graph-core/src/index.rs:45` `index_model` pre-sizes the node and edge columns but not
  `strings`, `node_ids` or `edge_ids`. `admit_edge` (line ~122) calls `edge_index` then
  `node_index` twice; each is a `strings.find` plus an `IndexSet<Interned>` lookup
  (`index/view.rs:33-42`).
- `crates/graph-wasm/src/ingest/ids.rs`: two `StringArena::default()`; node ids interned, then edge
  ids interned and both endpoints `find`-ed.

Do:
1. Additive API: `StringArena::with_capacity(strings: usize, bytes: usize)`, with a unit test next to
   the existing arena tests (same handles and same `get` as a default arena fed the same strings).
2. Use it in `check_ids` (node ids: `nodes.len()`; edge ids: `edges.len()`; bytes: the summed id
   lengths) and in `index_model` (strings and bytes counted in one pass over the records; `node_ids`
   and `edge_ids` with `IndexSet::with_capacity_and_hasher`). A count that is an upper bound
   carries a `Caveat:` line saying what it over-reserves.
3. Measure (step 4 below). Then, only if `StringArena::find` is still the top row, try one of these
   and keep it only if it wins by more than 3% and leaves every hash unchanged:
   (a) `admit_edge` resolves each endpoint once (no `find` and then a second set lookup), e.g. a
   `Vec<u32>` from arena slot to dense node index built while nodes are admitted, as a local of
   `index_model`, never a new `Topology` field;
   (b) keep the span in the map key so `find` reads one array less.
4. Measure with `scripts/studio.sh build` and then `deploy/perf/open.py 1000000 webgl2`, 3 runs
   before and 3 after, interleaved. Write `docs/measurements/open-core.md` with the `index_model`
   and `check_ids` inclusive times, the open times and the command lines.

Determinism: capacity changes a hash table's internal layout, never an `IndexMap`'s iteration
order, so every output must stay byte-identical. `hashgate --seeds 8` must pass and its negative
control must still fail. Any change to a snapshot hash or to a capabilities row means the change is
wrong: revert it.

Out of bounds: `crates/graph-contract` and `crates/graph-wasm/src/ingest.rs` (job open-ingest),
`packages/` (job open-synth), and any public signature in graph-core other than the new
`with_capacity`. Adding a dependency is a stop; graph-core's list is closed.

Paths: `crates/graph-core/src/arena.rs` (it is 185 lines; split it into a child module if it passes
300), `crates/graph-core/src/index.rs`, `crates/graph-wasm/src/ingest/ids.rs`,
`docs/measurements/open-core.md`.

Done when: quick.rows is green, including `wasm32-core`, `hashgate-8` and its negative control.
`index_model` plus `check_ids` inclusive time falls by at least 20% at 1M, with the measurement file
committed. If it falls by less, report the numbers: they are the result.
