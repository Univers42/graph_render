# Growing a live graph: append, not rebuild (P4)

Status: accepted, 2026-10-03, under full autonomy, with the devil verdict
**PROCEED-WITH-CONDITIONS**. The contract is `docs/contract/delta.md`.

## Context

- An AI process emits nodes and edges while the force layout runs. The plan's target is ≤ 30 ms per
  10 000-node batch at 1M nodes (`prompts/perf-plan.md`).
- Rebuilding cannot reach it. At 1M (`docs/measurements/perf-p4-carry.md`):
  - `index_model`: 1.3–2.7 s;
  - `ForceSession::carry`: 320–341 ms.

  Both are O(n + m) on growing data.

## Decision

1. **`Topology::extend`** appends a strict batch (C12's rules on the cumulative graph). It validates
   first, then mutates, so a refused batch leaves the topology unchanged.
2. **`ForceSession::grow`** absorbs what the topology gained. Afterwards the session is byte-identical
   to `carry(prev, topology)`, and so is every later tick. `carry` stays the reference.
3. **Two wasm exports**, `gm_graph_extend` and `gm_force_session_grow`, with no new error code.
   `ABI_VERSION` stays 1:
   - no existing signature changes;
   - no existing refusal code changes its meaning;
   - no accepted document version changes.

   The new exports add invalidation events (C7), and only to hosts that call them.
4. **The studio** applies queued batches between ticks and reheats. The reheat is its policy, not the
   motor's.

## Verdict

Risk scores: blast 4, reversibility 3, cost on failure 3, confidence 4.

The worst axis is blast. The change touches the topology every layout reads, a public ABI and the
SDK.

The conditions are the slices' acceptance criteria:

1. **One append-CSR primitive** in graph-core, with unit tests:
   - its rows equal `Csr::from_pairs` over the same pairs, compared logically (row by row), not by
     storage;
   - the topology's CSRs stay eager;
   - `git grep -nE 'OnceLock|OnceCell|RefCell' crates/graph-core/src/index` prints nothing;
   - `extend` takes `&mut self` and validates, then mutates.
2. **Two tests:**
   - `extend_matches_index_model`: over random strict streams, the topology stage's encoding
     (`stage/topology.rs`) of the extended topology equals that of `index_model(all)`;
   - `extend_refusal_leaves_topology_unchanged`: one case per refusal in `delta.md`.
3. **`grow_equals_carry` for both engines.** After each batch, and after 50 more ticks, these are
   byte-equal:
   - `x`, `y`, `vx`, `vy`, `fx`, `fy`;
   - the three link columns;
   - `alpha`, `alpha_target`, `tick_no`.

   The streams include:
   - a hub;
   - parallel edges;
   - self-loops;
   - an empty batch;
   - a batch whose node count crosses a power of two.

   `grow` rebuilds the collide `Grid::new(rows)`: `carry` resets `grid.order` to identity, and the
   first deposit walks it.
4. **The ABI docs say it.**
   - `wasm-abi.md`'s version rule (`:31`) and C7 state that a new export may add an invalidation
     event without a version bump, and that no existing export's refusal changes.
   - These comments, which claim a fixed topology or never-resized columns, are rewritten:
     - `graph-wasm/src/handle.rs:24-28`;
     - `graph-wasm/src/session.rs:198-205`;
     - `barnes_hut/link.rs:30-31`.
   - `codegen --check` and `capabilities --check` are green.
5. **The SDK invalidates views.**
   - `Motor.extend` calls `views.bump()`. An `sdk:test` reads a fresh view after an extend.
   - `ForceSession.grow` sets `#views` to null.
6. **The stream arm of the hash gate is green**, and its negative control `GM_MUTATE_DROP_DELTA=1`
   exits non-zero.
7. **A 0 → 1M stream bench**, native and wasm, reports per batch:
   - the median, p95 and max;
   - the load average.

   It is recorded in `docs/measurements/perf-p4-delta.md`. The 30 ms target gates on the median
   only. A miss is recorded, and the row stays `implemented`.
8. **The studio's motor worker logs `(tick_no, batch)` for each grow.** A render test shows that pick
   and hover ignore rows ≥ `nodeCount`.
9. **The merge floor is green** on every slice.

The verdict also found two gaps in the first draft of `delta.md`, now fixed there:

- The cost table left out the O(n) work done per batch:
  - zeroing `px` and `py`;
  - `Grid::new`;
  - the mesh's per-node slots.

  It also left out the reallocation spikes.
- The CSRs were lazy. That would have put interior mutability into a type the layouts share.

## Slices

| slice | branch | scope | conditions |
|---|---|---|---|
| P4a | `perf-p4a-extend` | graph-core: the append CSR, `Topology::extend`, `ForceSession::grow`, their tests | 1, 2, 3, 9 |
| P4b | `perf-p4b-abi` | the wasm exports, the SDK, the ABI docs, stream fixtures and the hash-gate arm | 4, 5, 6, 9 |
| P4c | `perf-p4c-stream` | the stream bench and the studio's `applyDeltas` | 7, 8, 9 |

P4b starts from P4a once P4a lands. P4c starts from P4b.

## Consequences

- Removals and attribute edits still go through a rebuild plus `carry`.
- New nodes are drawn only from the next structure snapshot on, up to 500 ms later, until the
  renderer, which a peer owns, gains an append path.
- Caveat: the simple-edge dedup and the link-bias update cost time proportional to degree, so a
  batch that touches a hub is slow but exact. The way out is a pair index, at about 32 bytes per
  simple edge.
