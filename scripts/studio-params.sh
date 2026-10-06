#!/usr/bin/env bash
# studio-params.sh — the studio layout-parameter gate: the dock's Layout settings panel, built
# from the motor's own schema and driven in headless Chromium with real mouse input
# (deploy/nav/params.py). No new dependency: the browser is the gm-chromium image, the probes
# are the standard library's socket and json.
#
#   scripts/studio-params.sh              the gate
#   STUDIO_PARAMS_BREAK=1 scripts/studio-params.sh    the negative control: it must fail
#
# Rows: a force layout's three published parameters are three controls of the kinds the schema
# says; a slider dragged by hand changes the value, the run's digest and the pixels, and costs
# exactly one run however far the pointer walked; `layoutset <param> <value>` reaches the same
# place in one run; `layoutreset` gives back the published defaults, digest and all; a value
# outside the published range is refused by name with the drawing untouched; a layered
# layout's own parameter redraws its own layers; and a published bool is a switch and turns the
# layered drawing on its side.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster in a container, so the pixels are read as one cheap signature and
# the digest off the run report is what "the drawing changed" is measured on. The break walks
# the slider to where its thumb already is, so the value does not move and every row that
# claims a change changed is red.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${PARAMS_IMAGE:-gm-chromium}
label=${STUDIO_PARAMS_LABEL:-current}
break=()
[[ ${STUDIO_PARAMS_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-params: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 params.py --dist /w/app/dist --out "/w/target/studio-params/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
