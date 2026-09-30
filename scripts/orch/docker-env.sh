# shellcheck shell=bash
# Sourced by the orch wrappers. The rootless daemon was restarted on 2026-09-29 23:57 with its
# runtime dir in /tmp/xdg-$UID, while /run/user/$UID became root 0700; shells started earlier kept
# DOCKER_HOST on the old socket and every gate row failed with exit 126. Keep DOCKER_HOST when its
# socket is usable, else take the first usable candidate.
docker_sock=${DOCKER_HOST-}
docker_sock=${docker_sock#unix://}
if [[ ! -S $docker_sock || ! -w $docker_sock ]]; then
  for docker_sock in "${XDG_RUNTIME_DIR:-/run/user/$UID}/docker.sock" "/tmp/xdg-$UID/docker.sock"; do
    [[ -S $docker_sock && -w $docker_sock ]] && { export DOCKER_HOST=unix://$docker_sock; break; }
  done
fi
unset docker_sock
