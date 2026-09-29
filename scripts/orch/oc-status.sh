#!/usr/bin/env bash
# oc-status.sh [root] — one line per OpenCode job journal under <root>/*/target/wf (default
# /goinfre/dlesieur/wt): label, state, minutes since the last event, session id, and the last
# text the model wrote (≤200 chars). Never prints a transcript.
# State: done(rc) when <label>.rc exists; else the OpenCode service is the liveness authority —
# RUNNING when oc-live.sh finds this label's .session-id (or any session) draining in the
# worktree; DEAD when the service says no session works there; UNKNOWN when the service could
# not be asked. The .pid file is not consulted: it outlives the process only by accident.
# Ponytail: STALLED is mtime-based (>30 min without a journal write); a job thinking silently for
# longer is reported stalled. Check the session in the OpenCode UI before interrupting it.
# Ponytail: UNKNOWN is not DEAD. oc-live.sh exits 2 when it cannot reach the service, and a
# worktree nobody could query is not a worktree with nothing running in it — a restart of the
# service empties the drain set without killing any job, so DEAD after a restart is an artefact,
# not evidence. Test seam: OC_LIVE_BIN (the oc-live.sh to ask).
set -uo pipefail
root=${1:-/goinfre/dlesieur/wt}; now=$(date +%s)
bin=/goinfre/dlesieur/orch/bin
oc_live=${OC_LIVE_BIN:-$bin/oc-live.sh}
for j in "$root"/*/target/wf/*.jsonl; do
  [[ -e $j ]] || continue
  base=${j%.jsonl}; label=$(basename "$base")
  wtd=$(dirname "$(dirname "$(dirname "$j")")"); wt=$(basename "$wtd")
  age=$(( (now - $(stat -c %Y "$j")) / 60 ))
  sid=$(cat "$base.session-id" 2>/dev/null); [[ -n $sid ]] || sid=$(jq -r '.sessionID // empty' "$j" 2>/dev/null | head -1)
  if [[ -f $base.rc ]]; then state="done(rc=$(cat "$base.rc"))"
  else
    "$oc_live" "$wtd" "$sid" >/dev/null 2>&1; lr=$?
    case $lr in
      0) state=RUNNING; ((age > 30)) && state=STALLED ;;
      1) state=DEAD ;;
      *) state=UNKNOWN ;;
    esac
  fi
  last=$(jq -r '.. | objects | select(.type? == "text") | .text? // empty' "$j" 2>/dev/null | tail -c 200 | tr '\n' ' ')
  printf '%-8s %-26s %-12s %4sm %s | %s\n' "$wt" "$label" "$state" "$age" "${sid:--}" "$last"
done
