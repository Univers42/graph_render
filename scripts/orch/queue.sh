#!/usr/bin/env bash
# queue.sh run|status|land <label> — the OpenCode job queue, so no orchestrator has to babysit jobs.
# Queue: scripts/orch/queue.txt, one job per line `label|agent|brief|rows|land` (brief and rows are
# paths from the repo top; rows may be empty; land is yes|no). `#` lines are comments. The file is
# re-read every minute, so appending a line queues a job.
# run: forever, keep N jobs live (N = the number in $GM_SCRATCH/orch/queue/max, default 3, re-read
# every minute). Each job gets its own worktree from origin/develop
# (wt-new.sh) and runs through oc-job.sh (agent, rows gate, commit, push of its branch). A job that
# ends with exit 0 and land=yes is landed: develop is merged into its branch, quick.rows runs on the
# merged tree, and the branch is pushed to develop as a fast-forward, one landing at a time.
# land <label>: the landing step alone, for a job launched by hand. Exit 0 landed, 1 red, 2 could not run.
# State: $GM_SCRATCH/orch/queue/<label>.{pid,rc,land}; logs: $GM_SCRATCH/orch/logs/job-<label>.out.
# Requeue a job: delete its .pid and .rc.
# Ponytail: landing trusts quick.rows and runs no review, so a job whose brief allows code must
# carry rows that test that code. A queue.sh killed mid-job leaves that job "live" (a .pid, no .rc)
# until its files are deleted.
set -uo pipefail
bin=$(dirname "$(readlink -f "$0")")
# shellcheck source=scripts/orch/scratch.sh
source "$bin/scratch.sh"
top=$(git -C "$bin" rev-parse --show-toplevel)
q=${QUEUE_FILE:-$bin/queue.txt}
st=$GM_SCRATCH/orch/queue
logs=$GM_SCRATCH/orch/logs
mkdir -p "$st" "$logs"
git_as=(git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com)

land() { # <label>
  (
    flock 9
    cd "$GM_SCRATCH/wt/$1" || exit 2
    git fetch -q origin develop || exit 2
    "${git_as[@]}" merge -q --no-edit -m updated origin/develop || {
      git merge --abort
      exit 1
    }
    "$bin/timed" "$bin/gate.sh" "target/land-$1" "$top/scripts/orch/rows/quick.rows" >/dev/null || exit 1
    git push -q origin HEAD HEAD:develop
  ) 9>"$st/land.lock"
}

start() { # <label> <agent> <brief> <rows> <land>
  local wt=$GM_SCRATCH/wt/$1
  (
    [[ -d $wt ]] || "$bin/wt-new.sh" "$1" || exit 2
    "$bin/oc-job.sh" "$1" "$wt" "$2" "$top/$3" ${4:+"$top/$4"}
    rc=$?
    if [[ $rc -eq 0 && $5 == yes ]]; then
      land "$1"
      echo $? >"$st/$1.land"
    fi
    echo "$rc" >"$st/$1.rc"
  ) >>"$logs/job-$1.out" 2>&1 &
  echo $! >"$st/$1.pid"
}

jobs_in_queue() { grep -v -e '^#' -e '^[[:space:]]*$' "$q"; }

status() {
  local label age j
  while IFS='|' read -r label _; do
    j=$GM_SCRATCH/wt/$label/target/wf/$label.jsonl
    age=-
    [[ -f $j ]] && age=$(($(date +%s) - $(stat -c %Y "$j")))s
    if [[ -f $st/$label.rc ]]; then
      printf '%-22s done rc=%s land=%s\n' "$label" "$(<"$st/$label.rc")" "$(cat "$st/$label.land" 2>/dev/null || echo -)"
    elif [[ -f $st/$label.pid ]]; then
      printf '%-22s live pid=%s journal-age=%s\n' "$label" "$(<"$st/$label.pid")" "$age"
    else
      printf '%-22s pending\n' "$label"
    fi
  done < <(jobs_in_queue)
}

run() {
  local label agent brief rows landq live
  while :; do
    live=0
    for f in "$st"/*.pid; do [[ -f $f && ! -f ${f%.pid}.rc ]] && live=$((live + 1)); done
    while IFS='|' read -r label agent brief rows landq; do
      [[ -f $st/$label.pid ]] && continue
      (($(cat "$st/max" 2>/dev/null || echo 3) > live)) || break
      start "$label" "$agent" "$brief" "$rows" "$landq"
      live=$((live + 1))
      sleep 20
    done < <(jobs_in_queue)
    sleep 60
  done
}

case ${1-} in
  run) run ;;
  status) status ;;
  land) [[ -n ${2-} ]] && land "$2" ;;
  *)
    echo "usage: queue.sh run|status|land <label>" >&2
    exit 2
    ;;
esac
