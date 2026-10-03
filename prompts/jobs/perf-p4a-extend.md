# Job perf-p4a-extend (agent build): grow a topology and a live force session by a batch, in O(batch)

Why: P4's live growth (`docs/contract/delta.md`, accepted with conditions in
`docs/decisions/delta-abi.md`). Rebuilding costs 1.3–2.7 s plus 320–341 ms per batch at 1M
(`docs/measurements/perf-p4-carry.md`); appending is O(batch). This slice is graph-core only: the
append CSR, `Topology::extend`, `ForceSession::grow` and the tests that prove them equal to the
rebuild. No ABI, no SDK, no studio.

Facts (verified on develop 90df26ed; `delta.md` is committed on this branch and is the spec):

- **Topology** (`crates/graph-core/src/index.rs:30-41`, 230 lines). Fields:
  - the strings: `strings`, `node_ids`, `edge_ids`;
  - the columns: `nodes`, `edges`;
  - the three CSRs: `out`, `inbound`, `hierarchy`, each a `Csr`;
  - the derived data: `by_database`, `notes`.

  `index_model` (`:54-75`) admits nodes first-wins (`admit_node` `:139`) and edges in order
  (`admit_edge` `:168`), then:
  - `build_adjacency` (`:197`) builds the three CSRs from `(row, edge)` pairs in edge order,
    and the degree column as `out.row(v).len() + inbound.row(v).len()`; a hierarchy edge is filed
    under `self.parent(i)`;
  - `group_nodes` (`:210`) fills `group` from an `IndexSet` of sources (local today), then
    `by_database` and `notes`.
- **The batch is strict and `index_model` is lenient.** On a valid strict batch the two agree,
  because nothing is dropped. The refusals are listed in `delta.md` "What refuses the whole batch".
  - Validate everything before the first mutation. Interning writes to the arena, so a refusal found
    late would leave a half state.
  - Endpoint lookup on an existing graph: `strings.find(id)`, then `node_ids.get_index_of`. The
    build's `slots` table is local to `index_model` and does not survive it.
  - Arena limits: `crates/graph-core/src/arena.rs:121` `intern`, and the `CapacityError` sites at
    `:132`, `:185` and `:256`; `find` is `:150`.
- **`Csr`** (`crates/graph-core/src/csr.rs:12-60`, 172 lines): `offsets` (rows + 1) and `values`,
  built by a stable counting sort (`from_pairs` `:30`).
  - Its readers use only `row`, `rows`, `len`, `is_empty` and `byte_len` (callers:
    `git grep -nE '\.(out|inbound|hierarchy)\(\)' crates/`, 20 sites, including
    `graph-cli/src/bench/campaign.rs:201`, `graph-cli/src/oracle_python/scale.rs:268` and
    `graph-core/tests/memory.rs:78`).
  - Accessors: `index/view.rs:145-160` return `&Csr`.
- **The force session** (`layout/force/session.rs:87-97`): `ForceSession { sim, deltas, mesh }`.
  - `seeded` (`:131`) is `Sim::new(topology, params, SEED)`.
  - `Sim::new` (`barnes_hut/sim.rs:77`) does `simple_graph(topology)`, then `golden_spiral(n)`,
    then `from_parts` (`:88`), which:
    - computes `link::geometry(&graph, &params)`;
    - zeroes `vx`, `vy`, `px` and `py`;
    - sets `fx` and `fy` to `None`;
    - sets `alpha` to `params.initial_alpha`.
  - `SimpleGraph` (`layout/force/mod.rs:68`, also read by FA2) holds `lo`, `hi`, `strength` and `rows: Csr`.
    `simple_graph` (`:98`), with `row_csr` at `:133`, dedups pairs `(min, max)` first-wins in edge order and drops
    self-loops.
- **`carry`** (`session/carry.rs:73-112`, 216 lines) is the reference for `grow`:
  - it seeds `to`, sets the mesh to `Mesh::new(rows)` when the session has one, and copies
    `alpha`, `alpha_target` and `tick_no`;
  - `Placement` (`:114-216`) then copies carried rows and places new rows: `place_new` `:183`,
    `carried_mean` `:204`, `OFFSET_RADIUS`;
  - `golden_spiral` (`barnes_hut/seed.rs:11`) computes each point from its row index alone.
