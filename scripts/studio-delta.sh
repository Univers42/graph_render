#!/usr/bin/env bash
# studio-delta.sh — the studio delta gate: a live graph that grows while the force layout keeps
# running. app/dist is served on 127.0.0.1 and driven in headless Chromium
# (deploy/nav/delta.py), the shape studio-live.sh has, with the motor worker's error channels
# watched the way studio-smoke.sh watches them. No new dependency: the browser is the
# gm-chromium image, the probe is the standard library plus the shared CDP client.
#
#   scripts/studio-delta.sh                    the gate
#   STUDIO_DELTA_BREAK=1 scripts/studio-delta.sh   the negative control: it must fail
#
# Rows: `deltas-drawn` — the drawn node count reaches base + 1000 within 2 s of the last of ten
# `el.applyDeltas(batch)` calls of 100 nodes each; `deltas-moving` — over 0.4 s after the burst
# the largest single-node travel is more than 1 world unit, in the graph the batches grew;
# `deltas-clean` — no page or motor-worker exception, no console or Log error, no failure banner
# in the shadow root; `deltas-refused` — one batch naming an id the graph already holds rejects,
# and the drawn count does not grow across that one call.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Why: the deltas are only worth having if the layout grows without a restart — a host that
# streams a graph wants the nodes drawn and still moving, and a batch that half-applies would
# draw a graph the motor never had.
#
# Ponytail: the control is one fault and it is in the page, not in the probe: `?break-deltas=1`
# reaches the worker, which drops the `grow` after an extend, so the drawing never grows past its
# base and `deltas-drawn` and `deltas-moving` go red on the thresholds they pass otherwise. This
# script adds no knob of its own and widens nothing for the control.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_DELTA_LABEL:-current}
break=()
[[ ${STUDIO_DELTA_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-delta: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

mkdir -p "$root/target"
exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 delta.py --dist /w/app/dist --out "/w/target/studio-delta/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
