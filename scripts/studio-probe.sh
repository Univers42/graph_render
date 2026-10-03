#!/usr/bin/env bash
# studio-probe.sh — one perf probe (deploy/perf/NAME.py) in headless Chromium, in Docker.
#
#   scripts/studio-probe.sh NAME [ARGS...]
#   scripts/studio-probe.sh settle 1000000 webgl2 gpu-1m
#
#   NAME        open | settle | settle-pan | zoom | settle-profile (deploy/perf/NAME.py)
#   ARGS        the probe's own argv, passed through untouched
#   PERF_MEMORY the container's memory cap (default 10g; a 1M-node case needs about 8g)
#   LIVE_PROFILE, LIVE_THREADS  forwarded to the probe when set (deploy/perf/live-tick.py)
#
# Exit: the probe's own exit code. 2 means the harness could not run, and under GM_GPU=1 also means
# the browser drew on a software rasteriser where the GPU was asked for (deploy/nav/gpu.py).
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Why this exists: the probes' docker run line used to be a line in each probe's docstring, copied
# by hand, so no one place could add a device to it. This is that place. GM_GPU=1 gives the
# container /dev/dri and the host's render/video gids (scripts/orch/gpu.sh); GM_GPU_BREAK=1 asks
# for the check without the device, which is the negative control and must exit non-zero.
#
# Ponytail: no GPU in CI, and a host with no /dev/dri cannot run the GPU arm at all — the knob is
# opt-in per run so a missing device is the probe's own error, not a broken script.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${PERF_IMAGE:-gm-chromium}
probe=${1:-}
[[ -n $probe ]] || { sed -n '2,20p' "${BASH_SOURCE[0]}" >&2; exit 2; }
if [[ ! -f "$root/deploy/perf/$probe.py" ]]; then
  echo "studio-probe: no such probe: deploy/perf/$probe.py" >&2
  exit 2
fi

source "$root/scripts/orch/gpu.sh"

if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-probe: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -f "$root/deploy/chromium.Dockerfile" -t "$image" "$root/deploy" || exit 2
fi

mkdir -p "$root/target"
mem=${PERF_MEMORY:-10g}
exec "$root/scripts/orch/drun" --rm --memory "$mem" --memory-swap "$mem" "${gpu_env[@]}" "${gpu_device[@]}" \
  -e LIVE_PROFILE -e LIVE_THREADS -v "$root:/w" -w /w "$image" python3 "deploy/perf/$probe.py" "${@:2}"