# studio-s7 perf table

Baseline: `deploy/perf/baseline.json` (see `studio-perf-baseline.md`). Nothing below was measured
in this step: the Docker socket was not reachable, so neither `scripts/studio.sh check` nor
`scripts/studio-perf.sh` ran. Every verdict is NOT RUN until `scripts/studio-perf.sh --label s7` writes numbers here.

| row | expectation | measured | verdict |
|---|---|---|---|
| perf-edge-batch | stroke() calls per frame <= edge styles; arrow fills <= 1 (2000 and 10000 nodes) | not measured | not run |
| perf-sprite-cache | 0 sprites rasterised on a second identical frame | not measured | not run |
| perf-label-layout | 0 layout runs over a redraw and a parked frame, >= 1 after a zoom | not measured | not run |
| perf-10k | 10000 nodes / 20000 edges pan/zoom p95, recorded not gated | not measured | not run |
| negative control | `STUDIO_PERF_BREAK=1 scripts/studio-perf.sh` exits non-zero | not measured | not run |

Ponytail: the counter rows are exact counts; the 10k timing is a shared, software-rastered host and is never gated.
