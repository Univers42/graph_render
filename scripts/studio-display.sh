#!/usr/bin/env bash
# studio-display.sh — the display-panel gate: app/dist served on 127.0.0.1, every control set the
# way the dock sets it in headless Chromium (deploy/nav/display.py), read back from the view.
#
#   scripts/studio-display.sh                     the gate
#   STUDIO_DISPLAY_BREAK=1 scripts/studio-display.sh   the negative control: it must fail
#
# Rows: arrows, text fade, node size (and by degree), link thickness, edge style, every theme's
# background and no motor call on a switch, glow, console equals dock, animate, dock target size.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container on a shared host; the numbers are the view's, and the
# two pixel rows (theme corner, glow) are exact only for this rasteriser.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_DISPLAY_LABEL:-current}
break=()
[[ ${STUDIO_DISPLAY_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-display: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 display.py --dist /w/app/dist --out "/w/target/studio-display/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
