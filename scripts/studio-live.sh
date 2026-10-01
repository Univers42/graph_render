#!/usr/bin/env bash
# studio-live.sh — the studio live-force gate: a force layout settling on screen, a node
# dragged 150 px with the graph following it, and the progress bar over the top of the
# canvas. app/dist is served on 127.0.0.1 and driven in headless Chromium with real CDP
# input (deploy/nav/live.py). No new dependency: the browser is the gm-chromium image, the
# probes are the standard library's socket and json.
#
#   scripts/studio-live.sh              the gate
#   STUDIO_LIVE_BREAK=1 scripts/studio-live.sh    the negative control: it must fail
#
# Rows: with no input after load, the largest node travel over 0.4 s is over 1 world unit;
# after a 150 px drag, at least one neighbour of the dragged node travels over 5 world units;
# the progress bar is on screen during a settle and gone once the graph has settled; and
# `live-dead-worker` — the bar is hidden, within the watchdog's own bound, after the motor
# worker is stopped mid-settle, with one console line naming the cause.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: the break takes the motor away from the page — `studio.destroy()` closes the
# worker port, so no force request is sent again — and removes the view's paint hook. All four
# rows go red; a control that left one row green would prove nothing about it. The watchdog row
# runs last for a second reason: it stops the worker on purpose, and the rows above need one.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${LIVE_IMAGE:-gm-chromium}
label=${STUDIO_LIVE_LABEL:-current}
break=()
[[ ${STUDIO_LIVE_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-live: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 live.py --dist /w/app/dist --out "/w/target/studio-live/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"