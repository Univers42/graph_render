#!/usr/bin/env bash
# oc-job.sh <label> <worktree> <agent> <body-file> [rows-file] — one verified OpenCode job: prompt =
# common rules + body; run on the free model; then the rows gate; on success commit + push the branch.
# Prints ≤40 lines: job rc, the agent's return block, the gate summary. Exit 0 = done + gate green,
# 2 = agent not done (blocked/partial/died), 1 = gate red, 3 = the worktree is not provably free.
# The fence: oc-live.sh asks the OpenCode service whether a session is draining in <worktree>, and
# only its exit 1 ("asked, none") lets a job start. Exit 2 (service down, no port, no password) and
# any other failure refuse with 3 — a worktree nobody could query is not a free worktree. The pgrep
# scan below stays as a second fence, not the only one: it catches an `opencode run` the service
# has not registered yet, and oc-live's drain set misses a live session idling between turns.
# Test seams: OC_LIVE_BIN (which oc-live.sh to ask), OC_JOB_BIN (the bin dir with oc-run.sh).
set -uo pipefail
label=$1 wt=$2 agent=$3 body=$4 rows=${5-}
bin=${OC_JOB_BIN:-/goinfre/dlesieur/orch/bin}
live=$("${OC_LIVE_BIN:-$bin/oc-live.sh}" "$wt" 2>&1); lr=$?
case $lr in
  1) ;;                                        # the service was asked: nothing drains in $wt
  0) echo "refused: $live already works in $wt"; exit 3 ;;
  2) echo "refused: cannot prove $wt is free: $live"; exit 3 ;;
  *) echo "refused: oc-live.sh failed (rc=$lr): $live"; exit 3 ;;
esac
for p in $(pgrep -f '/opencode run' || true); do
  [[ $(readlink "/proc/$p/cwd" 2>/dev/null) == "$wt" ]] && { echo "refused: pid $p already works in $wt"; exit 3; }
done
wf=$wt/target/wf; mkdir -p "$wf"; prompt=$wf/$label.prompt
cat /sgoinfre/students/dlesieur/orch/prompts/common-v2.txt "$body" >"$prompt"
"$bin/oc-run.sh" "$label" "$wt" "$agent" "$prompt"; rc=$?
# The verdict reads the whole last text part: a return block longer than the printed 30 lines once
# cut `status: done` off and turned a done job into exit 2 (s1-nav, 2026-09-29).
ret=$(jq -rs '[.[] | select(.part.type=="text") | .part.text] | last // ""' "$wf/$label.jsonl" 2>/dev/null)
echo "job rc=$rc"; tail -n 30 <<<"$ret"
[[ $rc -eq 0 ]] && grep -q 'status: done' <<<"$ret" || exit 2
if [[ -n $rows ]]; then
  (cd "$wt" && "$bin/gate.sh" "target/gate-$label" "$rows") >/dev/null; g=$?
  cat "$wt/target/gate-$label/summary.txt"; [[ $g -eq 0 ]] || exit 1
fi
cd "$wt" && git add -A && { git diff --cached --quiet || git -c user.name=LESdylan \
  -c user.email=dev.pro.photo@gmail.com commit -q -m updated; } && git push -q origin HEAD 2>&1 | tail -n 2
echo "committed $(git rev-parse --abbrev-ref HEAD) $(git rev-parse --short HEAD)"
