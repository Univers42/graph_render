# Phase G0 report — studio perf harness, baseline, ADRs

Shape: `prompt.md` §12. Branch `studio`, worktree `/goinfre/dlesieur/wt/studio`, commits
`940ce64` and `601d788`. Every command below was run on 2026-09-29 on that tree; exit codes
are the real ones.

## 0. What this phase changed

A perf gate that runs the built studio in headless Chromium inside Docker, the numbers the
first studio reaches under it, six ADRs for the points where the approved plan conflicts
with the existing documents, and `.env` kept out of the Docker build context.

No file under a fingerprinted path was touched (`crates/graph-cli/src/fingerprint.rs:21-35`),
so no gate evidence on `develop` is voided by this branch.

## 1. Authorization compliance

| Envelope item (plan §6, G0) | Done | Where |
|---|---|---|
| `deploy/chromium.Dockerfile` | yes | image `gm-chromium`, `debian:trixie-slim` + `chromium` 154.0.8037.57 |
| `deploy/perf/**` | yes | `cdp.py`, `run.py`, `rows.py`, `probes/{idle,frame,block}.js`, `drivers/v0.js`, `baseline.json` |
| `scripts/studio-perf.sh` | yes | |
| `docs/measurements/studio-perf-baseline.md` | yes | |
| 6 ADRs | yes | `docs/decisions/{studio-is-a-product,server-and-write-path,snapshot-cache,images-from-debian,render-ports-not-imports,server-dependencies}.md` |
| `.dockerignore` | yes | `.env` appended |

Deviations:

| # | Deviation | Reason |
|---|---|---|
| 1 | Branch `studio` starts from `develop` `0d4f9a7`, not from `gui` | `gui` (`03ade38`) and `abi-post` (`77ab863`) were already merged into `develop` |
| 2 | Viewport is 1920×1080, the plan's measurements used 1440×900 | at 1440×900 the 120-node case sat at the 60 Hz cap at both DPRs, so the slowdown the user reported was invisible |
| 3 | `perf-fps` passes at `min(2 × baseline, 54 fps)`, the plan says `2 × baseline` | the frame clock stops at 60 Hz; twice the 54.1 fps baseline at 120 nodes cannot be measured |
| 4 | `deploy/lint.Dockerfile` (image `gm-lint`) is outside the G0 list | shellcheck, shfmt and pyflakes had no Docker home; the rule is Docker-only |
| 5 | `940ce64` was pushed before the `shfmt` fix; `601d788` carries it | commands were chained with `;` instead of `&&` |

## 2. Ledger diff

None. G0 adds no capability and changes no motor code.

## 3. Gate table

| Row | Command | Expect | Exit | Result |
|---|---|---|---:|---|
| `perf-baseline` | `scripts/studio-perf.sh --label baseline-v0 --driver v0 --record-baseline` | 1 (negative control: the first studio must fail the budgets) | 1 | as expected |
| `env-ignored` | `grep -qx '.env' .dockerignore` | 0 | 0 | PASS |
| `perf-shellcheck` | `gm-lint shellcheck scripts/studio-perf.sh` | 0 | 0 | PASS |
| `perf-shfmt` | `gm-lint shfmt -i 2 -ci -d scripts/studio-perf.sh` | 0 | 0 | PASS |
| `perf-pyflakes` | `gm-lint pyflakes3 deploy/perf/*.py` | 0 | 0 | PASS |

Rows inside the baseline run (`target/studio-perf/baseline-v0/table.md`):

| Row | Expectation | Measured | Verdict |
|---|---|---|---|
| `perf-idle` | 0 animation callbacks/s when parked | 59.5/s | FAIL |
| `perf-block` | main thread blocked ≤ 50 ms by any layout | 3467.9 ms (`layout.packing.circle` at 500 nodes) | FAIL |
| `perf-js` | p95 JS per frame ≤ 4 ms at 2000 nodes | 3.9 ms | PASS |
| `perf-fps` | ≥ 2× baseline at DPR 2 (or the 54 fps cap) | no baseline file on the recording run | NOT-RUN |
| `perf-20k` | recorded, not gated | the first studio caps at 2000 nodes | not run |

| Nodes | DPR | Canvas | Worst fps | JS p95 ms |
|---:|---:|---|---:|---:|
| 120 | 1 | 1552×925 | 59.5 | 2.0 |
| 120 | 2 | 3104×1850 | 54.1 | 1.7 |
| 2000 | 1 | 1552×925 | 13.3 | 3.9 |
| 2000 | 2 | 3104×1850 | 7.0 | 3.6 |

## 4. 4-way hash table

Not applicable: no motor output changed.

## 5. Coverage table

| Symbol | Exercised by |
|---|---|
| `deploy/perf/run.py`, `cdp.py`, `rows.py`, the three probes, `drivers/v0.js` | the `perf-baseline` run itself |
| `rows.py` `_fps` with a baseline present | none yet: first exercised by the G1–G3 run |

The harness has no unit tests. Its check is the negative control: a studio known to be slow
is reported slow.

## 6. Ponytail markers added

| File | What it gets wrong |
|---|---|
| `deploy/chromium.Dockerfile`, `deploy/perf/run.py`, `scripts/studio-perf.sh` | software raster on a shared host: fps is comparable run to run on one machine and is not a user's browser fps |
| `deploy/perf/cdp.py` | single-threaded, ignores CDP events, one page target |
| `deploy/perf/probes/frame.js` | fps is capped by the 60 Hz clock; the profiled pass inflates JS time and feeds no budget row |
| `deploy/perf/probes/block.js` | a 5 ms heartbeat over-reports under host load |

## 7. What could not be verified

| Item | Status |
|---|---|
| `perf-fps` against a baseline | UNKNOWN until the next studio build runs under the harness |
| `perf-20k` | UNKNOWN: the first studio refuses more than 2000 nodes |
| fps in the user's own browser | UNKNOWN: manual row, read from the HUD once G1–G3 ship it |
| host load during the baseline | about 5 (other sessions' cargo jobs); the numbers were not re-measured on an idle host |

## 8. Stop-and-ask items

| # | Item | Needed by |
|---|---|---|
| 1 | The token in `.env` was inside the Docker build context before this phase. Rotate it. | now |
| 2 | proptest / fast-check as new dependencies | G2 |
| 3 | Devil verdict for the server and cache ADRs | D1, D2 |
