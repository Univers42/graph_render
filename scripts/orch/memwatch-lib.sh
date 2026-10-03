#!/usr/bin/env bash
# memwatch-lib.sh — the readers, decisions and kills behind memwatch.sh. Sourced; loading it has
# no side effect. Every reader takes its root from MEMWATCH_PROC / MEMWATCH_SYS / MEMWATCH_CGROOT,
# so `memwatch.sh selftest-fixtures` drives each decision from a fixture tree, and
# MEMWATCH_DRYRUN=1 turns every kill into a log line.
#
# The RAM path reads with bash builtins only: when the host is short of memory a fork is slow, and
# a watcher that forks on every tick is the first thing to stall.

MW_PROC=${MEMWATCH_PROC:-/proc}
MW_SYS=${MEMWATCH_SYS:-/sys}
MW_CG=${MEMWATCH_CGROOT:-/sys/fs/cgroup}
MW_PROTECT=" ${MEMWATCH_GPU_PROTECT:-Xorg Xwayland kwin_x11 kwin_wayland plasmashell gnome-shell llama-server} "

mw_log() {
  printf '%(%F %T)T memwatch: %s\n' -1 "$*" >&2
  [[ -z ${MEMWATCH_LOG:-} ]] || printf '%(%F %T)T memwatch: %s\n' -1 "$*" >>"$MEMWATCH_LOG"
}

# MW_TOTAL and MW_AVAIL (KiB) from meminfo, MW_PSI from the integer part of `full avg10`.
mw_read_ram() {
  local key val rest
  MW_TOTAL=0 MW_AVAIL=0 MW_PSI=0
  while read -r key val _; do
    case $key in MemTotal:) MW_TOTAL=$val ;; MemAvailable:) MW_AVAIL=$val && break ;; esac
  done <"$MW_PROC/meminfo"
  while read -r key rest; do
    [[ $key == full ]] || continue
    rest=${rest#avg10=}
    MW_PSI=${rest%%.*}
  done <"$MW_PROC/pressure/memory"
}

# MW_LEVEL = none | high | severe. High: MemAvailable under MEMWATCH_AVAIL_MIN_PCT (8) of RAM, or
# MEMWATCH_AVAIL_MIN_KIB when set, or memory PSI full avg10 at 15 or more; severe: under 4 % or 30.
# Caveat: MemAvailable is the kernel's own estimate and ignores swap, so a host that is still
# swapping smoothly reads as high; PSI is what separates swapping from thrashing.
mw_ram_level() {
  mw_read_ram
  local min=${MEMWATCH_AVAIL_MIN_KIB:-$((MW_TOTAL * ${MEMWATCH_AVAIL_MIN_PCT:-8} / 100))}
  local severe=$((MW_TOTAL * ${MEMWATCH_SEVERE_PCT:-4} / 100))
  if ((MW_AVAIL < severe || MW_PSI >= ${MEMWATCH_PSI_SEVERE:-30})); then
    MW_LEVEL=severe
  elif ((MW_AVAIL < min || MW_PSI >= ${MEMWATCH_PSI_HIGH:-15})); then
    MW_LEVEL=high
  else
    MW_LEVEL=none
  fi
}

# MW_VICTIM = the job cgroup using the most memory, MW_VICTIM_BYTES its use; empty under
# MEMWATCH_VICTIM_MIN_MB (256). Jobs are the gm.slice scopes (drun) and the user's gm-*.scope units
# (OpenCode jobs). MEMWATCH_ONLY_ID narrows the choice to cgroups whose name contains it.
mw_job_victim() {
  local d cur
  MW_VICTIM='' MW_VICTIM_BYTES=0
  for d in "$MW_CG"/gm.slice/*.scope "$MW_CG"/user.slice/user-*.slice/user@*.service/app.slice/gm-*.scope; do
    [[ -r $d/memory.current ]] || continue
    [[ -z ${MEMWATCH_ONLY_ID:-} || ${d##*/} == *"$MEMWATCH_ONLY_ID"* ]] || continue
    read -r cur <"$d/memory.current" || continue
    ((cur > MW_VICTIM_BYTES)) || continue
    MW_VICTIM=$d MW_VICTIM_BYTES=$cur
  done
  ((MW_VICTIM_BYTES >= ${MEMWATCH_VICTIM_MIN_MB:-256} << 20)) || MW_VICTIM=''
}

# The command line of a cgroup's first process, for the log.
mw_cgroup_cmd() {
  local pid args
  read -r pid 2>/dev/null <"$1/cgroup.procs" || return 0
  mapfile -d '' args 2>/dev/null <"$MW_PROC/$pid/cmdline" || return 0
  printf '%s' "${args[*]:0:6}"
}

