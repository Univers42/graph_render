#!/usr/bin/env bash
# studio-gpu.sh — the studio GPU-forces gate: the Forces panel's GPU switch off on load, on
# ticking the settle on the host's GPU through the SDK's `gpuMesh()`, Accuracy unavailable with
# a reason while it is on, and off handing the settle back to the CPU. app/dist is served on
# 127.0.0.1 and driven in Chromium on the host's GPU (deploy/nav/gpuforces.py).
#
#   GM_GPU=1 scripts/studio-gpu.sh                    the gate
#   GM_GPU=1 GM_GPU_BREAK=1 scripts/studio-gpu.sh     the negative control: it must fail
#
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
# Build first: scripts/studio.sh build. Never takes the host gate lock; a rows file puts it under
# gpu.lock, as every GPU row is, so two device users never share the adapter.
#
# Caveat: one adapter (this host's) answers for every GPU; the switch on another browser's
# adapter is that browser's to prove. Under the break the device is gone, so the switch falls
# back to the CPU and says so, which is what the device row must refuse.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${LIVE_IMAGE:-gm-chromium}
label=${STUDIO_GPU_LABEL:-current}

if [[ ${GM_GPU:-} != 1 ]]; then
  echo "studio-gpu: GM_GPU=1 is required — the gate is about the host's GPU" >&2
  exit 2
fi
if [[ ! -f "$root/app/dist/index.html" ]]; then
  echo "studio-gpu: app/dist is missing — run scripts/studio.sh build" >&2
  exit 2
fi
source "$root/scripts/orch/image.sh"
ensure_image "$image" || exit 2
source "$root/scripts/orch/gpu.sh"

exec "$root/scripts/orch/drun" --rm --memory 4g --memory-swap 4g "${gpu_env[@]}" "${gpu_device[@]}" \
  -v "$root:/w" -w /w/deploy/nav "$image" \
  python3 gpuforces.py --dist /w/app/dist --out "/w/target/studio-gpu/$label" \
  --commit "$(git -C "$root" rev-parse --short HEAD)"
