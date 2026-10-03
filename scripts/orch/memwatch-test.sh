#!/usr/bin/env bash
# memwatch-test.sh selftest|selftest-fixtures — the two checks behind memwatch.sh (see its header).
# Exit 0 = passed, 1 = ran and failed, 2 = could not run. MEMWATCH_BREAK=1 must turn both red.
set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
fails=0

# check <status> <name>: call right after the assertion, as `<assertion>; check $? "<name>"`.
check() {
  if (($1 == 0)); then echo "  ok    $2"; else echo "  FAIL  $2"; fails=$((fails + 1)); fi
}

# A fake /proc, /sys and cgroup tree under $1: 31 GB of RAM, three job cgroups, an amdgpu card at
# 98 % VRAM, and four DRM clients (Xorg protected; firefox with one client on two fds; chrome; i915).
make_fixture() {
  local r=$1 a pid comm id kib drv fd
  mkdir -p "$r/proc/pressure" "$r/sys/class/drm/card1/device" \
    "$r/cg/gm.slice/docker-aaa.scope" "$r/cg/gm.slice/docker-bbb.scope" \
    "$r/cg/user.slice/user-1000.slice/user@1000.service/app.slice/gm-oc-x.scope"
  echo 3221225472 >"$r/cg/gm.slice/docker-aaa.scope/memory.current"
  echo 1073741824 >"$r/cg/gm.slice/docker-bbb.scope/memory.current"
  echo 5368709120 >"$r/cg/user.slice/user-1000.slice/user@1000.service/app.slice/gm-oc-x.scope/memory.current"
  echo 8000000 >"$r/sys/class/drm/card1/device/mem_info_vram_total"
  echo 7840000 >"$r/sys/class/drm/card1/device/mem_info_vram_used"
  for a in "100 Xorg 1 2000000 amdgpu 5" "200 firefox 2 1500000 amdgpu 5" "200 firefox 2 1500000 amdgpu 6" \
    "200 firefox 3 100000 amdgpu 7" "300 chrome 4 300000 amdgpu 5" "400 i915app 9 9000000 i915 3"; do
    read -r pid comm id kib drv fd <<<"$a"
    mkdir -p "$r/proc/$pid/fdinfo"
    echo "$comm" >"$r/proc/$pid/comm"
    printf 'drm-driver:\t%s\ndrm-client-id:\t%s\ndrm-memory-vram:\t%s KiB\n' "$drv" "$id" "$kib" >"$r/proc/$pid/fdinfo/$fd"
  done
  echo "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-firefox.scope" >"$r/proc/200/cgroup"
  echo "0::/gm.slice/docker-bbb.scope" >"$r/proc/300/cgroup"
}

set_ram() { # <MemAvailable KiB> <psi full avg10>
  printf 'MemTotal: 32000000 kB\nMemFree: 1 kB\nMemAvailable: %s kB\n' "$1" >"$MEMWATCH_PROC/meminfo"
  printf 'some avg10=%s.00 avg60=0.00\nfull avg10=%s.00 avg60=0.00\n' "$2" "$2" >"$MEMWATCH_PROC/pressure/memory"
}

