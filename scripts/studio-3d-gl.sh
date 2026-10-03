#!/usr/bin/env bash
# studio-3d-gl.sh — the 3D GL gate: the WebGL2 3D layer against the Canvas2D 3D painter, over
# app/dist in headless Chromium (deploy/three/gl.py). Same shape and image as
# studio-backend.sh, with WebGL2 on SwiftShader, the only WebGL2 a GPU-less container has.
#
#   scripts/studio-3d-gl.sh                     the gate
#   STUDIO_3D_GL_BREAK=1 scripts/studio-3d-gl.sh    the negative control: it must fail
#
# Why: a 3D frame has two painters, packages/graph-render/src/webgl2/*3d.ts and
# three/paint3d.ts, and nothing else in the tree compares them; studio-backend.sh compares the
# 2D ones and studio-3d.sh reads the camera, not the pixels.
#
# Rows: each page names the painter it was asked for; the sphere turned by one drag differs
# between the two on at most the recorded ceiling of its pixels and neither is blank; a drag on
# the GL page sends no buffer and no texture, only the camera uniform; that page throws and
# logs nothing; a context lost mid session falls back to Canvas2D, names the loss and keeps
# drawing; a browser with no WebGL2 does the same and says why; `auto` on this software
# rasteriser keeps a 3D frame past the layer's threshold on Canvas2D, without a failure.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${THREE_IMAGE:-gm-chromium}
label=${STUDIO_3D_GL_LABEL:-current}
break=()
[[ ${STUDIO_3D_GL_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-3d-gl: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

commit=$(git -C "$root" rev-parse --short HEAD 2>/dev/null || echo unknown)
mkdir -p "$root/target"
exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/three "$image" \
  python3 gl.py --dist /w/app/dist --out "/w/target/studio-3d-gl/$label" \
  --commit "$commit" "${break[@]}"
