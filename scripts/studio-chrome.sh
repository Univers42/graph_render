#!/usr/bin/env bash
# studio-chrome.sh — the studio chrome gate: app/dist served on 127.0.0.1 and driven in
# headless Chromium (deploy/chrome/). No new dependency: the browser is the gm-chromium
# image, the probes are the standard library's socket, json, zlib and struct.
#
#   scripts/studio-chrome.sh                    the gate
#   STUDIO_CHROME_BREAK=1 scripts/studio-chrome.sh    the negative control: it must fail
#
# Rows: at 320 to 2560 CSS px and a device pixel ratio of 1 to 2, in both themes, with the
# dock and the console open and closed, over a width sweep up and back down, and with
# overlay scrollbars turned off: the page does not scroll sideways, the host's right and
# bottom edges are the viewport's, the outermost 8 px of the composite are the canvas and
# not the page showing through beside it, and the canvas' backing store is the CSS box
# times the device pixel ratio. A ratio that changes with the CSS size held still — a
# window moved to a screen of another density, a browser zoom — re-measures the backing
# store.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container on a shared host, so the pixel rows are about
# the page's own layout and not about how it was drawn. Linux ships overlay scrollbars, so
# most cells are measured on a browser that would not have shown a `100vw` bug; the report
# says so rather than implying the widths covered everything.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${CHROME_IMAGE:-gm-chromium}
label=${STUDIO_CHROME_LABEL:-current}
break=()
[[ ${STUDIO_CHROME_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-chrome: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/chrome "$image" \
  python3 chrome.py --dist /w/app/dist --out "/w/target/studio-chrome/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
