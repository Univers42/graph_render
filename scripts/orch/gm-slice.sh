#!/usr/bin/env bash
# gm-slice.sh install|check|remove — the one memory ceiling over every job container.
#
# Each `docker run` here carries its own --memory, but nothing bounded their sum: on 2026-10-03
# 11-18 job containers ran at once, none may swap (--memory-swap = --memory), so the kernel swapped
# the desktop out instead and the host froze at 20:32 (journal: memory pressure 20:27:57, 20:31:24,
# 20:32:23, then nothing). drun puts every job container under gm.slice; this unit caps the slice,
# so an excess is an OOM kill inside the slice (exit 137 in one gate) instead of a host freeze.
#
#   install   write /etc/systemd/system/gm.slice (sudo) and reload systemd
#   check     exit 0 when the unit is loaded with a MemoryMax, 1 otherwise
#   remove    delete the unit (sudo); drun then warns and runs without the ceiling
#
# GM_SLICE_PCT (default 60) is the ceiling as a share of MemTotal: 18.6 GiB on the 31 GiB host,
# which leaves ~12 GiB to the desktop, the browser and the services outside the slice.
# Ponytail: one fixed share for every host; a host whose own baseline is above 40 % of RAM still
# freezes, and a share too low turns large gates into OOM kills. The watcher (memwatch.sh) is the
# second layer for whatever runs outside the slice.
set -euo pipefail
unit=/etc/systemd/system/gm.slice

ceiling_bytes() {
  local kib
  kib=$(awk '/^MemTotal:/ {print $2}' /proc/meminfo)
  echo $((kib * 1024 * ${GM_SLICE_PCT:-60} / 100))
}

install_unit() {
  local max
  max=$(ceiling_bytes)
  printf '%s\n' '[Unit]' 'Description=graph_render job containers (scripts/orch/drun)' \
    'Before=slices.target' '' '[Slice]' 'MemoryAccounting=yes' "MemoryMax=$max" \
    'MemorySwapMax=0' | sudo -n tee "$unit" >/dev/null
  sudo -n systemctl daemon-reload
  echo "gm-slice: installed, MemoryMax=$((max >> 20)) MiB, no swap"
}

# GM_SLICE_BREAK=1 checks a slice that does not exist: the row's negative control.
check_unit() {
  local state max swap name=gm.slice
  [[ ${GM_SLICE_BREAK:-0} == 1 ]] && name=gm-absent.slice
  state=$(systemctl show -p LoadState --value "$name")
  max=$(systemctl show -p MemoryMax --value "$name")
  swap=$(systemctl show -p MemorySwapMax --value "$name")
  if [[ $state == loaded && $max =~ ^[0-9]+$ && $swap == 0 ]]; then
    echo "gm-slice: loaded, MemoryMax=$((max >> 20)) MiB, MemorySwapMax=0"
    return 0
  fi
  echo "gm-slice: not installed (state=$state MemoryMax=$max MemorySwapMax=$swap) — scripts/orch/gm-slice.sh install" >&2
  return 1
}

case ${1:-check} in
  install) install_unit && check_unit ;;
  check) check_unit ;;
  remove) sudo -n rm -f "$unit" && sudo -n systemctl daemon-reload && echo "gm-slice: removed" ;;
  *) echo "usage: gm-slice.sh install|check|remove" >&2; exit 2 ;;
esac
