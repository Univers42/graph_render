# perf-p4c-studio — `applyDeltas` in the studio, one grow per frame

What the host verb does, what the worker does with it, what the page draws, and the gate table
with the exit codes each row was run at (commit `0eb0858e`).

## What was built

**Protocol** (`src/motor/protocol.ts`). `force.deltas { batch }` is its own `Request` member, not a
`ForceRequest`, because a force request is answered synchronously from the loop's own state and this
one is answered by the tick that applied it. Two results: `deltas-applied { applied, nodeCount }`
and `deltas-structure { run }` — the structure snapshot the worker rebuilt, pushed because no
request is waiting for it. `GraphBatch` is the motor's own ingest shape (`DeltaNode`, `DeltaEdge`).

**Worker queue** (`src/motor/deltas.ts`, new). Batches are queued and drained at the top of every
frame: one `Motor.extend` per batch in arrival order, then one `ForceSession.grow` and one
`reheat(max(alpha, 0.3))` for the burst, then one answer per batch. A refused batch answers
`failed` with the motor's own typed error, the graph is as it was, and the rest of the burst still
applies. The `(tick, batch)` of every grow goes into a ring of the last 1024, readable through
`ForceHost.grows()`. The structure snapshot is rebuilt at most twice a second, or at once when the
tick before was empty — the burst is over, so there is nothing to wait for. `LoopDeps.now` is the
clock; no motor file reads `Date.now()`.

**Loop** (`src/motor/loop.ts`, split out of `liveLoop.ts` to stay under 300 lines). The class owns
the queue: it asks for a frame when a batch arrives on a settle that is not running one, refuses a
batch while the settle is paused, and refuses what is still queued when the session ends.

**Session** (`src/motor/session.ts`). `MotorLike.extend` is optional and the graph's node list grows
with it, after the motor has taken the batch: the studio must not be the one to say a snapshot holds
more nodes than the document does. `ForcePort` grows a `Growable<Handle>` member for the motor's own
`grow(handle)` — the studio's port addresses rows, not handles.

**Page** (`src/motor/deltasPage.ts`, new; `src/element.ts`; `src/actions/registry.ts`). The verb is
`APPLY_DELTAS` with `deltaBatch`, registered in `actions/registry.ts` next to `resolve`, which checks
the batch member by member and refuses it before the worker is asked. `element.applyDeltas(batch)`
resolves `{ applied }`; a refusal rejects with the motor's typed error and dispatches
`graph-error` on the element, whose `detail.error` is that error's `name`. The main thread draws
`xs.subarray(0, nodeCount)`: Caveat: new nodes move in the motor but are not drawn for up to 500 ms,
until the next structure snapshot.

`hostApi` stays `1`. The break control (`?break-deltas=1`, read by `element.ts` and carried on
`open`) makes the worker drop the `grow` after an extend; nothing else reads it.

## Gate table

Every row of `target/wf/p4c-studio.rows`, run one at a time. `scripts/orch/gate.sh` is the
orchestrator's row, not this job's.

| row | expect | command | exit |
|---|---|---|---|
| `studio-check` | 0 | `scripts/studio.sh check` | 0 |
| `delta` | 0 | `scripts/studio-delta.sh` | 0 |
| `negctl-delta` | non-zero | `STUDIO_DELTA_BREAK=1 scripts/studio-delta.sh` | 1 |
| `live` | 0 | `scripts/studio-live.sh` | 0 |
| `smoke` | 0 | `scripts/studio-smoke.sh` | 0 |
| `negctl-smoke` | non-zero | `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 |

`scripts/studio.sh wasm` → 0 (staged 1 618 741 bytes serial, 1 733 237 bytes threads).
`scripts/studio.sh check` → 0: 524 + 641 unit tests, 100 render tests, 0 skipped, eslint clean.

The four rows of `studio-delta`, and what they measured:

| row | measured | verdict |
|---|---|---|
| `deltas-drawn` | 1400 drawn of 1400 wanted, 1000 nodes applied, after 0.10 s | PASS |
| `deltas-moving` | largest travel 229.300 world units over 0.4 s | PASS |
| `deltas-clean` | no page or motor-worker exception, no console or Log error, no banner | PASS |
| `deltas-refused` | a batch naming `n-0`, already in the graph, rejected; 1400 drawn before and after | PASS |

The negative control drops the worker's `grow`: the batches still go in and the drawing still grows
(the snapshot carries the new nodes either way), so `deltas-moving` is the row that goes red —
largest travel 0.000 world units — and the gate exits 1.

## What it does not do

- **No renderer append path.** The view is handed a new frame; the pages draw
  `subarray(0, nodeCount)` and nothing in `graph-render` changed.
- **Removals and edits go through a reload.** `applyDeltas` only adds; a host that needs a node gone
  reloads the source, as `delta.md` says it does for this slice.
- **No timing.** This slice is the verb and the cadence; the numbers are P4c-bench's job.
- **The structure snapshot is not a layout result the studio reports.** It is an O(n) scatter
  (`layout.random`, `toBytes`, `describe`) rebuilt at most twice a second; `state.run` and
  `settings.layout` are left alone, so the console and the layout list still name the layout the
  reader chose.
- **A paused settle refuses a batch** rather than ticking behind the user's back.
- **A batch's node kind must be one of the studio's four** (and an edge's one of its five): the
  description the drawing and the lookup are built from is typed by them.
- `deltas-applied.nodeCount` is the count after the whole burst, so every batch in one burst answers
  the same count.