# Memory guard: host ceiling, a watcher, a motor budget

Status: accepted, 2026-10-03, under full autonomy, after two host freezes that needed a hard reset.

## Context

Two freezes, two causes, read from `journalctl -b -1` on host dlesieur42:

- **RAM, 2026-10-03 20:32.** The journal logs "under memory pressure" with 11 to 18 job containers
  up, then nothing until the reboot. Every container ran without `--memory`, and
  `vm.overcommit_memory=1` (set by microk8s) grants any allocation, so the host thrashed 25 GB of
  swap for minutes before the OOM killer acted. One allocator found since: a peer branch's index
  test asked for 8.36 GB (`index/columns.rs:107` on perf-p4a-extend, fixed on
  perf-open-synth-columns at 349445be).
- **GPU, 2026-10-02 23:36:23.** An Xorg gfx ring timeout 42 s before the reboot, with llama-server
  holding 6.3 of the RX 6600's 8.6 GB VRAM. Not this repo's process, but the desktop went with it.

The motor itself does not leak (`docs/measurements/memory-profile.md`: 0 bytes lost, 1.2 KB per
node on the force path). Three layouts build n² tables: Kamada-Kawai's hop matrix (8 B a cell),
neato's three packed triangles (12 B a triangle cell) and the circle-packing fallback's adjacency
(8 B a cell). At 100 000 nodes that is 80 GB asked for at once, and on wasm32 `n * n` in `usize`
wraps past 65 536 nodes into a wrong, small table.

## Decision

1. **A ceiling for every job.** `scripts/orch/drun` is the one way the repo starts a container:
   under `gm.slice` (MemoryMax 60 % of RAM, no swap; `scripts/orch/gm-slice.sh`), with
   `--oom-score-adj 500` and a 4 GiB default cap. `scripts/orch/drun-check.sh` fails on a bare
   `docker run` in any shell script or rows file.
2. **A watcher.** `gm-memwatch.service` (`scripts/orch/memwatch.sh`) watches RAM (PSI full and
   MemAvailable), VRAM (DRM fdinfo per client) and the kernel log for gfx ring timeouts. Under RAM
   pressure it kills the largest job cgroup, and with no job to blame it calls the kernel OOM
   killer at once instead of after minutes of swap thrash. Full VRAM held for two ticks kills the
   largest unprotected GPU client above 1 GiB; Xorg is never the victim. It never resets the GPU:
   the kernel already resets the ring, and a full reset takes the desktop with it.
3. **A motor budget.** `graph_core::budget::QUADRATIC_BYTES_MAX` is 1 GiB for a layout's quadratic
   tables, counted in `u64` with checked arithmetic. Past it the layout returns
   `StageError::Param { name: "nodes" }` before allocating: Kamada-Kawai and the circle-packing
   fallback above 11 585 nodes, neato above 13 376. Each limit is above its registry
   `scale_ceiling` and pinned by a test.
4. **Studio limits and releases.** The studio refuses a generated graph past 2 000 000 links and a
   document past 2^28 characters before building anything (`source/limits.ts`; measured in
   `docs/measurements/memory-profile.md`, section "Studio"). A wasm heap only grows, so a new
   source loads in a new worker and a worker that trapped or threw `RangeError` is retired
   (`motor/client.ts`). A destroyed view gives its WebGL contexts back with `WEBGL_lose_context`
   instead of waiting for a collection (`webgl2/hook.ts` `releaseBulk`). A page that ended
   uncleanly does not reopen its last source by itself (`state/persist.ts`), and a document past
   1 Mi characters is not stored.

## Consequences

- A runaway job dies alone, and the desktop keeps its memory. The live selftest
  (`memwatch.sh selftest`) shows a 2.5 GiB allocator in a 3 GiB container killed by the watcher
  (exit 137, not the cgroup OOM killer) within 30 s; `MEMWATCH_BREAK=1` turns it red.
- A graph past a quadratic layout's budget gets a refusal naming the budget, not a frozen tab.
  The force engines, which are linear, are unaffected.
- A graph that took the page down is not replayed at the next start; picked again, it opens.
- One budget on every host: a 64 GB host refuses at the same n as an 8 GB one. That is the price of
  bit-identical results; the refusal is part of the output.
- Containers started outside the repo (llama-server, MCP servers) are outside `gm.slice`. The
  watcher still sees them through PSI and VRAM; capping them is their owner's call. On 2026-10-03
  the owner had llama-server capped (`docker update --memory 8g --memory-swap 8g`) and stopped: VRAM
  in use fell from 7508 to 1806 MiB. Its compose file no longer exists, so the container keeps the cap.
- `vm.overcommit_memory` stays at microk8s's 1. Mode 0 refuses only one mapping larger than RAM plus
  swap: on this host it granted a 50 GiB anonymous map and refused 60 GiB (`mmap` probe, 2026-10-03).
  It would not have stopped a freeze made of many smaller allocations, and kubelet sets 1 again at
  each start.

Risk scores: blast 3 (every container launch, three layouts), reversibility 1
(`memwatch.sh remove`, `gm-slice.sh remove`, revert one module), cost on failure 2, confidence 2
(both selftests and the pinned limits run green).
