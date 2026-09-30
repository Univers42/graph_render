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
# OC_COMMON picks the job preamble (default scripts/orch/common.md). The house rules reach the job
# through AGENTS.md (scripts/orch/oc-kit.sh), not through this preamble.
set -uo pipefail
label=$1 wt=$2 agent=$3 body=$4 rows=${5-}
bin=${OC_JOB_BIN:-$(dirname "$(readlink -f "$0")")}
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
# OC_SESSION=<id> resumes that session (oc-run.sh): the rules and body are already in its history,
# so the prompt is only a continue order.
resume="Continue this task from where it stopped. Re-dispatch any cancelled or unfinished subagent slice in ONE message of parallel calls, then finish with the return block."
if [[ -n ${OC_SESSION-} ]]; then
  printf '%s\n' "$resume" >"$prompt"
else
  cat "${OC_COMMON:-$bin/common.md}" "$body" >"$prompt"
fi
"$bin/oc-run.sh" "$label" "$wt" "$agent" "$prompt"; rc=$?
# A provider 429 (`provider.quota`, seen 2026-09-29 on every free model in turn), or an
# "aborted ... inactivity" end (a quota-cut stream, same evening), ends the run
# with rc 1. The job resumes its session at once on the next model of OC_FALLBACK (the free
# models that passed a tool probe on 2026-09-29, 20:50; the user named longcat, and ruled out
# nemotron and big-pickle), starting after the one that was refused, and waits OC_QUOTA_WAIT
# only once a full round of the list was refused.
# Ponytail: the wait is fixed (600 s) and ignores any Retry-After; a model that is still limited
# costs one short resume; after OC_QUOTA_TRIES (3) rounds a still-limited job exits 2.
read -ra fb <<<"${OC_FALLBACK:-opencode/longcat-2.5-preview-free opencode/mimo-v2.6-flash-free}"
off=0; for i in "${!fb[@]}"; do [[ ${fb[i]} == "${OC_MODEL-}" ]] && off=$((i + 1)); done
for ((t = 0; rc != 0 && t < ${OC_QUOTA_TRIES:-3} * ${#fb[@]}; t++)); do
  tail -n 1 "$wf/$label.jsonl" | jq -e '.error.type == "provider.quota" or (.error.type == "aborted" and ((.error.message // "") | test("inactivity")))' >/dev/null || break
  ((t > 0 && t % ${#fb[@]} == 0)) && { echo "every model limited: wait ${OC_QUOTA_WAIT:-600} s"; sleep "${OC_QUOTA_WAIT:-600}"; }
  echo "provider quota: resume $((t + 1)) on ${fb[(t + off) % ${#fb[@]}]}"
  printf '%s\n' "$resume" >"$prompt"
  OC_MODEL=${fb[(t + off) % ${#fb[@]}]} OC_SESSION=$(<"$wf/$label.session-id") \
    "$bin/oc-run.sh" "$label" "$wt" "$agent" "$prompt"; rc=$?
done
# The verdict reads the whole last text part: a return block longer than the printed 30 lines once
# cut `status: done` off and turned a done job into exit 2 (s1-nav, 2026-09-29).
last_text() { jq -rs '[.[] | select(.part.type=="text") | .part.text] | last // ""' "$wf/$label.jsonl" 2>/dev/null; }
# A space-bunny run can end rc 0 in the middle of its work, with no return block (the kit's jobs,
# 2026-09-30): resume the same session, at most OC_RESUMES (3) times, before calling it not done.
for ((r = 0; rc == 0 && r < ${OC_RESUMES:-3}; r++)); do
  last_text | grep -q '^status:' && break
  echo "no return block: resume $((r + 1))"
  printf '%s\n' "$resume" >"$prompt"
  OC_SESSION=$(<"$wf/$label.session-id") "$bin/oc-run.sh" "$label" "$wt" "$agent" "$prompt"
  rc=$?
done
ret=$(last_text)
echo "job rc=$rc"; tail -n 30 <<<"$ret"
[[ $rc -eq 0 ]] && grep -q 'status: done' <<<"$ret" || exit 2
if [[ -n $rows ]]; then
  (cd "$wt" && "$bin/gate.sh" "target/gate-$label" "$rows") >/dev/null; g=$?
  cat "$wt/target/gate-$label/summary.txt"; [[ $g -eq 0 ]] || exit 1
fi
cd "$wt" && git add -A && { git diff --cached --quiet || git -c user.name=LESdylan \
  -c user.email=dev.pro.photo@gmail.com commit -q -m updated; } && git push -q origin HEAD 2>&1 | tail -n 2
echo "committed $(git rev-parse --abbrev-ref HEAD) $(git rev-parse --short HEAD)"