# Kill every process of a cgroup at once (cgroup v2 cgroup.kill): no signal is trapped and no
# daemon round-trip is needed, which matters when docker itself is slow under pressure.
mw_kill_cgroup() {
  local why=$1 dir=$2
  [[ ${MEMWATCH_BREAK:-0} == 1 ]] && { mw_log "MEMWATCH_BREAK set, ${dir##*/} left alive"; return 0; }
  mw_log "kill cgroup ${dir##*/} ($((MW_VICTIM_BYTES >> 20)) MiB): $why — $(mw_cgroup_cmd "$dir")"
  [[ ${MEMWATCH_DRYRUN:-0} == 1 ]] || echo 1 >"$dir/cgroup.kill"
}

# The cgroup directory of a pid when it runs inside a job cgroup, empty otherwise.
mw_job_cgroup_of() {
  local line
  read -r line 2>/dev/null <"$MW_PROC/$1/cgroup" || return 0
  line=${line#0::}
  case $line in */gm.slice/*.scope* | */app.slice/gm-*.scope*) echo "$MW_CG${line%%.scope*}.scope" ;; esac
}

# Kill one process: its whole job cgroup when it is a job, else TERM, then KILL after 5 s.
mw_kill_pid() {
  local why=$1 pid=$2 comm=$3 cg
  cg=$(mw_job_cgroup_of "$pid")
  if [[ -n $cg ]]; then
    read -r MW_VICTIM_BYTES 2>/dev/null <"$cg/memory.current" || MW_VICTIM_BYTES=0
    mw_kill_cgroup "$why (gpu client $comm pid $pid)" "$cg"
    return
  fi
  [[ ${MEMWATCH_BREAK:-0} == 1 ]] && { mw_log "MEMWATCH_BREAK set, pid $pid left alive"; return 0; }
  mw_log "kill pid $pid ($comm): $why"
  [[ ${MEMWATCH_DRYRUN:-0} == 1 ]] && return 0
  kill -TERM "$pid" 2>/dev/null || return 0
  (sleep 5 && kill -0 "$pid" 2>/dev/null && kill -KILL "$pid") &
}

# Last resort for a host thrashing with no job to blame: the kernel OOM killer, now, instead of
# after minutes of swap thrash. Job containers carry oom_score_adj 500 and still go first.
mw_kernel_oom() {
  [[ ${MEMWATCH_BREAK:-0} == 1 || ${MEMWATCH_SYSRQ:-1} == 0 ]] && return 0
  mw_log "severe pressure (avail $((MW_AVAIL >> 10)) MiB, psi full $MW_PSI) and no job to kill: kernel OOM killer"
  [[ ${MEMWATCH_DRYRUN:-0} == 1 ]] || echo f >"$MW_PROC/sysrq-trigger"
}

# One RAM tick. Returns 0 when it killed something, so the caller can cool down.
mw_ram_tick() {
  mw_ram_level
  [[ $MW_LEVEL == none ]] && return 1
  mw_job_victim
  if [[ -n $MW_VICTIM ]]; then
    mw_kill_cgroup "$MW_LEVEL memory pressure (avail $((MW_AVAIL >> 10)) MiB, psi full $MW_PSI)" "$MW_VICTIM"
    return 0
  fi
  if [[ $MW_LEVEL == severe && -z ${MEMWATCH_ONLY_ID:-} ]]; then
    mw_kernel_oom
    return 0
  fi
  return 1
}

# MW_VRAM_PCT = VRAM in use on the card with the most VRAM, as a whole percent; 0 without amdgpu.
mw_vram_pct() {
  local d used total best=0
  MW_VRAM_PCT=0
  for d in "$MW_SYS"/class/drm/card[0-9]*/device; do
    [[ -r $d/mem_info_vram_total ]] || continue
    read -r total <"$d/mem_info_vram_total"
    read -r used <"$d/mem_info_vram_used"
    ((total > best)) || continue
    best=$total MW_VRAM_PCT=$((used * 100 / total))
  done
}

