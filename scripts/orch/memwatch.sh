#!/usr/bin/env bash
# memwatch.sh run|install|remove|status|selftest|selftest-fixtures — the host-wide RAM and GPU guard.
#
# gm-slice.sh caps the job containers; this watches the whole host, because the freezes came from
# both sides: RAM (2026-10-03 20:32, journal "under memory pressure" with 11-18 job containers up,
# then nothing until the reboot) and the GPU (2026-10-02 23:36:23, an Xorg gfx ring timeout 42 s
# before the reboot, llama-server holding 6.3 of the card's 8.6 GB VRAM). Every tick it checks
#   RAM       under pressure, kill the largest job cgroup (gm.slice scopes, gm-*.scope user units);
#             no job to blame and the host thrashing: the kernel OOM killer now (sysrq f), not
#             minutes of swap thrash first;
#   VRAM      full and held: kill the largest unprotected GPU client above 1 GiB;
#   GPU hang  the kernel log: a process that hangs the gfx ring twice in 120 s is killed.
# Killing a process frees its RAM and its VRAM together. Nothing resets the GPU itself: the kernel
# already resets the ring after each timeout, and a full reset takes the desktop down with it.
#
#   run                the loop; the service runs it as root (cgroup.kill, every client's fdinfo)
#   install            copy to /usr/local/lib/gm-memwatch, write gm-memwatch.service, enable, restart
#   remove             stop, disable and delete the service and the copy
#   status             exit 0 when the service is active and its copy matches this tree
#   selftest           a real allocator in a 3 GiB drun container must be killed by the watcher
#   selftest-fixtures  every decision against a fixture /proc, /sys and cgroup tree; nothing killed
# Tunables are the MEMWATCH_* variables in memwatch-lib.sh. MEMWATCH_BREAK=1 disables every kill,
# so both selftests must fail under it.
#
# Caveat: a bash loop cannot lock its pages in RAM. The unit runs it at OOMScoreAdjust=-1000 and
# top CPU and IO weight, and a tick forks nothing until there is pressure, but a host already deep
# in swap thrash can still page the watcher out and delay it by seconds.
set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
# shellcheck source=memwatch-lib.sh
. "$here/memwatch-lib.sh"
unit=/etc/systemd/system/gm-memwatch.service
dest=/usr/local/lib/gm-memwatch

# The kernel log follower, for gfx ring timeouts. Its pids go to MW_JOURNAL and MW_FOLLOWER.
mw_follow_kernel() {
  local kfd line
  exec {kfd}< <(exec journalctl -k -f -n0 -o short-unix 2>/dev/null)
  MW_JOURNAL=$!
  while read -r -u "$kfd" line; do mw_ring_line "$line"; done &
  MW_FOLLOWER=$!
  trap 'kill "$MW_JOURNAL" "$MW_FOLLOWER" 2>/dev/null' EXIT
}

mw_run() {
  local nap pause=${MEMWATCH_INTERVAL:-1}
  trap 'exit 0' TERM INT
  [[ ${MEMWATCH_GPU:-1} == 1 ]] && mw_follow_kernel
  exec {nap}<> <(:)
  mw_log "watching: tick ${pause}s, RAM floor ${MEMWATCH_AVAIL_MIN_KIB:-${MEMWATCH_AVAIL_MIN_PCT:-8} %}, gpu ${MEMWATCH_GPU:-1}${MEMWATCH_ONLY_ID:+, only ${MEMWATCH_ONLY_ID:0:12}}"
  while ((${MEMWATCH_RUN_FOR:-0} == 0 || SECONDS < ${MEMWATCH_RUN_FOR:-0})); do
    if mw_ram_tick || { [[ ${MEMWATCH_GPU:-1} == 1 ]] && mw_vram_tick; }; then
      read -r -t "${MEMWATCH_COOLDOWN:-3}" -u "$nap" _
    fi
    read -r -t "$pause" -u "$nap" _
  done
}

mw_install() {
  sudo -n install -d "$dest" &&
    sudo -n install -m 0755 "$here/memwatch.sh" "$here/memwatch-lib.sh" "$dest/" || return 1
  printf '%s\n' '[Unit]' 'Description=graph_render RAM and GPU watcher (scripts/orch/memwatch.sh)' \
    'After=docker.service' '' '[Service]' "ExecStart=$dest/memwatch.sh run" 'Restart=always' \
    'RestartSec=2' 'OOMScoreAdjust=-1000' 'Nice=-10' 'CPUWeight=1000' 'IOWeight=1000' '' \
    '[Install]' 'WantedBy=multi-user.target' | sudo -n tee "$unit" >/dev/null
  sudo -n systemctl daemon-reload && sudo -n systemctl enable -q gm-memwatch.service &&
    sudo -n systemctl restart gm-memwatch.service && mw_status
}

mw_remove() {
  sudo -n systemctl disable -q --now gm-memwatch.service 2>/dev/null
  sudo -n rm -rf "$unit" "$dest" && sudo -n systemctl daemon-reload && echo "memwatch: removed"
}

mw_status() {
  local f rc=0
  if ! systemctl is-active --quiet gm-memwatch.service; then
    echo "memwatch: gm-memwatch.service is not active (scripts/orch/memwatch.sh install)" >&2
    rc=1
  fi
  for f in memwatch.sh memwatch-lib.sh; do
    cmp -s "$here/$f" "$dest/$f" && continue
    echo "memwatch: the installed $f differs from $here/$f (re-run install)" >&2
    rc=1
  done
  journalctl -u gm-memwatch.service -n 5 --no-pager -o short-iso 2>/dev/null | cut -c1-200
  ((rc == 0)) && echo "memwatch: active, the installed copy matches this tree"
  return $rc
}

case ${1:-status} in
  run) mw_run ;;
  install) mw_install ;;
  remove) mw_remove ;;
  status) mw_status ;;
  selftest | selftest-fixtures) exec "$here/memwatch-test.sh" "$1" ;;
  *) echo "usage: memwatch.sh run|install|remove|status|selftest|selftest-fixtures" >&2; exit 2 ;;
esac
