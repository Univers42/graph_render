#!/usr/bin/env bash
# studio-s3gaps.sh — the display-panel gate: app/dist served on 127.0.0.1, every control set the
# way the dock sets it in headless Chromium (deploy/nav/display.py), read back from the view.
#
#   scripts/studio-s3gaps.sh                     the gate
#   STUDIO_S3GAPS_BREAK=1 scripts/studio-s3gaps.sh   the negative control: it must fail
#
# Rows: background (theme, flat, aurora; corner pixel; 0 rAF parked), node size bounds and the
# inverted-pair refusal, the animate default, console equals dock for the new actions.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container on a shared host; the numbers are the view's, and the
# two pixel rows (theme corner, glow) are exact only for this rasteriser.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_S3GAPS_LABEL:-current}
break=()
[[ ${STUDIO_S3GAPS_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-s3gaps: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 s3gaps.py --dist /w/app/dist --out "/w/target/studio-s3gaps/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