# One line per process holding amdgpu VRAM, "<KiB> <pid> <comm>", largest first. A DRM client
# shared by several fds counts once (drm-client-id), as in the kernel's own accounting.
mw_gpu_clients() {
  local f key val unit id vram pid
  local -A seen=() per_pid=()
  while read -r f; do
    id='' vram=0
    while read -r key val unit; do
      case $key in
        drm-client-id:) id=$val ;;
        drm-memory-vram:) case $unit in MiB) vram=$((val << 10)) ;; GiB) vram=$((val << 20)) ;; *) vram=$val ;; esac ;;
      esac
    done <"$f"
    [[ -n $id && -z ${seen[$id]:-} ]] || continue
    seen[$id]=1
    pid=${f#"$MW_PROC"/}
    pid=${pid%%/*}
    per_pid[$pid]=$((${per_pid[$pid]:-0} + vram))
  done < <(grep -lsE '^drm-driver:\s+amdgpu' "$MW_PROC"/[0-9]*/fdinfo/* 2>/dev/null)
  for pid in "${!per_pid[@]}"; do
    printf '%s %s %s\n' "${per_pid[$pid]}" "$pid" "$(<"$MW_PROC/$pid/comm")"
  done | sort -rn
}

# MW_GPU_PID / MW_GPU_COMM / MW_GPU_KIB = the largest GPU client outside MEMWATCH_GPU_PROTECT
# holding at least $1 MiB of VRAM; MW_GPU_PID is empty when there is none.
mw_gpu_victim() {
  local kib pid comm
  MW_GPU_PID='' MW_GPU_COMM='' MW_GPU_KIB=0
  while read -r kib pid comm; do
    [[ $MW_PROTECT == *" $comm "* ]] && continue
    ((kib >= $1 << 10)) || return 0
    MW_GPU_PID=$pid MW_GPU_COMM=$comm MW_GPU_KIB=$kib
    return 0
  done < <(mw_gpu_clients)
}

# One VRAM tick: VRAM at MEMWATCH_VRAM_PCT (97) or more for MEMWATCH_VRAM_HOLD (20) ticks in a row
# frees the largest unprotected client holding MEMWATCH_VRAM_VICTIM_MIN_MB (1024) or more.
# Caveat: a full VRAM is not a hang by itself (the driver evicts to GTT); the hold and the 1 GiB
# floor keep an ordinary browser tab out of reach, but a game that fills VRAM on purpose is killed.
MW_VRAM_HELD=0
mw_vram_tick() {
  mw_vram_pct
  if ((MW_VRAM_PCT < ${MEMWATCH_VRAM_PCT:-97})); then
    MW_VRAM_HELD=0
    return 1
  fi
  ((++MW_VRAM_HELD >= ${MEMWATCH_VRAM_HOLD:-20})) || return 1
  MW_VRAM_HELD=0
  mw_gpu_victim "${MEMWATCH_VRAM_VICTIM_MIN_MB:-1024}"
  [[ -n $MW_GPU_PID ]] || return 1
  mw_kill_pid "VRAM at $MW_VRAM_PCT % for ${MEMWATCH_VRAM_HOLD:-20} ticks, freeing $((MW_GPU_KIB >> 10)) MiB" \
    "$MW_GPU_PID" "$MW_GPU_COMM"
}

# A protected process (Xorg, the compositor) hung the gfx ring: never kill it, free VRAM from the
# largest unprotected client instead, when VRAM is at MEMWATCH_RING_VRAM_PCT (90) or more.
mw_ring_relief() {
  mw_vram_pct
  if ((MW_VRAM_PCT < ${MEMWATCH_RING_VRAM_PCT:-90})); then
    mw_log "gpu ring timeout in $1 pid $2, VRAM $MW_VRAM_PCT %: left alone"
    return 0
  fi
  mw_gpu_victim "${MEMWATCH_GPU_VICTIM_MIN_MB:-512}"
  if [[ -z $MW_GPU_PID ]]; then
    mw_log "gpu ring timeout in $1 pid $2, VRAM $MW_VRAM_PCT %: no unprotected client to free"
    return 0
  fi
  mw_kill_pid "gpu ring timeout in $1 with VRAM at $MW_VRAM_PCT %, freeing $((MW_GPU_KIB >> 10)) MiB" \
    "$MW_GPU_PID" "$MW_GPU_COMM"
}

# Feed one `journalctl -k -o short-unix` line. The kernel resets the gfx ring after each timeout
# and names the process; one that hangs it twice within MEMWATCH_RING_WINDOW (120) seconds keeps
# hanging the GPU and is killed (2026-10-03 16:47: firefox, two timeouts 10 s apart).
declare -gA MW_RING=()
mw_ring_line() {
  local re='^([0-9]+)[.0-9]* .*amdgpu: +Process ([^ ]+) pid ([0-9]+) thread' ts comm pid last
  [[ $1 =~ $re ]] || return 0
  ts=${BASH_REMATCH[1]} comm=${BASH_REMATCH[2]} pid=${BASH_REMATCH[3]}
  if [[ $MW_PROTECT == *" $comm "* ]]; then
    mw_ring_relief "$comm" "$pid"
    return 0
  fi
  last=${MW_RING[$pid]:-0}
  MW_RING[$pid]=$ts
  if ((last > 0 && ts - last <= ${MEMWATCH_RING_WINDOW:-120})); then
    mw_kill_pid "second gpu ring timeout in $((ts - last)) s" "$pid" "$comm"
  else
    mw_log "gpu ring timeout in $comm pid $pid, the first in ${MEMWATCH_RING_WINDOW:-120} s"
  fi
}
