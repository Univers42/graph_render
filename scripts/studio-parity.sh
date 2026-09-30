#!/usr/bin/env bash
# studio-parity.sh — the SciGraphs parity gate: the built studio's parity page, drawn in
# headless Chromium at 1920x1080, and the screenshot judged against the pinned sources.
#
#   scripts/studio-parity.sh [--break]
#
# Build first: scripts/studio.sh build. The gate never takes the host gate lock and never
# runs the hash gate.
#
# Exit: 0 every gating row PASS · 1 a gating row FAIL or NOT-RUN · 2 could not run.
#
#   --break   the negative control: the background is expected one byte off in blue, so
#             the run must fail. STUDIO_PARITY_BREAK=1 is the same thing.
#
# Writes: target/studio-parity/parity.png, report.txt, report.json
#
# Ponytail: the reference the harness computes the colormap from is mounted read-only at
# /refs; without it the gate reports NOT-RUN rather than a colour of its own.
set -uo pipefail

root=$(git -C "$(dirname -- "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)
image=${PARITY_IMAGE:-gm-chromium}
source "$(dirname "$(readlink -f "$0")")/orch/scratch.sh"
refs=${STUDIO_REFS:-$GM_SCRATCH/refs}
label=${PARITY_LABEL:-current}
break_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --break)
      break_args=(--break)
      shift
      ;;
    --label)
      label=$2
      shift 2
      ;;
    --help)
      sed -n '2,20p' "${BASH_SOURCE[0]}"
      exit 0
      ;;
    *)
      echo "studio-parity: unknown argument $1" >&2
      exit 2
      ;;
  esac
done
[[ ${STUDIO_PARITY_BREAK:-0} == 1 ]] && break_args=(--break)

if [[ ! -f "$root/app/dist/parity.html" ]]; then
  echo "studio-parity: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if [[ ! -f "$refs/matplotlib-3.10.0/_cm_listed.py" ]]; then
  echo "studio-parity: $refs is not the pinned reference tree (scripts/orch/fetch-refs.sh)" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

exec docker run --rm --memory 4g --memory-swap 4g -v "$root:/w" -v "$refs:/refs:ro" -w /w "$image" \
  python3 deploy/parity/run.py --dist app/dist --out "target/studio-parity/$label" \
  --root /w --reference /refs/matplotlib-3.10.0/_cm_listed.py \
  --commit "$(git -C "$root" rev-parse --short HEAD)" "${break_args[@]}"
