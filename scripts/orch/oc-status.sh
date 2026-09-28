#!/usr/bin/env bash
# oc-status.sh [root] — one line per OpenCode job journal under <root>/*/target/wf (default
# /goinfre/dlesieur/wt): label, state, minutes since the last event, session id, and the last
# text the model wrote (≤200 chars). Never prints a transcript.
# Ponytail: STALLED is mtime-based (>30 min without a journal write); a job thinking silently for
# longer is reported stalled. Check the session in the OpenCode UI before interrupting it.
set -uo pipefail
root=${1:-/goinfre/dlesieur/wt}; now=$(date +%s)
for j in "$root"/*/target/wf/*.jsonl; do
  [[ -e $j ]] || continue
  base=${j%.jsonl}; label=$(basename "$base"); wt=$(basename "$(dirname "$(dirname "$(dirname "$j")")")")
  age=$(( (now - $(stat -c %Y "$j")) / 60 ))
  if [[ -f $base.rc ]]; then state="done(rc=$(cat "$base.rc"))"
  elif [[ -f $base.pid ]] && kill -0 "$(cat "$base.pid")" 2>/dev/null; then
    state=RUNNING; ((age > 30)) && state=STALLED
  else state=DEAD; fi
  sid=$(cat "$base.session-id" 2>/dev/null)
  last=$(jq -r '.. | objects | select(.type? == "text") | .text? // empty' "$j" 2>/dev/null | tail -c 200 | tr '\n' ' ')
  printf '%-8s %-26s %-12s %4sm %s | %s\n' "$wt" "$label" "$state" "$age" "${sid:--}" "$last"
done
