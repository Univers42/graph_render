#!/usr/bin/env bash
# studio-filters.sh — the studio filtering gate: app/dist served on 127.0.0.1 and driven in
# headless Chromium through the studio's own registry (deploy/nav/filters.py). No new
# dependency: the browser is the gm-chromium image, the probes are the standard library's.
#
#   scripts/studio-filters.sh                    the gate
#   STUDIO_FILTERS_BREAK=1 scripts/studio-filters.sh    the negative control: it must fail
#
# Rows: a query with AND/OR/NOT/parens hides exactly the set the probe's own evaluator says;
# orphans hide exactly the degree-0 nodes; a kind toggle hides exactly that kind; a filter
# change costs no layout call unless relayout is on, and exactly one when it is; three groups
# colour a doubly-matching node from the first and reorder and recolour on demand, with a
# legend in that order; colour-by-key gives equal keys one colour, the metric colouring gives
# the look library's own inferno bytes; groups beat the metric; search highlights exactly its
# matches and fitresults puts every one of them on screen; a bad query refuses with its column
# and leaves the filter alone.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: the probe re-implements the query grammar and the colormap in Python so a row is
# an oracle and not a second copy of the product agreeing with itself — but a disagreement
# about the grammar's spelling (is `path:` a prefix?) is then a red row about the probe. Those
# readings are marked `# Ponytail:` in deploy/nav/filtersrows.py; read them before believing one.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${FILTERS_IMAGE:-gm-chromium}
label=${STUDIO_FILTERS_LABEL:-current}
break=()
[[ ${STUDIO_FILTERS_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-filters: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 filters.py --dist /w/app/dist --out "/w/target/studio-filters/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
