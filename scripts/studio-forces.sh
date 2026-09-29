#!/usr/bin/env bash
# studio-forces.sh — the studio navigation gate: app/dist served on 127.0.0.1 and driven in
# headless Chromium with real CDP input (deploy/nav/). No new dependency: the browser is the
# gm-chromium image, the probes are the standard library's socket and json.
#
#   scripts/studio-forces.sh              the gate
#   STUDIO_FORCES_BREAK=1 scripts/studio-forces.sh    the negative control: it must fail
#
# Rows: the Forces section exists; its four sliders are labelled Center force, Repel force, Link
# force, Link distance and are reached by four Tab presses; the reason "live forces need the
# motor session (force-wasm)" is shown while no adapter exists; a view-only drag moves a node.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: the break control removes the panel from the page, not from the build; see forces.py.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_FORCES_LABEL:-current}
break=()
[[ ${STUDIO_FORCES_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-forces: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 forces.py --dist /w/app/dist --out "/w/target/studio-forces/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