fixture_ram() {
  set_ram 20000000 0 && mw_ram_level
  [[ $MW_LEVEL == none ]]
  check $? "plenty of RAM is level none"
  set_ram 2000000 0 && mw_ram_level
  [[ $MW_LEVEL == high ]]
  check $? "6 % available is level high"
  set_ram 20000000 40 && mw_ram_level
  [[ $MW_LEVEL == severe ]]
  check $? "psi full 40 is level severe"
  mw_job_victim
  [[ ${MW_VICTIM##*/} == gm-oc-x.scope ]]
  check $? "the largest job is the OpenCode scope"
  MEMWATCH_ONLY_ID=bbb mw_job_victim
  [[ ${MW_VICTIM##*/} == docker-bbb.scope ]]
  check $? "MEMWATCH_ONLY_ID narrows to its container"
  MEMWATCH_VICTIM_MIN_MB=8000 mw_job_victim
  [[ -z $MW_VICTIM ]]
  check $? "no job above the floor is no victim"
  : >"$MEMWATCH_LOG" && set_ram 2000000 0 && mw_ram_tick
  grep -q "kill cgroup gm-oc-x.scope (5120 MiB)" "$MEMWATCH_LOG"
  check $? "a high tick kills the largest job"
  : >"$MEMWATCH_LOG" && set_ram 900000 0 && MEMWATCH_VICTIM_MIN_MB=8000 mw_ram_tick
  grep -q "kernel OOM killer" "$MEMWATCH_LOG"
  check $? "severe with no job calls the kernel OOM killer"
}

fixture_gpu() {
  mw_vram_pct
  [[ $MW_VRAM_PCT == 98 ]]
  check $? "VRAM reads 98 %"
  [[ $(mw_gpu_clients | head -1) == "2000000 100 Xorg" ]] && mw_gpu_clients | grep -qx "1600000 200 firefox"
  check $? "one client on two fds counts once"
  ! mw_gpu_clients | grep -q i915app
  check $? "a non-amdgpu client is ignored"
  mw_gpu_victim 1024
  [[ $MW_GPU_PID == 200 && $MW_GPU_KIB == 1600000 ]]
  check $? "the victim is firefox, Xorg is protected"
  mw_gpu_victim 2000
  [[ -z $MW_GPU_PID ]]
  check $? "no unprotected client above 2 GiB is no victim"
  [[ $(mw_job_cgroup_of 300) == "$MEMWATCH_CGROOT/gm.slice/docker-bbb.scope" ]]
  check $? "a client inside a job maps to its scope"
  : >"$MEMWATCH_LOG" && MEMWATCH_VRAM_HOLD=2 mw_vram_tick
  ! grep -q kill "$MEMWATCH_LOG"
  check $? "a first full-VRAM tick waits"
  MEMWATCH_VRAM_HOLD=2 mw_vram_tick
  grep -q "kill pid 200 (firefox): VRAM at 98 %" "$MEMWATCH_LOG"
  check $? "a held full VRAM kills firefox"
}

fixture_ring() {
  local fx='kernel: amdgpu 0000:03:00.0: amdgpu:  Process firefox pid 200 thread firefox:cs0 pid 201'
  : >"$MEMWATCH_LOG"
  mw_ring_line "1000.25 host $fx"
  ! grep -q kill "$MEMWATCH_LOG"
  check $? "a first ring timeout only logs"
  mw_ring_line "1010.50 host $fx"
  grep -q "kill pid 200 (firefox): second gpu ring timeout in 10 s" "$MEMWATCH_LOG"
  check $? "a second within 120 s kills the process"
  : >"$MEMWATCH_LOG" && mw_ring_line "1500.00 host $fx"
  ! grep -q kill "$MEMWATCH_LOG"
  check $? "a repeat after the window only logs"
  mw_ring_line "1600.00 host ${fx//firefox pid 200/Xorg pid 100}"
  grep -q "kill pid 200 (firefox): gpu ring timeout in Xorg" "$MEMWATCH_LOG" && ! grep -q "kill pid 100" "$MEMWATCH_LOG"
  check $? "an Xorg timeout at 98 % VRAM frees firefox, not Xorg"
  echo 4000000 >"$MEMWATCH_SYS/class/drm/card1/device/mem_info_vram_used"
  : >"$MEMWATCH_LOG" && mw_ring_line "1700.00 host ${fx//firefox pid 200/Xorg pid 100}"
  grep -q "VRAM 50 %: left alone" "$MEMWATCH_LOG" && ! grep -q kill "$MEMWATCH_LOG"
  check $? "an Xorg timeout at 50 % VRAM is left alone"
}

selftest_fixtures() {
  local r
  r=$(mktemp -d) || return 2
  make_fixture "$r"
  export MEMWATCH_PROC=$r/proc MEMWATCH_SYS=$r/sys MEMWATCH_CGROOT=$r/cg MEMWATCH_LOG=$r/log MEMWATCH_DRYRUN=1
  # shellcheck source=memwatch-lib.sh
  . "$here/memwatch-lib.sh"
  fixture_ram 2>/dev/null
  fixture_gpu 2>/dev/null
  fixture_ring 2>/dev/null
  rm -rf "$r"
  echo "memwatch selftest-fixtures: $fails failed"
  ((fails == 0))
}

# A node process in a 3 GiB drun container commits 64 MiB every 250 ms up to 2.5 GiB. A private
# root watcher, narrowed to that container, with its floor 1 GiB under today's MemAvailable, must
# kill it (exit 137, not an OOM kill: the cap is never reached) and log the kill within 30 s.
selftest_live() {
  local dir log id rc oom watcher
  "$here/gm-slice.sh" check >/dev/null || return 2
  # A directory, not a /tmp file: with fs.protected_regular root may not O_CREAT-open a file
  # another user owns in a sticky directory, and the watcher's log lines would vanish.
  dir=$(mktemp -d) || return 2
  log=$dir/log
  id=$("$here/drun" -d --name "gm-memwatch-selftest-$$" --memory 3g --memory-swap 3g --pull never \
    node:22.23.3-slim node -e 'const k=[];setInterval(()=>{if(k.length<40)k.push(Buffer.alloc(64<<20,1))},250)') || return 2
  . "$here/memwatch-lib.sh"
  mw_read_ram
  sudo -n env MEMWATCH_ONLY_ID="$id" MEMWATCH_AVAIL_MIN_KIB=$((MW_AVAIL - (1 << 20))) MEMWATCH_GPU=0 \
    MEMWATCH_INTERVAL=0.5 MEMWATCH_RUN_FOR=32 MEMWATCH_LOG="$log" MEMWATCH_BREAK="${MEMWATCH_BREAK:-0}" \
    "$here/memwatch.sh" run 2>/dev/null &
  watcher=$!
  rc=$(timeout 30 docker wait "$id")
  oom=$(docker inspect -f '{{.State.OOMKilled}}' "$id" 2>/dev/null)
  docker rm -f "$id" >/dev/null 2>&1
  wait "$watcher"
  sed 's/^/  log: /' "$log" 2>/dev/null
  [[ $rc == 137 ]]
  check $? "the allocator exited 137"
  [[ $oom == false ]]
  check $? "it was not the cgroup OOM killer"
  grep -q "kill cgroup docker-$id" "$log"
  check $? "the watcher logged the kill"
  rm -rf "$dir"
  echo "memwatch selftest: $fails failed"
  ((fails == 0))
}

case ${1:-} in
  selftest) selftest_live ;;
  selftest-fixtures) selftest_fixtures ;;
  *) echo "usage: memwatch-test.sh selftest|selftest-fixtures" >&2; exit 2 ;;
esac
