#!/usr/bin/env bash
# studio-local.sh — the local-graph and settings gate: app/dist served on 127.0.0.1 and driven in
# headless Chromium (deploy/local/), on the navigation gate's probe library. No new dependency.
#
#   scripts/studio-local.sh                       the gate
#   STUDIO_LOCAL_BREAK=1 scripts/studio-local.sh  the negative control: it must fail
#
# Rows: the local graph at depth 1, 2 and 5 and by direction equals an independent BFS over the
# fixture's edges; the view draws exactly the local set, and Escape draws every node again;
# two settings persist across a reload per source, a second source keeps its own, and a
# localStorage that throws leaves the defaults; export, reset, import is byte-identical and
# "reset this panel" touches one panel; "?" lists the bindings and each one runs its action.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container on a shared host; a red row is about the studio's
# state and the painted-node count, not about the load. The local set is the fixture's edges
# read by the probe, so a fixture the ingest reorders would show as a red row, not a pass.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_LOCAL_LABEL:-current}
break=()
[[ ${STUDIO_LOCAL_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-local: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/local "$image" \
  python3 local.py --dist /w/app/dist --out "/w/target/studio-local/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
