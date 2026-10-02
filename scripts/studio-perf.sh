#!/usr/bin/env bash
# studio-perf.sh — the studio perf gate: app/dist driven in headless Chromium, in Docker.
#
#   scripts/studio-perf.sh [--label NAME] [--driver NAME] [--edge-colour flat|gradient] [--record-baseline]
#                          [--cases N,N,...] [--layout ID] [--backend auto|canvas2d|webgl2]
#
#   --label NAME        output directory under target/studio-perf/ (default: current)
#   --driver NAME       deploy/perf/drivers/NAME.js (default: hook)
#   --edge-colour MODE  appearance.edgecolour the drawing is measured in (default: flat)
#   --record-baseline   also write deploy/perf/baseline.json from this run
#   --cases N,N,...     a scale measurement at these node counts, DPR 1, instead of the gate
#                       (its gating rows read NOT-RUN, so it exits 1; read table.md)
#   --layout ID         with --cases: lay every case out with ID (layout.random keeps 1M nodes fast)
#   --backend NAME      open the page at ?backend=NAME; webgl2/auto draw on SwiftShader
#   PERF_MEMORY         the container's memory cap (default 4g; a 1M-node case needs about 8g)
#   GM_GPU=1            opt in to the host's GPU: --device /dev/dri and the hardware GL backend
#                       (scripts/orch/gpu.sh). The gate's numbers are then read off a real
#                       rasteriser, which no baseline on this host was; the SwiftShader arm stays
#                       the default and is what a gating row is compared against.
#   GM_GPU_BREAK=1      the negative control: GM_GPU=1 without the device, so the probe exits 2
#
# Exit: 0 every gating row PASS · 1 a gating row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Ponytail: software raster on a shared host — fps drops when other jobs run. A red
# perf-fps under load is re-run alone before it is believed; it is never counted green. Under
# GM_GPU=1 the raster is hardware and the same load still moves the numbers, so the arm is a
# measurement, not a gating row: docs/measurements/perf-p5c.md carries the tables.
set -uo pipefail

root=$(git -C "$(dirname -- "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)
image=${PERF_IMAGE:-gm-chromium}
label=current
driver=hook
edge_colour=flat
record=()
cases=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --label)
      label=$2
      shift 2
      ;;
    --driver)
      driver=$2
      shift 2
      ;;
    --edge-colour)
      edge_colour=$2
      label="$label-$2"
      shift 2
      ;;
    --cases)
      cases+=(--cases "$2")
      shift 2
      ;;
    --layout)
      cases+=(--layout "$2")
      shift 2
      ;;
    --backend)
      cases+=(--backend "$2")
      label="$label-$2"
      shift 2
      ;;
    --record-baseline)
      record=(--record-baseline deploy/perf/baseline.json)
      shift
      ;;
    --help)
      sed -n '2,21p' "${BASH_SOURCE[0]}"
      exit 0
      ;;
    *)
      echo "studio-perf: unknown argument $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-perf: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

baseline=()
[[ -f "$root/deploy/perf/baseline.json" ]] && baseline=(--baseline deploy/perf/baseline.json)

# The GPU arm's device and env, the same pair scripts/studio-probe.sh gives a probe.
source "$root/scripts/orch/gpu.sh"

mem=${PERF_MEMORY:-4g}
exec docker run --rm --memory "$mem" --memory-swap "$mem" -e STUDIO_PERF_BREAK="${STUDIO_PERF_BREAK:-}" \
  "${gpu_env[@]}" "${gpu_device[@]}" -v "$root:/w" -w /w "$image" \
  python3 deploy/perf/run.py --dist app/dist --out "target/studio-perf/$label" \
  --driver "$driver" --edge-colour "$edge_colour" --commit "$(git -C "$root" rev-parse --short HEAD)" \
  "${baseline[@]}" "${record[@]}" "${cases[@]}"
