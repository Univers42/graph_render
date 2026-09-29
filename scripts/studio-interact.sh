#!/usr/bin/env bash
# studio-interact.sh — the studio interaction gate: app/dist served on 127.0.0.1 and driven in
# headless Chromium with real CDP input (deploy/nav/interact.py). No new dependency: the browser is the
# gm-chromium image, the probes are the standard library's socket and json.
#
#   scripts/studio-interact.sh              the gate
#   STUDIO_INTERACT_BREAK=1 scripts/studio-interact.sh    the negative control: it must fail
#
# Rows: hover fades the rest to 0.12 and labels the neighbourhood; click and shift-click select;
# shift+drag selects the nodes in the box; a 150 px node drag lands under the pointer; the node
# menu offers focus, pin, hide, copy id; copy id fills the clipboard state; hide removes the node.
# The break control expects a fade of 0.13, which the app does not make.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: opacity is read after a fixed 0.6 s wait; a starved host may read an unfinished fade.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_INTERACT_LABEL:-current}
break=()
[[ ${STUDIO_INTERACT_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-interact: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 interact.py --dist /w/app/dist --out "/w/target/studio-interact/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
