#!/usr/bin/env bash
# studio-overlap.sh — the overlap probe: after a settle, how many drawn node discs cover each
# other on screen, at the force defaults and after `forces.spread`, on force/clustered.json and
# a 10 000-node synthetic graph; and the live loop's frame rate at 10 000 nodes. A built studio
# is served on 127.0.0.1 and driven in headless Chromium over CDP (deploy/nav/overlap.py).
#
#   scripts/studio-overlap.sh                          the probe, over app/dist
#   STUDIO_OVERLAP_DIST=target/dist-before scripts/studio-overlap.sh   another build (repo-relative)
#   STUDIO_OVERLAP_BREAK=1 scripts/studio-overlap.sh   the negative control: it must fail
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
# Output: target/studio-overlap/<STUDIO_OVERLAP_LABEL>/{report.json,table.md,*.png}.
#
# Ponytail: the frame rate is the page's own count of live frames in a software-raster container
# on a shared host; two runs of the same build differ by a few percent. Compare builds by
# interleaving runs, never one run against a number recorded on another day.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${OVERLAP_IMAGE:-gm-chromium}
dist=${STUDIO_OVERLAP_DIST:-app/dist}
label=${STUDIO_OVERLAP_LABEL:-current}
break=()
[[ ${STUDIO_OVERLAP_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/$dist/index.html" ]]; then
  echo "studio-overlap: $dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

exec docker run --rm --memory 6g --memory-swap 6g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 overlap.py --dist "/w/$dist" --out "/w/target/studio-overlap/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
