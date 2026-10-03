# Perf — Animate (`force.start`) really restarts the settle

## Design

`forces.animate` (`packages/graph-studio/src/actions/forces.ts:86`) reaches the worker as
`force.start`, which `ForceLoop.apply` answers with `drop()` and `shuffle()`
(`packages/graph-studio/src/motor/liveLoop.ts:185-193`).

What `shuffle()` did before this change: `deps.scatter(deps.handle)` — the motor's own
`layout.random` over the graph handle — then `session.reheat(1)`, and it answered `1`. The graph
handle's layout columns are not the force session's position columns: `ForceSession::new` seeds its
own (`crates/graph-core/src/layout/force/session.rs:114,133`), so the scatter wrote a layout nothing
read, at O(n) plus the layout stage, and the settle then carried on from wherever it had stopped.
That is why three comments claimed Animate "throws the nodes back to random positions"
(`live.ts`, `liveSession.ts`, `liveLoop.ts`): they described the scatter, not the drawing.

What it does now: `deps.restart()` releases the session and answers a new one over the same handle
and engine, the knobs are written back with `setParams`, and the alpha answered is the new session's
own. The `LiveForce` object keeps its identity, so the stepping loop does not replace itself, and the
row table the port already holds is still the row table: a fresh session over the same topology has
the same dense order.

The unit row is `packages/graph-studio/tests/live-session.motor.test.ts`, "Animate restarts the
settle from the session's own start positions": it ticks the real wasm session 20 times, presses
Animate, and asks a second real session — never stepped — for its start positions and its born alpha.
It fails on develop (the nodes are 40.36, -18.41 … where the fresh session starts at 12, 12) and
passes on the branch. It also reads the four knobs back through the ABI afterwards, which is the part
a restart could silently drop.

## Browser, 1 000 000 nodes

Host: this worktree, 20 cores, `gm-chromium` (Chrome/154.0.8037.92), `GM_GPU=1`, renderer
`ANGLE (AMD, Vulkan 1.4.305 (AMD Radeon RX 6600 (RADV NAVI23)), radv)`. Load average over the six
runs below: **6.5 – 14.0** (1-minute), from other jobs on the host; printed per run in the last
column.

Graph: `layout.force.particle_mesh`, synthetic `random`, degree 2, seed 1, 1 000 000 nodes — the perf
driver's own graph (`deploy/perf/drivers/hook.js`), so the run is one a user could type.

**Method.** No studio code is changed to be measured. A script installed before the page loads wraps
`Worker`, stamps `performance.mark` on every force request the page posts and on the answer the
worker sends back with the same seq, and takes a `performance.measure` across the pair. A worker
handles a force request synchronously in `serve`
(`packages/graph-studio/src/motor/serve.ts:15-19`), so that measure is the worker's own handling of
`force.start` — `drop()` plus `shuffle()` — plus one `postMessage` each way. Each round presses
Pause first and waits for the frame count to stop moving, so no frame is in flight behind the press
and the first frame after it is the restarted settle's own.

| round | arm | `force.start` answer (ms) | `force.params` control (ms) | answer − control (ms) | first frame after it (ms) | alpha on that frame | load average |
|---|---|---|---|---|---|---|---|
| 1 | before | 1068.3 | 21.3 | 1047.0 | 1072.9 | 1 | 9.61 |
| 2 | before | 677.6 | 14.2 | 663.4 | 700.4 | 1 | 13.06 |
| 3 | before | 603.5 | 20.3 | 583.2 | 623.9 | 1 | 9.31 |
| **median** | **before** | **677.6** | **20.3** | **657.3** | **700.4** | 1 | 9.31 – 13.06 |
| 1 | after | 366.1 | 19.6 | 346.5 | 370.6 | 1 | 6.48 |
| 2 | after | 354.6 | 19.1 | 335.5 | 359.0 | 1 | 7.29 |
| 3 | after | 368.2 | 19.2 | 349.0 | 372.7 | 1 | 14.04 |
| **median** | **after** | **366.1** | **19.2** | **346.9** | **370.6** | 1 | 6.48 – 14.04 |

