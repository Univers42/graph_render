#!/usr/bin/env bash
# studio-3d.sh — the studio 3D gate: app/dist served on 127.0.0.1 and driven in headless
# Chromium with real CDP input (deploy/three/). No new dependency: the browser is the
# gm-chromium image, the probes are the standard library's socket and json, and the CDP
# client is the perf gate's (deploy/perf/cdp.py), the same one every other gate drives with.
#
#   scripts/studio-3d.sh              the gate
#   STUDIO_3D_BREAK=1 scripts/studio-3d.sh    the negative control: it must fail
#
# Rows: `layout.basic3d.sphere` is chosen from the console and draws a 3D frame whose nodes
# are not coplanar; a 200 px drag turns the camera and moves the projected nodes; the same
# drag back turns it the other way; a vertical drag tips it; a wheel notch pulls the camera
# in; a right-drag slides the drawing without turning anything; the console's `headon` puts
# a turned camera back; and the HUD carries a 3D badge while one is drawn.
#
# The negative control is STUDIO_3D_BREAK: it pins the camera to the orbit it holds, so every
# drag is accepted by the view and thrown away. A gate that cannot see a frozen camera would
# pass that, which is the only thing the negctl is for.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container on a shared host, and the numbers are the
# projection's own floats, not the rasteriser's — so a red row here is about the camera, not
# about the load. No row claims how the drawing looks; that is what a screenshot is for.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${THREE_IMAGE:-gm-chromium}
label=${STUDIO_3D_LABEL:-current}
break=()
[[ ${STUDIO_3D_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-3d: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/three "$image" \
  python3 three.py --dist /w/app/dist --out "/w/target/studio-3d/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"