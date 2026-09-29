#!/usr/bin/env bash
# oc-job.sh <label> <worktree> <agent> <body-file> [rows-file] — one verified OpenCode job: prompt =
# common rules + body; run on the free model; then the rows gate; on success commit + push the branch.
# Prints ≤40 lines: job rc, the agent's return block, the gate summary. Exit 0 = done + gate green,
# 2 = agent not done (blocked/partial/died), 1 = gate red, 3 = another OpenCode job already owns the worktree.
set -uo pipefail
label=$1 wt=$2 agent=$3 body=$4 rows=${5-}
bin=/goinfre/dlesieur/orch/bin
for p in $(pgrep -f '/opencode run' || true); do
  [[ $(readlink "/proc/$p/cwd" 2>/dev/null) == "$wt" ]] && { echo "refused: pid $p already works in $wt"; exit 3; }
done
wf=$wt/target/wf; mkdir -p "$wf"; prompt=$wf/$label.prompt
cat /sgoinfre/students/dlesieur/orch/prompts/common-v2.txt "$body" >"$prompt"
"$bin/oc-run.sh" "$label" "$wt" "$agent" "$prompt"; rc=$?
ret=$(jq -r 'select(.part.type=="text") | .part.text' "$wf/$label.jsonl" 2>/dev/null | tail -n 30)
echo "job rc=$rc"; echo "$ret"
[[ $rc -eq 0 ]] && grep -q 'status: done' <<<"$ret" || exit 2
if [[ -n $rows ]]; then
  (cd "$wt" && "$bin/gate.sh" "target/gate-$label" "$rows") >/dev/null; g=$?
  cat "$wt/target/gate-$label/summary.txt"; [[ $g -eq 0 ]] || exit 1
fi
cd "$wt" && git add -A && { git diff --cached --quiet || git -c user.name=LESdylan \
  -c user.email=dev.pro.photo@gmail.com commit -q -m updated; } && git push -q origin HEAD 2>&1 | tail -n 2
echo "committed $(git rev-parse --abbrev-ref HEAD) $(git rev-parse --short HEAD)"
