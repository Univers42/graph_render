# Job perf-p4c-studio (agent build): `applyDeltas` in the studio, one grow per frame

Why: service-dod step 3 ends with a host calling `applyDeltas(batch)` on `<graph-studio>` and the live
layout growing without a restart. `docs/contract/delta.md` §"The studio" is the spec; P4b gave the SDK
`Motor.extend` and `ForceSession.grow`. `docs/contract/host-api.md` condition 8 binds the element method:
**atomic per call, no coalescing across calls, `DeltaResult` without a per-id `refused[]`**. "Coalesced
per animation frame" means one `grow` and one reheat per tick for every batch queued since the last
tick; each call still succeeds or fails whole and gets its own answer.

Facts (develop 873f168e; re-check on the tree you start from, perf-p4b-abi after P4b's commits):
- `packages/graph-studio/src/motor/protocol.ts:58-72`: `Request` (`open`, `load`, `layout`,
  `analysis`) and `ForceRequest` (`force.start`, `force.drag`, `force.release`, `force.params`,
  `force.pause`, `force.resume`, `force.stop`); `:91-98` `Result`; `:83-90` `ForceFrame` (`xs`, `ys` f32,
  `alpha`, `running`).
- The live loop: `motor/liveLoop.ts` (253 lines; `DEFAULT_PERIOD_MS = 16`, reheat at `:102-106`), the
  session adapter `motor/liveSession.ts` (142 lines; `createLiveForce` `:123`, `reheat` `:138`, the id→row
  table `rowsOf` `:55`), the port interface `motor/live.ts:95-103`.
- The structure snapshot path: `motor/session.ts` (298 lines; `toBytes` `:37`, `describe` `:138`).
- Every user action is registered once in `src/actions/registry.ts` and called through `resolve`.
- `packages/graph-studio/src/element.ts` (248 lines). host-api v1 does not declare `applyDeltas`
  (condition 8): this slice adds it, additively, and bumps nothing (`hostApi` stays `1`).
- Pick and hover live in `packages/graph-render` (peer-owned: do not edit it). They see only the frame
  the studio hands them, so drawing `xs.subarray(0, nodeCount)` makes them ignore rows ≥ `nodeCount`
  with no renderer change.
- `scripts/studio-live.sh` (`deploy/nav/live.py`) is the live-force browser gate to model the new one on.
- Peer branch `host-api` (peer 41) will edit `element.ts` for `load`, `focus`, `select` and events.
  Edit `element.ts` additively; if it lands first, merge develop and keep both.

Do, in order:
1. **Protocol.** `force.deltas { batch }` request and `deltas-applied { applied, nodeCount }` /
   `failed` results, one answer per request, keyed by `seq`.
2. **Worker.** Queue batches. Before the next tick: one `Motor.extend` per queued batch (a refused batch
   answers `failed` with the SDK's typed error and leaves the graph as it was), then one
   `ForceSession.grow`, then `reheat(max(alpha, 0.3))`. Log `(tick_no, batch_index)` per grow to a bounded
   ring (last 1 024) readable from the worker for replay. Rebuild the structure snapshot at most twice a
   second, or when the queue drains.
3. **Main thread.** Draw `subarray(0, nodeCount)` of each frame until the next structure snapshot.
   `Caveat:` new nodes move in the motor but are not drawn for up to 500 ms.
4. **Action and element.** Register `applyDeltas` in `src/actions/registry.ts` (arguments checked
   there). `element.applyDeltas(batch): Promise<{ applied: number }>` calls it; a refusal rejects with
   the typed error whose `name` equals `graph-error.detail.error`, and emits `graph-error`.
5. **Unit tests** (`node:test`, ≤ 300 lines a file): three calls queued before one tick give three
   extends, one grow and three answers; a refused middle batch answers `failed` and leaves the other two
   applied; the log ring is bounded.
6. **Browser gate** `scripts/studio-delta.sh` (+ `deploy/nav/delta.py`), modelled on `studio-live.sh`:
   load a fixture, start forces, call `el.applyDeltas` 10 times with 100 nodes each, then check: the drawn
   node count reaches base + 1 000 within 2 s; the layout keeps moving (largest travel over 0.4 s > 1
   world unit); no page error; one refused batch (a duplicate id) rejects and draws nothing new.
   `STUDIO_DELTA_BREAK=1` drops the worker's `grow` call and must exit non-zero.
7. **Rows** `scripts/orch/rows/perf-p4c-studio.rows`: `studio-check`, `delta`, `negctl-delta`, `live`,
   `smoke`, `negctl-smoke` (copy the shape of `perf-p6-data.rows`).
8. **Report** `docs/measurements/perf-p4c-studio.md`: what was built, the gate table, what it does not do
   (no renderer append path; removals and edits go through a reload).

Paths you may edit: `packages/graph-studio/src/motor/*`, `src/actions/registry.ts` (additive),
`src/element.ts` (additive), new tests under `packages/graph-studio/tests/`, `scripts/studio-delta.sh`,
`deploy/nav/delta.py`, `scripts/orch/rows/perf-p4c-studio.rows`, the report. Not `packages/graph-render`,
`crates/`, `app/`. No new dependency, no type assertions, 40 lines a function, 300 a file.

Done when:
- `scripts/studio.sh check` exits 0;
- `scripts/orch/gate.sh target/rows-p4c-studio scripts/orch/rows/perf-p4c-studio.rows` writes a
  `summary.txt` with every row PASS (negative controls exit non-zero).

Return: the branch tip, the gate summary lines, and every deviation from this brief.
