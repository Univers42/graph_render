#!/usr/bin/env bash
# showcase.sh — record the studio's showcase video and encode it with the README's stills.
#
#   scripts/showcase.sh record [SCENE...]  # GPU screencast of every scene (or the named ones)
#   scripts/showcase.sh encode             # target/showcase -> docs/media (mp4, preview, stills)
#   scripts/showcase.sh                    # record, then encode
#
#   Scenes: deploy/perf/showcase_scenes.py. Recording: deploy/perf/showcase.py (gm-chromium, GPU).
#   Encoding: deploy/perf/showcase_encode.py (gm-media, ffmpeg).
#
# Exit: 0 written · 1 the video is over its size budget, or the recording saw a page error ·
# 2 could not run (no app/dist, no GPU, no image).
# Build first: scripts/studio.sh build. Never takes the host gate lock.
#
# Caveat: the recording runs on this host's GPU and its frame rate is what the GPU kept up with;
# a different host gives a different video from the same scenes. Not a gate, never in a rows file.
set -uo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root" || exit 2
mode=${1:-all}
[[ $# -gt 0 ]] && shift

record() {
  rm -rf "$root/target/showcase"
  GM_GPU=1 "$root/scripts/studio-probe.sh" showcase record "$@"
  local status=$?
  # The probe's container writes as root; hand target/showcase back so encode can read it.
  "$root/scripts/orch/gr" chown -R "$(id -u):$(id -g)" target/showcase >/dev/null || return 2
  return "$status"
}

encode() {
  source "$root/scripts/orch/image.sh"
  ensure_image gm-media || return 2
  "$root/scripts/orch/drun" --rm --user "$(id -u):$(id -g)" -e HOME=/tmp -v "$root:/w" -w /w \
    gm-media python3 deploy/perf/showcase_encode.py
}

case $mode in
  record) record "$@" ;;
  encode) encode ;;
  all) record && encode ;;
  *) sed -n '2,13p' "${BASH_SOURCE[0]}" >&2; exit 2 ;;
esac
