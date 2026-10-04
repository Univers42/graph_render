#!/usr/bin/env bash
# studio-embed.sh — the embed gate: app/dist served on 127.0.0.1 and app/embed.html opened in
# headless Chromium (deploy/nav/embed.py). The page has no React of its own and drives
# <graph-studio> through the host API alone (docs/contract/host-api.md). Same gm-chromium image as
# studio-smoke.sh, no new dependency.
#
#   scripts/studio-embed.sh                          the gate
#   STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh     the negative control: it must exit 1
#
# Runs, each in its own browser: `plain` (no COOP/COEP), `isolated` (COOP/COEP) and `csp` (the CSP a
# host is asked for: script-src 'self' 'wasm-unsafe-eval', worker-src 'self').
#
# Rows: the load as the smoke gate judges it (no store error, no banner, nodes drawn, no exception,
# no console error, a screenshot per run in target/studio-embed/<label>/), the host's loadGraph and
# its graph-load, a resolve set before the element was defined, the five events heard by a
# `document` listener across two shadow roots, node-open by double click, by Enter (and not by Enter
# from the search box, with two selected, or once blurred) and by the inspector's Open button (also
# reached by Tab), two overlapping loads (one CancelledError, one graph-load), a refused load whose
# error name matches its graph-error, and no graph-studio.* key in localStorage across a reload.
#
# The negative control is one run per fault, each injected over CDP or in the bytes served, never a
# product switch; each run reports only the rows its fault targets, and every one must FAIL.
#
# Exit: gate 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
#       STUDIO_EMBED_BREAK=1: 1 every targeted row FAIL · 0 a control did not bite · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_EMBED_LABEL:-current}
break=()
if [[ ${STUDIO_EMBED_BREAK:-} == 1 ]]; then
  break=(--break)
  label=${STUDIO_EMBED_LABEL:-break}
fi

if [[ ! -f "$root/app/dist/embed.html" ]]; then
  echo "studio-embed: app/dist/embed.html is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2

mkdir -p "$root/target"
exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 embed.py --dist /w/app/dist --out "/w/target/studio-embed/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
