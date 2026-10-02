#!/usr/bin/env bash
# studio-nav.sh — the studio navigation gate: app/dist served on 127.0.0.1 and driven in
# headless Chromium with real CDP input (deploy/nav/). No new dependency: the browser is the
# gm-chromium image, the probes are the standard library's socket and json.
#
#   scripts/studio-nav.sh              the gate
#   STUDIO_NAV_BREAK=1 scripts/studio-nav.sh    the negative control: it must fail
#
# Rows: a 200 px drag moves the camera offset 200 px ±0.5; a wheel notch and a ctrlKey pinch
# at (x,y) leave the world point under the cursor within 0.5 px; a double-click on the
# background zooms ×2 at the cursor; F, 0, +, -, the arrows and Escape each do what they
# name and the scale stays in [0.02, 40]; space+drag and middle-drag pan; the edge gradient
# mode is switched from the dock and read back off one known mixed edge; and switching the
# Layout to force.drl leaves every node centre inside the canvas on a camera fitted to that
# drawing, not the camera from before the switch.
#
# STUDIO_NAV_BREAK=1 inverts three of those claims: the drag expects a move that is not made,
# the gradient mode is never turned on, and the layout switch expects the camera to sit still.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container on a shared host, and the numbers are the camera's
# (a float), not the rasteriser's — so a red row here is about the camera, not about the load.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_NAV_LABEL:-current}
break=()
[[ ${STUDIO_NAV_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-nav: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 nav.py --dist /w/app/dist --out "/w/target/studio-nav/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
