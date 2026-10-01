#!/usr/bin/env bash
# studio-smoke.sh — the load-smoke gate: app/dist served on 127.0.0.1 and opened in headless
# Chromium (deploy/nav/smoke.py), watching the browser's error channels from load until the drawing
# settles. Same shape as studio-live.sh, same gm-chromium image, no new dependency.
#
#   scripts/studio-smoke.sh                  the gate
#   STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh   the negative control: it must fail
#
# Why: on 2026-10-01 the studio died on load with `exports.gm_dim is not a function` behind the
# "Unexpected studio error" banner, over a stale graph_wasm.wasm. Every other browser gate passed:
# each reads its own rows, and none of them looked at whether the page came up at all.
#
# Rows: no uncaught exception, no console or Log error, no studio.store error, no banner in the
# shadow root, and a node count over zero.
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${NAV_IMAGE:-gm-chromium}
label=${STUDIO_SMOKE_LABEL:-current}
break=()
[[ ${STUDIO_SMOKE_BREAK:-} == 1 ]] && break=(--break)

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-smoke: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

mkdir -p "$root/target"
exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 smoke.py --dist /w/app/dist --out "/w/target/studio-smoke/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break[@]}"
