# shellcheck shell=bash
# Sourced by the scripts that build a `docker run` for a studio perf probe (scripts/studio-perf.sh,
# scripts/studio-probe.sh). This is the ONE place the GPU arm is wired in on the docker side: the
# device, the two host groups that own it, and the env the probes read (deploy/nav/gpu.py).
#
#   GM_GPU=1        the container is given /dev/dri and the host's render/video gids; the probes
#                   then drop --enable-unsafe-swiftshader for the hardware GL backend.
#   GM_GPU_BREAK=1  the negative control: GM_GPU=1 still asks for the check, but no device is
#                   passed, so the browser falls back to a software rasteriser and the probe must
#                   exit non-zero.
#
# Off unless the host asks: there is no GPU device in CI, and every probe runs unchanged without
# these. Only the perf wrappers source this file, so the parity gates (studio-backend.sh) keep the
# software rasteriser that is the same on every host.
#
# Sets: gpu_device (array of docker args, empty on the software arm), gpu_env (array of -e args).
# gpu_env always carries both knobs, empty or not: an unset GM_GPU inside the container is the
# software arm, and the probes read the variable rather than its absence from the command line.

# gpu_gid <group> <fallback> — the host's gid for <group>, so --group-add matches the owner of
# /dev/dri/renderD128 on this host rather than a number copied from another one.
# Ponytail: a host with no such group falls back to the recorded gids, which is right only on a
# host that numbers them the same; the probe's renderer line is what notices it was wrong.
gpu_gid() {
  local gid
  gid=$(getent group "$1" 2>/dev/null | cut -d: -f3)
  [[ -n $gid ]] && printf '%s\n' "$gid" || printf '%s\n' "$2"
}

gpu_env=(-e "GM_GPU=${GM_GPU:-}" -e "GM_GPU_BREAK=${GM_GPU_BREAK:-}")
gpu_device=()

if [[ ${GM_GPU:-} == 1 && ${GM_GPU_BREAK:-} != 1 ]]; then
  # --group-add is by gid: the container's own users have no business in render or video, and the
  # device node is only openable by them.
  gpu_device=(--device /dev/dri
              --group-add "$(gpu_gid render 993)"
              --group-add "$(gpu_gid video 44)")
fi