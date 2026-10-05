#!/usr/bin/env bash
# hub-mem.sh — the container half of row hub-memory (spec §6): every writer holds a MAX_BODY body
# of the worst shape at once, inside a graph-hub capped at 1 GiB of RAM and no swap.
#
#   hub-mem.sh measure    GRAPH_HUB_WRITERS=2, GRAPH_HUB_DB_POOL=8, the defaults   (row: expect 0)
#   hub-mem.sh control    GRAPH_HUB_WRITERS=6, GRAPH_HUB_DB_POOL=10                (row: expect 137)
#   hub-mem.sh upload     one GRAPH_HUB_MAX_DOC_BYTES document, five timed /layout uploads against a
#                         real graph-server in a second container (Decision 4; row hub-upload-timeout)
#
# upload execs scripts/orch/hub-mem-upload-run.sh, which starts that second container and holds the
# rest of the plumbing; scripts/orch/hub-mem-upload.sh holds the helpers it calls.
#
# Each run builds the client case, resets this worktree's database and hub (hub-pg.sh, hub-run.sh),
# and starts the hub with GM_HUB_HOLD_BODIES and GRAPH_HUB_WRITERS_PER_KEY equal to the writer count
# (src/hooks.rs: every write waits after reading its body until all have, then all parse together;
# the case sends with one key). It runs tests/memory/container.rs, reads the hub's peak (VmHWM of its
# process, memory.peak of its cgroup, the OOM flag), writes them to target/hub-mem/<verb>.txt and
# removes the hub.
#
# Exit: 0 every write answered and VmHWM under 1 GiB · 1 a write failed, the hub died, or VmHWM
#       reached 1 GiB · 137 the kernel killed the hub at its cap · 2 could not run (usage, a build,
#       docker)
#
# upload's exit codes are its own, from scripts/orch/hub-mem-upload-run.sh: 0 the slowest of the five
# uploads was under 8000 ms, the motor's log held no 408 and every Graph-Seq matched · 1 one of those
# failed, and target/hub-mem/upload.txt names which · 2 could not run.
#
# Caveat: the barrier aligns the start of every parse, not the peaks; a write that finishes before
# the slowest one peaks has freed part of its tree, so the measured peak can sit below WRITERS × the
# single-body peak of docs/measurements/hub-memory.md. The control (6 × about 213 MB) is sized to
# pass the cap even so.
# Caveat: VmHWM counts the hub's resident pages; memory.peak is what the cap is enforced on and also
# counts page cache and kernel memory charged to the container, so it reads higher. "unknown" where
# the cgroup file is not readable from the host.

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2
out=target/hub-mem
cap_kib=$((1 << 20))
features=db-tests,negctl,test-hooks
case_name=container::peak_rss_at_every_cap_fits_one_gib

case "${1-}" in
measure) writers=2 pool=8 ;;
control) writers=6 pool=10 ;;
upload) exec "$here/hub-mem-upload-run.sh" ;;
--help | -h) sed -n '2,/^$/p' "$0" | sed -e 's/^# \{0,1\}//' -e '/^$/d'; exit 0 ;;
*) echo "hub-mem: usage: hub-mem.sh measure|control|upload" >&2; exit 2 ;;
esac
verb=$1
export GRAPH_HUB_WRITERS=$writers GRAPH_HUB_WRITERS_PER_KEY=$writers GRAPH_HUB_DB_POOL=$pool
export GM_HUB_HOLD_BODIES=$writers HUB_RUN_MEMORY=1g

hub() { "$here/hub-run.sh" "$@"; }
client() {
  "$here/gr" -e HUB_MEM_WRITERS="$writers" -e GM_HUB_PG_URL="$("$here/hub-pg.sh" url)" cargo test --manifest-path server/Cargo.toml \
    -p graph-hub --features "$features" --test memory "$@"
}

# A `kB` field of /proc/<pid>/status, empty once the process is gone.
status_kib() { awk -v field="$2:" '$1 == field { print $2 }' "/proc/$1/status" 2>/dev/null; }

# The cgroup v2 directory of <pid>'s container, read while the process lives: docker removes it
# with the container.
cgroup_dir() { printf '/sys/fs/cgroup%s' "$(sed -n 's/^0:://p' "/proc/$1/cgroup" 2>/dev/null)"; }

prepare() {
  "$here/hub-pg.sh" reset && "$here/hub-pg.sh" start && hub reset && hub start
}

verdict() {
  local oom=$1 test_rc=$2 hwm=$3
  if [ "$oom" = true ]; then echo 137
  elif [ "$test_rc" -ne 0 ] || [ -z "$hwm" ]; then echo 1
  elif [ "$hwm" -ge "$cap_kib" ]; then echo 1
  else echo 0
  fi
}

client --no-run || { echo "hub-mem: the client case did not build" >&2; exit 2; }
trap 'hub reset >/dev/null 2>&1' EXIT
prepare || { echo "hub-mem: the database or the hub did not start" >&2; exit 2; }
pid=$(hub inspect '{{.State.Pid}}')
case "$pid" in '' | 0 | *[!0-9]*) echo "hub-mem: no hub process" >&2; exit 2 ;; esac
idle=$(status_kib "$pid" VmHWM)
cgroup=$(cgroup_dir "$pid")
client -- "$case_name" --exact --ignored --nocapture
test_rc=$?
hwm=$(status_kib "$pid" VmHWM)
peak=$(cat "$cgroup/memory.peak" 2>/dev/null || echo unknown)
oom=$(hub inspect '{{.State.OOMKilled}}')
rc=$(verdict "$oom" "$test_rc" "$hwm")
mkdir -p "$out"
{
  echo "verb=$verb writers=$writers pool=$pool cap_kib=$cap_kib"
  echo "idle_vmhwm_kib=$idle peak_vmhwm_kib=${hwm:-gone} cgroup_peak_bytes=$peak"
  echo "oom_killed=$oom test_exit=$test_rc exit=$rc"
} | tee "$out/$verb.txt"
exit "$rc"