**Read: 677.6 ms → 366.1 ms of worker's own handling, a 1.9× cut; 657.3 ms → 346.9 ms against the
`force.params` control, 1.9×.** The control is flat across the arms (20.3 vs 19.2 ms median), so the
difference is the restart and not the transport. The time to the first frame of the restarted settle
tracks it (700.4 → 370.6 ms): the loop is timer-paced, so the frame lands one frame after the worker
answers, not after a whole settle.

The two arms answer the same alpha (1) — that is the point of the change, not an accident: before, the
port hard-coded `1` and reheated; now the number is the new session's own `initial_alpha`, read through
the ABI.

**Caveats, in order of how much they move the number.**

- `answer_ms` carries the worker's event-loop turn as well as its work. That is what the control row is
  for; read the difference, not the sum.
- The host was not quiet (6.5 – 14.0 on 20 cores). The before arm has the wider spread — 603 to
  1068 ms — because a whole `layout.random` at 1M nodes is the larger and more interruptible job. Three
  alternated rounds and a median are what this table is worth; a single before/after pair is not.
- `frame_ms` is the first frame, which is one timer frame after the answer. It is not the settle: at
  1M nodes a `particle_mesh` tick is seconds, so nothing here says how long the settle takes.
- Both arms are one host, one browser build, one graph. The ratio is the claim; the absolute numbers
  are this host's.

## What it does not do

- **It does not make the first load cheaper.** The first load still builds a snapshot: a layout run,
  `toBytes`, and the digest the studio shows. That is `perf-snapshot-build`, a different job. This
  change is only about what happens when the user presses Animate on a graph that is already drawn.
- **It does not remove the O(n) from a restart.** A restart still costs what a new force session costs
  at 1M nodes — 346.9 ms against a control that does nothing but write thirteen parameters — because
  `gm_force_session_create` seeds the positions and builds the session's own structures. What is gone
  is the layout stage that ran on top of it and whose result nothing read. A cheaper restart means a
  cheaper session create in the ABI, which is out of this job's paths.
- **It does not change the drawing's look.** The restart lands on the session's deterministic seed
  positions, which is the same draw every time and the same one the settle started from. Before, the
  scatter moved a layout the drawing never showed, so the nodes never visibly moved on Animate at all.
- **It does not survive a graph whose node count changed.** A restart keeps the port's existing row
  table, so pins and rows from a previous topology stay as they were. That was already true of a
  re-layout before this change; `liveSession.ts` keeps the caveat and the fix (re-layout, which makes
  a new session and a new port).
- **It is not measured in Node.** The numbers above are the browser, through a worker, on the hardware
  arm. The Node unit row proves the semantics; it proves nothing about the cost.

## Reproducing

The probe is not in the tree (this job's paths do not include `deploy/`). It is the driver above plus
two builds of `app/dist`: one from this branch, one with `live.ts`, `liveSession.ts` and `session.ts`
replaced by their `develop` versions — the only three files the change touches, so the worker bundles
differ in exactly that code.

```sh
scripts/studio.sh build                       # stages the wasm and the fixtures, builds app/dist
# the "before" arm: put the develop versions of the three files back, vite build --outDir <other>
for i in 1 2 3; do for arm in before after; do
  GM_GPU=1 docker run --rm --memory 12g --memory-swap 12g -e GM_GPU=1 \
    --device /dev/dri --group-add "$(getent group render | cut -d: -f3)" \
    --group-add "$(getent group video | cut -d: -f3)" \
    -v "$PWD:/w" -v <probe>:/probe -w /w gm-chromium \
    python3 /probe/animate.py "target/animate/dist-$arm" "$arm" 1000000 1
done; done
```

## Gates run on this tree

| command | exit | note |
|---|---|---|
| `scripts/studio.sh check` | 0 | tsc ×4, 453 + 571 unit + 98 render tests, eslint, vite build |
| `scripts/studio.sh check` | 1 (before the split) | eslint: `createLiveForce` was 43 lines; `sessionState` and `rowTable` now carry it |
| `scripts/studio-live.sh` | 0 | 4 rows PASS |
| `STUDIO_LIVE_BREAK=1 scripts/studio-live.sh` | 1 | negative control: 4 rows FAIL |
| `scripts/studio-smoke.sh` | 0 | 6 rows PASS |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 | negative control: non-zero |