- **Determinism rules** (`prompt.md` §6):
  - `libm` for every transcendental, no FMA;
  - `IndexMap`/`IndexSet` with `FixedState`, never `HashMap` iteration;
  - wire integers are `u32`;
  - a new struct field needs every constructor, across crates.
- **Limits.**
  - graph-core gets no new dependency and no `unsafe`.
  - Functions ≤ 40 lines with ≤ 4 parameters; files ≤ 300 lines; nesting ≤ 3. Split into child
    modules: `index.rs` is at 230 lines, `carry.rs` at 216 and `session.rs` at 283.

Do, in order:

1. **Pin first.** In `~/goinfre/bench/p4a/`, write `xtree.sh`, modelled on
   `~/goinfre/bench/pm-stencil/xtree.sh`. It prints `graph-cli snapshot` sha256 values for these
   layouts:
   - `force.barnes_hut`;
   - `force.particle_mesh`;
   - `dag.sugiyama`;
   - `circular.hierarchy`.

   Use 333, 4096 and 20000 nodes, seed 0. Check the id form with `graph-cli snapshot --help`. Run it
   on the untouched worktree and keep `xtree-base.out`.
2. **`AppendCsr`** (new module under `crates/graph-core/src/csr/`, unit-tested on its own):
   - **Storage.** `values: Vec<u32>` and one span per row (`start`, `len`, `cap`, all `u32`).
   - **Building.** `from_pairs` builds with zero slack, so `cap == len` and the values sit in row
     order. Reuse `Csr::from_pairs` for the counting sort; don't write a second one.
   - **Appending.**
     - `push_row()` adds an empty row.
     - `append(row, value)` writes in place when `len < cap`.
     - A row ending at the tail of `values` pushes.
     - Any other row moves to the tail with `cap = max(4, 2·len)` and leaves dead slots behind.
   - **Compaction.** When dead slots exceed live ones, compact back to zero slack, in row order.
   - **Errors.** Counts are `u32`; an overflow is a `CapacityError`, never a panic or a truncation.
   - **Reading.** `row`, `rows`, `len`, `is_empty` and `byte_len` with `Csr`'s meaning. `byte_len`
     counts the allocation, slack included.
   - **Tests.**
     - Rows equal `Csr::from_pairs` over the same pairs, row by row, after:
       - a build alone;
       - random interleaved appends;
       - appends that force a move;
       - appends that force a compaction.
     - A frozen build's `values` equal `Csr`'s.
3. **Switch the topology to it.**
   - `out`, `inbound` and `hierarchy` become `AppendCsr`, and the `view.rs` accessors return
     `&AppendCsr`.
   - Fix the 20 call sites only where they fail to compile.
   - `group_nodes`'s source set becomes a `Topology` field, and the `Default` and `Clone` derives
     stay.
   - `SimpleGraph::rows` becomes `AppendCsr` too.
   - Gate condition: `git grep -nE 'OnceLock|OnceCell|RefCell' crates/graph-core/src/index`
     prints nothing.
4. **`Topology::extend(&mut self, nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<(), ExtendError>`**
   lives in `index/extend.rs`.
   - **Validate first**, per `delta.md`; `ExtendError` names which rule refused. Then mutate:
     - reuse `admit_node` and `admit_edge` (or their field-pushing halves), so the columns cannot
       drift from `index_model`'s;
     - append to the three CSRs;
     - add 1 to the degree of each endpoint;
     - update `group`, `by_database` and `notes`.
   - An empty batch is `Ok` and changes nothing.
5. **`ForceSession::grow(&mut self, topology: &Topology) -> Result<(), SessionError>`** lives in
   `session/grow.rs`.
   - **Absorb.** The session absorbs rows `[rows .. node_count)` and raw edges
     `[absorbed .. edge_count)`. Add the `absorbed` field to every constructor.
   - **Refuse.** With `SessionError::ColumnLength { column: "grow", .. }` when the topology has fewer
     nodes or edges than the session absorbed. Nothing changes on refusal.
   - **New simple edges.** Dedup each new simple edge by scanning the smaller endpoint row, then
     append it to `lo`, `hi`, `strength` and the rows.
   - **Link geometry.** Recompute `link_distance`, `link_strength` and `link_bias` for the new edges
     and for every edge of a node whose degree changed. One per-edge function, shared with
     `link::geometry`, so the two cannot drift.
   - **Placement.** Place new rows with the same function `carry`'s `Placement` uses; extract it,
     don't copy it. Push zeroed `vx` and `vy`, and set `fx` and `fy` to `None`.
   - **Scratch.** Zero `px` and `py` over every row, as `from_parts` does.
   - **The mesh.** When the session has one, it becomes `Mesh::new(rows)`.
     - That is the whole mesh, `Grid::new(rows)` included: `carry` starts `grid.order` at the
       identity, and the first deposit walks it.
     - Keeping the FFT plan when the side does not change is P4c's call, on its bench. Don't do it
       here.
