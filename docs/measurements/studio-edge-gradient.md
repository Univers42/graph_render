# Studio edge gradient — flat against gradient, at 2000 nodes

Measured 2026-10-01 on this worktree, image `gm-chromium` (Chromium 154.0.8037.57), software
raster, on the same shared host as `studio-perf-baseline.md`. Graph: the driver's own
`random`, degree 3, seed 1, 2000 nodes and 5 991 edges — the graph the baseline was recorded on,
so the two runs are comparable.

```sh
scripts/studio.sh build
scripts/studio-perf.sh --label flat                       # exit 1, see below
scripts/studio-perf.sh --edge-colour gradient             # exit 1, see below
```

Both exits are 1 and both are the `perf-fps` row, in **both** modes: the flat mode is the one
the renderer shipped in, plus one string comparison per frame, and it measures 1.5 fps against
the baseline's 7 at 2000 nodes on DPR 2. That row is a shared-host reading on this machine
(`studio-perf.sh`: "A red perf-fps under load is re-run alone before it is believed"), not a
property of the mode. `perf-edge-batch`, the row this work is about, **passes in both modes**.

## Stroke counts and frame rates at 2000 nodes

Counters are the view's own, read on one settled frame at DPR 1 (`deploy/perf/probes/stats.js`);
the frame rates are the three phases of `probes/frame.js` at DPR 2, where the canvas is about
3000×2000 and the rasteriser, not the script, is the cost.

| | flat | gradient |
|---|---:|---:|
| edges drawn | 5 991 | 5 991 |
| mixed edges (two colours at the ends) | 0 | 4 764 |
| `stroke()` calls per frame | 3 | 15 |
| edge styles | 1 | 15 |
| per-edge gradient strokes | 0 | 0 |
| fps: zoom-in / zoom-out / zoom-back | 1.5 / 3.0 / 59.9 | 2.4 / 4.4 / 60.0 |
| JS p95 ms: the three phases | 4.2 / 1.1 / 0.7 | 4.0 / 1.5 / 1.0 |

Reading the gradient column: 4 764 of the 5 991 edges join two nodes of different colours, which
is over `MIXED_EDGE_BUDGET` (512, `canvas2d/edgeGradient.ts:19`), so this frame is the
**fallback** — every mixed edge is drawn in the linear mean of its two colours and batched per
colour pair, 15 strokes for the 15 palette entries of a 2000-node metric colouring. The fallback
is what `perf-edge-batch` is asked to hold down, and 15 ≤ 15 + floor(5 991 / 2 048).

The flat mode's 3 strokes are 1 style plus floor(5 991 / 2 048) chunks. The gradient mode's 15
are the 15 colour pairs, one stroke each, and no chunking because no pair holds more than
2 048 edges at this size.

A frame under the budget is a different drawing: `canvas2d/edgeGradient.ts` gives each mixed
edge its own `createLinearGradient`, one stroke each, capped at 512 of them. The nav gate's
`edge-gradient` row is measured on such a frame (a 400-node graph, 120 mixed edges, 120
gradient strokes) and reads three pixels along one of them within a byte of the linear ramp.

**Ponytail:** the frame rates compare run to run on one machine under software raster, and the
two modes differ by less than the run-to-run spread — 1.5 and 2.4 fps on a baseline of 7 are
both "this host". What is exact is the counter column: it is the renderer's own count, not a
timing. The gradient mode's cost is bounded by `MIXED_EDGE_BUDGET` and by the number of
distinct colour pairs, not by the edge count, and that is what `perf-edge-batch` reads.

## Not measured here

- A gradient frame at 2000 nodes: the budget is crossed long before that, so the per-edge
  gradient path is only exercised on graphs below 512 mixed edges.
- The curved and routed edges' gradients, which follow the chord between their ends rather than
  their path — see the `Caveat` in `canvas2d/edgeGradient.ts`.
- A GPU: every number above is software raster in a container.

## Control: develop without this change (2026-10-01)

`perf-fps` is red on this host before the change too. Same host, same hour, run alone, load average 6–8:

| Build | 120 nodes | 2000 nodes | `perf-edge-batch` |
|---|---|---|---|
| origin/develop dcf5d94, flat | 28.8 fps | 1.5 fps | — |
| this branch, flat | 28.3 fps | 1.5 fps | PASS (3 strokes, 1 style) |
| this branch, gradient | 46.1 fps | 2.4 fps | PASS (15 strokes, 15 pairs, 4764 mixed, 0 gradients) |

The red `perf-fps` is the host's software raster under load, not this change; it is not counted green.
Re-run alone on a quiet host: `scripts/studio-perf.sh --label flat` and `--edge-colour gradient`.
