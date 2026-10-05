#!/usr/bin/env bash
# oc-run.sh <label> <worktree> <agent> <prompt-file> — one headless OpenCode job on the free model.
# Journal: <worktree>/target/wf/<label>.jsonl (+ .pid, .rc, .session-id). Exits with opencode's code.
# The worktree must already contain develop's opencode.json and .opencode/agents (merge develop first).
# Ponytail: the hard timeout (OC_TIMEOUT, default 4h) kills a slow-but-alive job too; resume it with
# `opencode run --session <id>` rather than relaunching from scratch: OC_SESSION=<id> does that.
set -uo pipefail
# GM_SCRATCH reaches the job and its MCP servers (scripts/orch/pw-mcp.sh) through the environment.
source "$(dirname "$(readlink -f "$0")")/scratch.sh"
label=$1 wt=$2 agent=$3 prompt=$4
OC=${OC:-/home/dlesieur/.opencode/bin/opencode}
MODEL=${OC_MODEL:-opencode/space-bunny-free#max}
wf=$wt/target/wf; mkdir -p "$wf"; j=$wf/$label.jsonl
echo $$ >"$wf/$label.pid"; rm -f "$wf/$label.rc"
cd "$wt" || exit 1
# A resume appends, so the journal keeps the first run's subagent calls for the orchestrator's count.
[[ -n ${OC_SESSION-} ]] || : >"$j"
# stdin is /dev/null: `opencode run` reads a non-tty stdin into the prompt, so a launcher whose stdin
# is an open socket (a backgrounded tool shell) blocked it before it reached the server (2026-10-05).
timeout "${OC_TIMEOUT:-14400}" "$OC" run -m "$MODEL" --agent "$agent" --format json --auto \
  --title "$label" ${OC_SESSION:+--session "$OC_SESSION"} "$(cat "$prompt")" </dev/null >>"$j" 2>>"$wf/$label.stderr"
rc=$?
echo "$rc" >"$wf/$label.rc"
jq -r '.. | .sessionID? // empty' "$j" 2>/dev/null | head -1 >"$wf/$label.session-id"
exit "$rc"