6. **Tests** (graph-core, `cargo test`), with names fixed so the next slices can find them:
   - **`extend_matches_index_model`.** Generate random strict streams of 1 to 8 batches; seeded, no
     clock. The topology stage's encoding (`stage/topology.rs:50` `encode`) of the extended topology
     equals that of `index_model` over all records.
   - **`extend_refusal_leaves_topology_unchanged`.**
     - One case per refusal in `delta.md`.
     - On each, the encoding before equals the encoding after.
     - The next valid batch still extends correctly.
   - **`grow_equals_carry`**, for both Barnes-Hut and particle-mesh sessions.
     - Streams:
       - a hub that gains 200 edges;
       - parallel edges;
       - self-loops;
       - an empty batch;
       - a batch that takes the node count across a power of two.
     - Compare the grown session with `prev_session.carry(prev, topology)` after each batch, and
       again after 50 more ticks on both. These must be bit-equal (`to_bits`, `Option` included):
       - `x`, `y`, `vx`, `vy`, `fx`, `fy`;
       - `link_distance`, `link_strength`, `link_bias`;
       - `alpha`, `alpha_target`, `tick_no`.
     - Include one pinned row (`fx`/`fy` set) before a batch.
   - **Negative controls.** Run each and report the failing test's name, then revert it:
     - start `place_new`'s `fresh` counter at 1 in `grow` only: `grow_equals_carry` must fail;
     - skip the degree update in `extend`: `extend_matches_index_model` must fail.
7. **Gates.** Report the exit code of each.
   - `scripts/orch/gr cargo fmt --all --check` → 0
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` → 0
   - `scripts/orch/gr cargo test --workspace --no-fail-fast` → 0
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → 0
   - `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8`
     → 1
   - `xtree.sh` on the branch, then `diff xtree-base.out xtree-branch.out` → 0. A frozen build has
     to keep every byte.
   - the `git grep` of step 3 → no output.
8. **Report.** Write `docs/measurements/perf-p4a-extend.md`:
   - what was built;
   - the gate table;
   - the negative controls;
   - the memory change: `AppendCsr` against `Csr` bytes per row and per value at 1M, from
     `byte_len` on a 1M `graph-cli` topology, or from arithmetic labelled "estimated";
   - a `Caveat:` for the degree-proportional dedup and bias cost on hubs;
   - "what it does not do": no ABI, no timing claim (P4c measures).

   If `graph-core/tests/memory.rs` fails a ceiling, do not raise it. Report the numbers under
   "decisions needed".

Paths you may edit:

- `crates/graph-core/src/csr.rs` and a new `crates/graph-core/src/csr/`;
- `crates/graph-core/src/index.rs`, `crates/graph-core/src/index/`;
- `crates/graph-core/src/layout/force/{mod.rs,session.rs}` and
  `crates/graph-core/src/layout/force/session/`;
- `crates/graph-core/src/layout/force/barnes_hut/{sim.rs,link.rs}`;
- the call sites step 3 breaks (compile fixes only);
- `docs/measurements/perf-p4a-extend.md`;
- `~/goinfre/bench/p4a/`.

Do not touch `graph-wasm`, `graph-sdk-js`, `packages/`, `app/`, `docs/contract/` or the registry.

Done when:

- every step-7 gate has its expected exit code;
- both negative controls name their failing test;
- the three named tests exist and pass;
- the report exists.

Return block:

```
status: done | partial | blocked
tests: extend_matches_index_model <pass|fail>, extend_refusal_leaves_topology_unchanged <..>, grow_equals_carry <..>
negctl: fresh-at-1 -> <failing test>; no-degree -> <failing test>
xtree: <equal | differs: which lines>
memory: AppendCsr <bytes/row> vs Csr <bytes/row>
changed: <files>
commands: <each gate> -> <rc>
deviations: <none | list>
decisions needed: <none | list>
```
