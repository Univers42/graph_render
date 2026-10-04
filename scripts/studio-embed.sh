#!/usr/bin/env bash
# studio-embed.sh — the embed gate: app/dist served on 127.0.0.1 and app/embed.html opened in
# headless Chromium (deploy/nav/embed.py). The page has no React of its own and drives
# <graph-studio> through the host API alone (docs/contract/host-api.md). Same gm-chromium image as
# studio-smoke.sh, no new dependency.
#
#   scripts/studio-embed.sh                          the gate
#   STUDIO_EMBED_BREAK=1 scripts/studio-embed.sh     the negative control: it must exit 1
#   STUDIO_EMBED_PACK=target/pack/graph-studio-0.1.0 scripts/studio-embed.sh
#                                                 the same gate over a built pack (the path is
#                                                 relative to the worktree root): the host page is
#                                                 rebuilt with its element import aliased to the
#                                                 pack, and the pack's files are served beside it
#                                                 (deploy/nav/embedpack.py), which adds one row
#                                                 per run: which wasm the worker fetched
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
# STUDIO_EMBED_PACK=<pack dir>: the same gate, served over a built pack instead of over app/dist.
pack=${STUDIO_EMBED_PACK:-}
label=${STUDIO_EMBED_LABEL:-current}
# Where the host page over the pack is built (pack runs only); read by in_node below.
host=target/pack-host/$label
break=()
if [[ ${STUDIO_EMBED_BREAK:-} == 1 ]]; then
  break=(--break)
  label=${STUDIO_EMBED_LABEL:-break}
fi

if [[ -n $pack && ! -f "$root/$pack/graph-studio.js" ]]; then
  echo "studio-embed: $pack/graph-studio.js is missing — run scripts/studio-pack.sh $pack first" >&2
  exit 2
fi

dist=/w/app/dist
out=$label
extra=()
if [[ -z $pack && ! -f "$root/app/dist/embed.html" ]]; then
  echo "studio-embed: app/dist/embed.html is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2
refs=${STUDIO_REFS:-${REFS:-$GM_SCRATCH/refs}}

# in_node <command...> — node in the image, over the same path scripts/studio.sh uses.
in_node() {
  "$root/scripts/orch/drun" --rm -e NPM_CONFIG_UPDATE_NOTIFIER=false -e PACK_MODE=host \
    -e PACK_DIR="/w/$pack" -e PACK_OUT_DIR="/w/$host" \
    -v "$root:/w" -w /w/app -v "$refs:/refs:ro" "$GM_NODE_IMAGE" "$@"
}

# The host page over the pack. Handed back to the user: the container is root, and a root-owned
# target/ is one the next run cannot rewrite.
build_host_page() {
  [[ -x $root/app/node_modules/.bin/vite ]] || in_node npm ci --ignore-scripts --no-audit --no-fund || return 2
  in_node sh -c "node_modules/.bin/vite build --config vite.pack.config.ts &&
    chown -R $(id -u):$(id -g) /w/$host" || return 2
}

mkdir -p "$root/target"
if [[ -n $pack ]]; then
  echo "[studio-embed] building the host page over $pack into $host"
  build_host_page || exit 2
  dist=/w/$host
  out=$label-pack
  extra=(--pack "/w/$pack")
fi

exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 embed.py --dist "$dist" --out "/w/target/studio-embed/$out" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${extra[@]}" "${break[@]}"
