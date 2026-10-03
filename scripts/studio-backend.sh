#!/usr/bin/env bash
# studio-backend.sh — the backend gate: the WebGL2 layer against Canvas2D, over app/dist in
# headless Chromium (deploy/nav/backend.py). Same shape and image as studio-smoke.sh, with
# WebGL2 on SwiftShader, the only WebGL2 a GPU-less container has.
#
#   scripts/studio-backend.sh                  the gate
#   STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh   the negative control: it must fail
#
# Why: P5 gave the renderer a second painter (packages/graph-render/src/webgl2/) and nothing in
# the tree compares it with the 2D painter it replaces. A layer that draws the wrong thing, or
# draws nothing, passes every other browser gate, because each of them reads its own rows and
# none of them looks at which painter drew.
#
# Rows: the two painters' screenshots differ on at most the recorded ceiling of their pixels and
# both are non-blank; `auto` reaches for the layer on a 20000-node graph; a browser with no
# WebGL2 falls back to Canvas2D, says why, draws, and throws nothing; a context lost in the
# session does the same and names the loss.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_BACKEND_LABEL:-current}
break=()
[[ ${STUDIO_BACKEND_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-backend: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

mkdir -p "$root/target"
exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 backend.py --dist /w/app/dist --out "/w/target/studio-backend/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"